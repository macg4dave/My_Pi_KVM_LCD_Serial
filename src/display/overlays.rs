use std::time::Duration;
use std::time::Instant;

use crate::{
    display::{
        icon_bank::{IconBank, IconPalette, PaletteRequest},
        lcd::Lcd,
    },
    payload::{Icon, RenderFrame},
    Error, Result,
};

const SCROLL_GAP: &str = "    |    ";

/// Render a single frame with no scrolling offsets.
pub fn render_frame_once(lcd: &mut Lcd, frame: &RenderFrame) -> Result<()> {
    let mut icon_bank = IconBank::new();
    render_frame_with_scroll(lcd, frame, (0, 0), false, &mut icon_bank).map(|_| ())
}

/// Render a frame, applying scroll offsets and optional heartbeat overlay.
pub fn render_frame_with_scroll(
    lcd: &mut Lcd,
    frame: &RenderFrame,
    offsets: (usize, usize),
    heartbeat_on: bool,
    icon_bank: &mut IconBank,
) -> Result<IconPalette> {
    lcd.set_blink(frame.blink)?;

    if frame.clear {
        lcd.clear()?;
    }

    let width = lcd.cols() as usize;
    let palette = icon_bank.build_palette(
        lcd,
        PaletteRequest {
            bar_required: frame.bar_percent.is_some(),
            heartbeat: heartbeat_on,
            icons: &frame.icons,
        },
    )?;
    let bar_row = frame.bar_row;
    let mut line1 = if bar_row == Some(0) && frame.bar_percent.is_some() {
        render_bar(frame.bar_percent.unwrap(), width, &palette)
    } else {
        view_line(&frame.line1, width, offsets.0, frame.scroll_enabled)
    };
    let mut line2 = if bar_row == Some(1) && frame.bar_percent.is_some() {
        render_bar(frame.bar_percent.unwrap(), width, &palette)
    } else {
        view_line(&frame.line2, width, offsets.1, frame.scroll_enabled)
    };

    if heartbeat_on && width > 0 {
        if bar_row == Some(0) {
            overlay_heartbeat(&mut line2, width, &palette);
        } else {
            overlay_heartbeat(&mut line1, width, &palette);
        }
    }

    overlay_icons(
        &mut line1,
        &mut line2,
        width,
        &frame.icons,
        bar_row,
        &palette,
    );

    let out1 = if line1.trim().is_empty() && bar_row != Some(0) {
        ""
    } else {
        &line1
    };
    let out2 = if line2.trim().is_empty() && bar_row != Some(1) {
        ""
    } else {
        &line2
    };

    lcd.write_lines(out1, out2)?;
    Ok(palette)
}

/// Avoids flicker by respecting a minimum interval between render calls.
pub fn render_if_allowed(
    lcd: &mut Lcd,
    frame: &RenderFrame,
    last_render: &mut Instant,
    min_interval: Duration,
    scroll_offsets: (usize, usize),
    heartbeat_on: bool,
    icon_bank: &mut IconBank,
) -> Result<Option<IconPalette>> {
    let now = Instant::now();
    if now.duration_since(*last_render) < min_interval {
        return Ok(None);
    }
    *last_render = now;
    let palette = render_frame_with_scroll(lcd, frame, scroll_offsets, heartbeat_on, icon_bank)?;
    Ok(Some(palette))
}

pub fn line_needs_scroll(text: &str, width: usize) -> bool {
    text.chars().count() > width
}

pub fn advance_offset(text: &str, width: usize, current: usize) -> usize {
    let len = text.chars().count();
    if len <= width {
        return 0;
    }
    let gap_len = SCROLL_GAP.chars().count();
    let cycle = (2 * len) + gap_len; // text + gap + text
    (current + 1) % cycle
}

pub fn render_parse_error(lcd: &mut Lcd, cols: u8, err: &Error) -> Result<()> {
    let width = cols as usize;
    let msg = truncate_with_ellipsis(&format!("{err}"), width);
    lcd.set_backlight(true)?;
    lcd.set_blink(true)?;
    lcd.write_line(0, "ERR PARSE")?;
    lcd.write_line(1, &msg)?;
    Ok(())
}

pub fn render_reconnecting(lcd: &mut Lcd, cols: u8) -> Result<()> {
    let width = cols as usize;
    let title: String = "RECONNECTING".chars().take(width).collect();
    let detail = truncate_to_width("retrying...", width);
    lcd.clear()?;
    lcd.set_backlight(true)?;
    lcd.set_blink(false)?;
    lcd.write_line(0, &title)?;
    lcd.write_line(1, &detail)?;
    Ok(())
}

