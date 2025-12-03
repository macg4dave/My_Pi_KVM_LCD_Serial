use super::{lifecycle::create_shutdown_flag, AppConfig, Logger};
use crate::{
    display::overlays::{
        advance_offset, line_needs_scroll, render_if_allowed, render_offline_message,
    },
    lcd::Lcd,
    payload::{Defaults as PayloadDefaults, RenderFrame},
    Error, Result,
};
use std::{
    thread,
    time::{Duration, Instant},
};

const MIN_RENDER_MS: u64 = 200;
const BLINK_INTERVAL_MS: u64 = 500;

const DEMO_PAYLOADS: [&str; 22] = [
    r#"{"schema_version":1,"line1":"Up 12:34 CPU 42%","line2":"RAM 73%","bar_value":73,"bar_max":100,"bar_label":"RAM","mode":"dashboard","page_timeout_ms":4000}"#,
    r#"{"schema_version":1,"line1":"CPU LOAD","line2":"Cores busy","bar":68,"bar_label":"CPU","page_timeout_ms":3500}"#,
    r#"{"schema_version":1,"line1":"MEM usage","line2":"Using 1.8GB","bar_value":720,"bar_max":1000,"bar_label":"MEM","page_timeout_ms":3500}"#,
    r#"{"schema_version":1,"line1":"DISK {0x00} /","line2":"85% used","bar":85,"bar_label":"DISK","page_timeout_ms":3500}"#,
    r#"{"schema_version":1,"line1":"NET {0x00} 12.3Mbps","line2":"bar on top","bar":65,"bar_line1":true,"icons":["battery"],"page_timeout_ms":3500}"#,
    r#"{"schema_version":1,"line1":"ALERT: Temp","line2":"85C HOT!","blink":true,"duration_ms":8000,"page_timeout_ms":4000}"#,
    r#"{"schema_version":1,"line1":"ALERT: Fan Fail","line2":"Check cooling","blink":true,"backlight":true,"page_timeout_ms":4000}"#,
    r#"{"schema_version":1,"line1":"Backlight OFF demo","line2":"It should go dark","backlight":false,"page_timeout_ms":3500}"#,
    r#"{"schema_version":1,"line1":"Clear + Test Pattern","line2":"Ensure wiring is OK","clear":true,"test":true,"page_timeout_ms":3500}"#,
    r#"{"schema_version":1,"line1":"Long banner that scrolls across the top line without showing line 2","line2":"ignored","mode":"banner","scroll_speed_ms":220,"page_timeout_ms":5000}"#,
    r#"{"schema_version":1,"line1":"Scroll disabled for this long string that would otherwise move","line2":"","scroll":false,"page_timeout_ms":4000}"#,
    r#"{"schema_version":1,"line1":"TTL example","line2":"Expires quickly","duration_ms":2000,"page_timeout_ms":3000}"#,
    r#"{"schema_version":1,"line1":"Config reload hint","line2":"Reload config now","config_reload":true,"page_timeout_ms":3000}"#,
    r#"{"schema_version":1,"line1":"Dashboard forces bottom bar","line2":"even if requested top","bar":88,"bar_line1":true,"mode":"dashboard","page_timeout_ms":4000}"#,
    r#"{"schema_version":1,"line1":"Top bar only","line2":"bar_line1=true","bar":50,"bar_line1":true,"page_timeout_ms":3000}"#,
    r#"{"schema_version":1,"line1":"Icons: Heart","line2":"{0x06} beats","icons":["heart"],"page_timeout_ms":3000}"#,
    r#"{"schema_version":1,"line1":"Icons: Arrow","line2":"Look right","icons":["arrow"],"page_timeout_ms":3000}"#,
    r#"{"schema_version":1,"line1":"Icons: Battery","line2":"Charge 90%","icons":["battery"],"bar":90,"page_timeout_ms":3000}"#,
    r#"{"schema_version":1,"line1":"Fast scroll speed","line2":"0123456789abcdef0123456789abcdef","scroll_speed_ms":120,"page_timeout_ms":4000}"#,
    r#"{"schema_version":1,"line1":"Slow scroll speed","line2":"abcdefghijklmnopqrstuvwxyz","scroll_speed_ms":400,"page_timeout_ms":4000}"#,
    r#"{"schema_version":1,"line1":"Wide bar label","line2":"","bar":40,"bar_label":"NETWORK","page_timeout_ms":3000}"#,
    r#"{"schema_version":1,"line1":"Checksum demo","line2":"no checksum set","page_timeout_ms":2500}"#,
];

