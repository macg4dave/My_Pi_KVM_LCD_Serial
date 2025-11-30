use crate::{
    cli::RunOptions,
    config::{Config, DEFAULT_BAUD, DEFAULT_COLS, DEFAULT_DEVICE, DEFAULT_ROWS},
    lcd::{Lcd, BAR_EMPTY, BAR_FULL},
    payload::Defaults as PayloadDefaults,
    payload::RenderFrame,
    serial::SerialPort,
    Error, Result,
};
use std::{fs, time::{Duration, Instant}};

const SCROLL_GAP: &str = "    |    ";
/// Config for the daemon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    pub device: String,
    pub baud: u32,
    pub cols: u8,
    pub rows: u8,
    pub scroll_speed_ms: u64,
    pub page_timeout_ms: u64,
    pub button_gpio_pin: Option<u8>,
    pub payload_file: Option<String>,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            device: DEFAULT_DEVICE.to_string(),
            baud: DEFAULT_BAUD,
            cols: DEFAULT_COLS,
            rows: DEFAULT_ROWS,
            scroll_speed_ms: crate::payload::DEFAULT_SCROLL_MS,
            page_timeout_ms: crate::payload::DEFAULT_PAGE_TIMEOUT_MS,
            button_gpio_pin: None,
            payload_file: None,
        }
    }
}

pub struct App {
    config: AppConfig,
}

impl App {
    pub fn new(config: AppConfig) -> Self {
        Self { config }
    }

    pub fn from_options(opts: RunOptions) -> Result<Self> {
        let cfg_file = Config::load_or_default()?;
        let merged = AppConfig::from_sources(cfg_file, opts);
        Ok(Self::new(merged))
    }

    /// Entry point for the daemon. Wire up serial + LCD here.
    pub fn run(&self) -> Result<()> {
        let mut lcd = Lcd::new(self.config.cols, self.config.rows)?;
        lcd.render_boot_message()?;

        if let Some(path) = &self.config.payload_file {
            let defaults = PayloadDefaults {
                scroll_speed_ms: self.config.scroll_speed_ms,
                page_timeout_ms: self.config.page_timeout_ms,
            };
            let frame = load_payload_from_file(path, defaults)?;
            lcd.set_backlight(frame.backlight_on)?;
            lcd.set_blink(frame.blink)?;
            return render_frame(&mut lcd, &frame);
        }

        let mut port = SerialPort::connect(&self.config.device, self.config.baud)?;
        port.send_line("INIT")?;

        let mut state = crate::state::RenderState::new(Some(PayloadDefaults {
            scroll_speed_ms: self.config.scroll_speed_ms,
            page_timeout_ms: self.config.page_timeout_ms,
        }));
        let mut buffer = String::new();
        let mut last_render = Instant::now();
        let min_render_interval = Duration::from_millis(200);
        let mut current_frame: Option<RenderFrame> = None;
        let mut next_page = Instant::now();
        let mut next_scroll = Instant::now();
        let mut scroll_offsets = (0usize, 0usize);
        let mut button = Button::new(self.config.button_gpio_pin).ok();
        let mut backlight_state = true;
        let blink_interval = Duration::from_millis(500);
        let mut next_blink = Instant::now();

        loop {
            let now = Instant::now();

            if let Some(btn) = button.as_mut() {
                if btn.is_pressed() {
                    if let Some(frame) = state.next_page() {
                        current_frame = Some(frame);
                        scroll_offsets = (0, 0);
                        next_scroll =
                            now + Duration::from_millis(self.config.scroll_speed_ms);
                        lcd.clear()?;
                        if let Some(frame) = current_frame.as_ref() {
                            next_page = now + Duration::from_millis(frame.page_timeout_ms);
                            render_if_allowed(
                                &mut lcd,
                                frame,
                                &mut last_render,
                                min_render_interval,
                                scroll_offsets,
                            )?;
                        }
                    }
                }
            }

            buffer.clear();
            let read = port.read_line(&mut buffer)?;
            if read > 0 {
                let line = buffer.trim_end_matches(&['\r', '\n'][..]).trim();
                if !line.is_empty() {
                    match state.ingest(line) {
                        Ok(Some(frame)) => {
                            current_frame = Some(frame.clone());
                            scroll_offsets = (0, 0);
                            next_scroll =
                                now + Duration::from_millis(self.config.scroll_speed_ms);
                            lcd.clear()?;
                            backlight_state = frame.backlight_on;
                            lcd.set_backlight(backlight_state)?;
                            lcd.set_blink(frame.blink)?;
                            next_blink = now + blink_interval;
                            if let Some(frame) = current_frame.as_ref() {
                                next_page =
                                    now + Duration::from_millis(frame.page_timeout_ms);
                                render_if_allowed(
                                    &mut lcd,
                                    frame,
                                    &mut last_render,
                                    min_render_interval,
                                    scroll_offsets,
                                )?;
                            }
                        }
                        Ok(None) => { /* duplicate */ }
                        Err(err) => eprintln!("frame error: {err}"),
                    }
                }
            }

                if state.len() > 1 && now >= next_page {
                    if let Some(frame) = state.next_page() {
                        current_frame = Some(frame);
                        scroll_offsets = (0, 0);
                        if let Some(frame) = current_frame.as_ref() {
                            next_page = now + Duration::from_millis(frame.page_timeout_ms);
                            lcd.clear()?;
                            backlight_state = frame.backlight_on;
                            lcd.set_backlight(backlight_state)?;
                            lcd.set_blink(frame.blink)?;
                            next_blink = now + blink_interval;
                            render_if_allowed(
                            &mut lcd,
                            frame,
                            &mut last_render,
                            min_render_interval,
                            scroll_offsets,
                        )?;
                    }
                }
            }

            if let Some(frame) = current_frame.as_ref() {
                let width = lcd.cols() as usize;
                let needs_scroll = match frame.bar_row {
                    Some(0) => line_needs_scroll(&frame.line2, width),
                    Some(1) => line_needs_scroll(&frame.line1, width),
                    _ => line_needs_scroll(&frame.line1, width)
                        || line_needs_scroll(&frame.line2, width),
                };
                if needs_scroll && now >= next_scroll {
                    scroll_offsets = (
                        advance_offset(&frame.line1, lcd.cols() as usize, scroll_offsets.0),
                        advance_offset(&frame.line2, lcd.cols() as usize, scroll_offsets.1),
                    );
                    next_scroll =
                        now + Duration::from_millis(frame.scroll_speed_ms);
                    render_if_allowed(
                        &mut lcd,
                        frame,
                        &mut last_render,
                        min_render_interval,
                        scroll_offsets,
                    )?;
                }

                if frame.blink {
                    if now >= next_blink {
                        backlight_state = !backlight_state;
                        lcd.set_backlight(backlight_state)?;
                        next_blink = now + blink_interval;
                    }
                } else if backlight_state != frame.backlight_on {
                    backlight_state = frame.backlight_on;
                    lcd.set_backlight(backlight_state)?;
                }
            }
        }
    }
}