pub fn render_offline_message(lcd: &mut Lcd, cols: u8) -> Result<()> {
    let width = cols as usize;
    let title: String = truncate_to_width("SERIAL OFFLINE", width);
    let detail = truncate_to_width("will retry...", width);
    lcd.clear()?;
    lcd.set_backlight(true)?;
    lcd.set_blink(true)?;
    lcd.write_line(0, &title)?;
    lcd.write_line(1, &detail)?;
    Ok(())
}

fn render_bar(percent: u8, width: usize, palette: &IconPalette) -> String {
    if width == 0 {
        return String::new();
    }

    let max_level = 5usize;
    let total_units = width * max_level;
    let filled_units = (percent as usize * total_units) / 100;
    let mut s = String::with_capacity(width);
    for col in 0..width {
        let remaining = filled_units.saturating_sub(col * max_level);
        let level = remaining.min(max_level);
        s.push(palette.bar_char(level));
    }
    s
}

fn view_with_scroll(text: &str, width: usize, offset: usize) -> String {
    let chars: Vec<char> = text.chars().collect();
    if chars.len() <= width {
        return text.to_string();
    }
    let gap: Vec<char> = SCROLL_GAP.chars().collect();
    let mut cycle: Vec<char> = chars.clone();
    cycle.extend_from_slice(&gap);
    cycle.extend_from_slice(&chars);

    let start = if cycle.is_empty() {
        0
    } else {
        offset % cycle.len()
    };
    cycle.iter().cycle().skip(start).take(width).collect()
}

fn truncate_to_width(text: &str, width: usize) -> String {
    text.chars().take(width).collect()
}

fn view_line(text: &str, width: usize, offset: usize, scroll_enabled: bool) -> String {
    if scroll_enabled {
        return view_with_scroll(text, width, offset);
    }
    truncate_with_ellipsis(text, width)
}

fn truncate_with_ellipsis(text: &str, width: usize) -> String {
    if text.chars().count() <= width {
        return text.to_string();
    }
    if width <= 3 {
        return truncate_to_width(text, width);
    }
    let mut s: String = text.chars().take(width - 3).collect();
    s.push_str("...");
    s
}

fn overlay_heartbeat(text: &mut String, width: usize, palette: &IconPalette) {
    if width == 0 {
        return;
    }
    let mut chars: Vec<char> = text.chars().collect();
    if chars.len() < width {
        chars.resize(width, ' ');
    } else if chars.len() > width {
        chars.truncate(width);
    }
    if let Some(last) = chars.last_mut() {
        *last = palette.heartbeat_char();
    }
    *text = chars.into_iter().collect();
}

fn overlay_icons(
    line1: &mut String,
    line2: &mut String,
    width: usize,
    icons: &[Icon],
    bar_row: Option<u8>,
    palette: &IconPalette,
) {
    if icons.is_empty() || width == 0 {
        return;
    }
    let target = if bar_row == Some(1) { line1 } else { line2 };
    let icon = icons[0];
    let Some(icon_char) = palette.icon_char(icon).or_else(|| icon.ascii_fallback()) else {
        return;
    };
    let mut chars: Vec<char> = target.chars().collect();
    if chars.len() < width {
        chars.resize(width, ' ');
    } else if chars.len() > width {
        chars.truncate(width);
    }
    if let Some(last) = chars.last_mut() {
        *last = icon_char;
    }
    *target = chars.into_iter().collect();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn view_with_scroll_wraps_through_gap() {
        let text = "HELLOWORLD";
        let width = 4;
        let len = text.chars().count();

        let start = view_with_scroll(text, width, 0);
        let before_gap = view_with_scroll(text, width, len - 1);
        let after_gap = view_with_scroll(text, width, len + SCROLL_GAP.chars().count() + len);

        assert_ne!(before_gap, start, "should advance before wrap");
        assert_eq!(after_gap, start, "should wrap around after gap");
    }

    #[test]
    fn view_with_scroll_shows_gap_marker() {
        let text = "HELLOWORLD";
        let width = 5;
        let offset = text.chars().count() + SCROLL_GAP.chars().position(|c| c == '|').unwrap_or(0);
        let view = view_with_scroll(text, width, offset);
        assert!(
            view.contains('|'),
            "gap marker '|' should appear during scroll"
        );
    }

    #[test]
    fn view_line_truncates_with_ellipsis_when_scroll_disabled() {
        let text = "THIS STRING IS LONG";
        let view = view_line(text, 6, 0, false);
        assert_eq!(view, "THI...");
    }
}