pub fn run_demo(lcd: &mut Lcd, config: &mut AppConfig, logger: &Logger) -> Result<()> {
    let defaults = PayloadDefaults {
        scroll_speed_ms: config.scroll_speed_ms,
        page_timeout_ms: config.page_timeout_ms,
    };
    let frames = build_demo_frames(defaults)?;
    logger.info(format!(
        "demo: cycling {} frames (ctrl-c to exit)",
        frames.len()
    ));

    let running = create_shutdown_flag()?;
    let mut idx = 0usize;
    let mut current_frame = frames[idx].clone();
    logger.info(format!("demo payload: {}", DEMO_PAYLOADS[idx]));
    let mut last_render = Instant::now();
    let min_render_interval = Duration::from_millis(MIN_RENDER_MS);
    let mut scroll_offsets = super::events::ScrollOffsets::zero();
    let mut next_scroll = Instant::now();
    let mut next_page = Instant::now() + Duration::from_millis(current_frame.page_timeout_ms);
    let mut backlight_state = current_frame.backlight_on;
    let blink_interval = Duration::from_millis(BLINK_INTERVAL_MS);
    let mut next_blink = Instant::now() + blink_interval;

    lcd.clear()?;
    lcd.set_backlight(current_frame.backlight_on)?;
    lcd.set_blink(current_frame.blink)?;
    render_if_allowed(
        lcd,
        &current_frame,
        &mut last_render,
        min_render_interval,
        (scroll_offsets.top, scroll_offsets.bottom),
        false,
    )?;

    while running.load(std::sync::atomic::Ordering::SeqCst) {
        let now = Instant::now();

        // Page advance
        if now >= next_page {
            idx = (idx + 1) % frames.len();
            current_frame = frames[idx].clone();
            logger.info(format!("demo payload: {}", DEMO_PAYLOADS[idx]));
            scroll_offsets = super::events::ScrollOffsets::zero();
            next_scroll = now + Duration::from_millis(current_frame.scroll_speed_ms);
            next_page = now + Duration::from_millis(current_frame.page_timeout_ms);
            backlight_state = current_frame.backlight_on;
            lcd.clear()?;
            lcd.set_backlight(current_frame.backlight_on)?;
            lcd.set_blink(current_frame.blink)?;
            render_if_allowed(
                lcd,
                &current_frame,
                &mut last_render,
                min_render_interval,
                (scroll_offsets.top, scroll_offsets.bottom),
                false,
            )?;
        }

        // Scrolling
        let width = lcd.cols() as usize;
        let needs_scroll = match current_frame.bar_row {
            Some(0) => {
                current_frame.scroll_enabled && line_needs_scroll(&current_frame.line2, width)
            }
            Some(1) => {
                current_frame.scroll_enabled && line_needs_scroll(&current_frame.line1, width)
            }
            _ => {
                current_frame.scroll_enabled
                    && (line_needs_scroll(&current_frame.line1, width)
                        || line_needs_scroll(&current_frame.line2, width))
            }
        };
        if needs_scroll && now >= next_scroll {
            scroll_offsets = scroll_offsets.update(
                advance_offset(
                    &current_frame.line1,
                    lcd.cols() as usize,
                    scroll_offsets.top,
                ),
                advance_offset(
                    &current_frame.line2,
                    lcd.cols() as usize,
                    scroll_offsets.bottom,
                ),
            );
            next_scroll = now + Duration::from_millis(current_frame.scroll_speed_ms);
            render_if_allowed(
                lcd,
                &current_frame,
                &mut last_render,
                min_render_interval,
                (scroll_offsets.top, scroll_offsets.bottom),
                false,
            )?;
        }

        // Blink backlight for alert frames.
        if current_frame.blink && now >= next_blink {
            backlight_state = !backlight_state;
            lcd.set_backlight(backlight_state)?;
            next_blink = now + blink_interval;
        }

        thread::sleep(Duration::from_millis(25));
    }

    render_offline_message(lcd, config.cols)?;
    Ok(())
}

fn build_demo_frames(defaults: PayloadDefaults) -> Result<Vec<RenderFrame>> {
    let mut frames = Vec::with_capacity(DEMO_PAYLOADS.len());
    for raw in DEMO_PAYLOADS {
        match RenderFrame::from_payload_json_with_defaults(raw, defaults) {
            Ok(frame) => frames.push(frame),
            Err(err) => return Err(Error::Parse(format!("demo payload invalid: {err}"))),
        }
    }
    Ok(frames)
}