impl AppConfig {
    pub fn from_sources(config: Config, opts: RunOptions) -> Self {
        Self {
            device: opts
                .device
                .unwrap_or_else(|| config.device.clone()),
            baud: opts.baud.unwrap_or(config.baud),
            cols: opts.cols.unwrap_or(config.cols),
            rows: opts.rows.unwrap_or(config.rows),
            scroll_speed_ms: config.scroll_speed_ms,
            page_timeout_ms: config.page_timeout_ms,
            button_gpio_pin: config.button_gpio_pin,
            payload_file: opts.payload_file,
        }
    }
}

fn render_frame(lcd: &mut Lcd, frame: &RenderFrame) -> Result<()> {
    render_frame_with_scroll(lcd, frame, (0, 0))
}

fn load_payload_from_file(path: &str, defaults: PayloadDefaults) -> Result<RenderFrame> {
    let raw = fs::read_to_string(path)?;
    RenderFrame::from_payload_json_with_defaults(&raw, defaults)
}

fn render_bar(percent: u8, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    let interior = width;
    let filled = (percent as usize * interior) / 100;
    let mut s = String::with_capacity(width);
    for i in 0..interior {
        s.push(if i < filled { BAR_FULL } else { BAR_EMPTY });
    }
    s
}

fn render_if_allowed(
    lcd: &mut Lcd,
    frame: &RenderFrame,
    last_render: &mut Instant,
    min_interval: Duration,
    scroll_offsets: (usize, usize),
) -> Result<()> {
    let now = Instant::now();
    if now.duration_since(*last_render) < min_interval {
        return Ok(());
    }
    *last_render = now;
    render_frame_with_scroll(lcd, frame, scroll_offsets)
}

fn render_frame_with_scroll(
    lcd: &mut Lcd,
    frame: &RenderFrame,
    offsets: (usize, usize),
) -> Result<()> {
    lcd.set_blink(frame.blink)?;

    if frame.clear {
        lcd.clear()?;
    }

    let width = lcd.cols() as usize;
    let bar_row = frame.bar_row;
    let line1 = if bar_row == Some(0) && frame.bar_percent.is_some() {
        render_bar(frame.bar_percent.unwrap(), width)
    } else {
        view_with_scroll(&frame.line1, width, offsets.0)
    };
    let line2 = if bar_row == Some(1) && frame.bar_percent.is_some() {
        render_bar(frame.bar_percent.unwrap(), width)
    } else {
        view_with_scroll(&frame.line2, width, offsets.1)
    };

    if line1.trim().is_empty() && bar_row != Some(0) {
        lcd.write_line(0, "")?;
    } else {
        lcd.write_line(0, &line1)?;
    }

    if line2.trim().is_empty() && bar_row != Some(1) {
        lcd.write_line(1, "")?;
    } else {
        lcd.write_line(1, &line2)?;
    }
    Ok(())
}

fn line_needs_scroll(text: &str, width: usize) -> bool {
    text.chars().count() > width
}

fn advance_offset(text: &str, width: usize, current: usize) -> usize {
    let len = text.chars().count();
    if len <= width {
        return 0;
    }
    let gap_len = SCROLL_GAP.chars().count();
    let cycle = (2 * len) + gap_len; // text + gap + text
    (current + 1) % cycle
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
    cycle
        .iter()
        .cycle()
        .skip(start)
        .take(width)
        .collect()
}

#[cfg(target_os = "linux")]
struct Button {
    pin: rppal::gpio::InputPin,
    last: Instant,
    debounce: Duration,
}

#[cfg(target_os = "linux")]
impl Button {
    fn new(pin: Option<u8>) -> Result<Self> {
        let pin = match pin {
            Some(p) => p,
            None => return Err(Error::InvalidArgs("no button pin configured".into())),
        };
        let gpio = rppal::gpio::Gpio::new()
            .map_err(|e| Error::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;
        let input = gpio
            .get(pin)
            .map_err(|e| Error::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?
            .into_input_pullup();
        Ok(Self {
            pin: input,
            last: Instant::now(),
            debounce: Duration::from_millis(150),
        })
    }

    fn is_pressed(&mut self) -> bool {
        let now = Instant::now();
        if self.pin.is_low() && now.duration_since(self.last) > self.debounce {
            self.last = now;
            true
        } else {
            false
        }
    }
}

#[cfg(not(target_os = "linux"))]
struct Button;

#[cfg(not(target_os = "linux"))]
impl Button {
    fn new(_pin: Option<u8>) -> Result<Self> {
        Err(Error::InvalidArgs("button unsupported on this platform".into()))
    }

    fn is_pressed(&mut self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn set_temp_home() -> std::path::PathBuf {
        let mut dir = std::env::temp_dir();
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        dir.push(format!("seriallcd_app_test_home_{stamp}"));
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("HOME", &dir);
        dir
    }

    #[test]
    fn config_from_options() {
        let home = set_temp_home();
        let opts = RunOptions {
            device: Some("/dev/ttyUSB1".into()),
            baud: Some(57_600),
            cols: Some(16),
            rows: Some(2),
            payload_file: None,
        };
        let cfg = AppConfig::from_sources(Config::default(), opts.clone());
        assert_eq!(cfg.device, "/dev/ttyUSB1");
        assert_eq!(cfg.baud, 57_600);
        assert_eq!(cfg.cols, 16);
        assert_eq!(cfg.rows, 2);

        let app = App::from_options(opts).unwrap();
        assert_eq!(app.config.device, "/dev/ttyUSB1");
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn config_prefers_file_values_when_cli_missing() {
        let home = set_temp_home();
        let cfg_file = Config {
            device: "/dev/ttyS0".into(),
            baud: 9_600,
            cols: 16,
            rows: 2,
            scroll_speed_ms: crate::config::DEFAULT_SCROLL_MS,
            page_timeout_ms: crate::config::DEFAULT_PAGE_TIMEOUT_MS,
            button_gpio_pin: None,
        };
        let opts = RunOptions::default();
        let merged = AppConfig::from_sources(cfg_file.clone(), opts);
        assert_eq!(merged.device, cfg_file.device);
        assert_eq!(merged.baud, cfg_file.baud);
        assert_eq!(merged.cols, cfg_file.cols);
        assert_eq!(merged.rows, cfg_file.rows);
        let _ = std::fs::remove_dir_all(home);
    }

    #[test]
    fn view_with_scroll_wraps_through_gap() {
        let text = "HELLOWORLD";
        let width = 4;
        let len = text.chars().count();

        let start = view_with_scroll(text, width, 0);
        let before_gap = view_with_scroll(text, width, len - 1);
        let after_gap =
            view_with_scroll(text, width, len + SCROLL_GAP.chars().count() + len);

        assert_ne!(before_gap, start, "should advance before wrap");
        assert_eq!(after_gap, start, "should wrap around after gap");
    }

    #[test]
    fn view_with_scroll_shows_gap_marker() {
        let text = "HELLOWORLD";
        let width = 5;
        let offset = text.chars().count() + SCROLL_GAP.chars().position(|c| c == '|').unwrap_or(0);
        let view = view_with_scroll(text, width, offset);
        assert!(view.contains('|'), "gap marker '|' should appear during scroll");
    }
}
