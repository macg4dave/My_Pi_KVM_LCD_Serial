use std::{
    thread,
    time::{Duration, Instant},
};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

use super::connection::{attempt_serial_connect, BackoffController};
use super::events::ScrollOffsets;
use super::input::Button;
use super::lifecycle::{create_shutdown_flag, render_shutdown};
use super::{AppConfig, Logger};
use crate::{
    config::Config,
    display::overlays::{
        advance_offset, line_needs_scroll, render_if_allowed, render_parse_error,
        render_reconnecting,
    },
    lcd::Lcd,
    payload::{Defaults as PayloadDefaults, RenderFrame},
    serial::SerialPort,
    Error, Result,
};

const HEARTBEAT_GRACE_MS: u64 = 5_000;
const HEARTBEAT_BLINK_MS: u64 = 1_000;

/// Drive the main render loop: reads serial, rotates pages, scrolls text, handles reconnects.
pub(super) fn run_render_loop(
    lcd: &mut Lcd,
    config: &mut AppConfig,
    logger: &Logger,
    mut backoff: BackoffController,
    mut port: Option<SerialPort>,
) -> Result<()> {
    let mut state = crate::state::RenderState::new(Some(PayloadDefaults {
        scroll_speed_ms: config.scroll_speed_ms,
        page_timeout_ms: config.page_timeout_ms,
    }));
    let mut buffer = String::new();
    let mut last_render = Instant::now();
    let min_render_interval = Duration::from_millis(200);
    let mut current_frame: Option<RenderFrame> = None;
    let mut next_page = Instant::now();
    let mut next_scroll = Instant::now();
    let mut scroll_offsets = ScrollOffsets::zero();
    let mut button = Button::new(config.button_gpio_pin).ok();
    let mut backlight_state = true;
    let blink_interval = Duration::from_millis(500);
    let mut next_blink = Instant::now();
    let mut reconnect_displayed = port.is_none();
    let mut last_frame_at = Instant::now();
    let heartbeat_grace = Duration::from_millis(HEARTBEAT_GRACE_MS);
    let mut heartbeat_visible = false;
    let mut next_heartbeat = Instant::now() + Duration::from_millis(HEARTBEAT_BLINK_MS);

    if reconnect_displayed {
        render_reconnecting(lcd, config.cols)?;
    }

    let running: Arc<AtomicBool> = create_shutdown_flag()?;

    while running.load(Ordering::SeqCst) {
        let now = Instant::now();
        let heartbeat_active = now.duration_since(last_frame_at) >= heartbeat_grace;
        if heartbeat_active && now >= next_heartbeat {
            heartbeat_visible = !heartbeat_visible;
            next_heartbeat = now + Duration::from_millis(HEARTBEAT_BLINK_MS);
        } else if !heartbeat_active {
            heartbeat_visible = false;
            next_heartbeat = now + Duration::from_millis(HEARTBEAT_BLINK_MS);
        }
        let heartbeat_on = heartbeat_active && heartbeat_visible;

        if let Some(btn) = button.as_mut() {
            if btn.is_pressed() {
                if let Some(frame) = state.next_page() {
                    current_frame = Some(frame);
                    scroll_offsets = ScrollOffsets::zero();
                    next_scroll =
                        now + Duration::from_millis(config.scroll_speed_ms);
                    lcd.clear()?;
                    if let Some(frame) = current_frame.as_ref() {
                        next_page = now + Duration::from_millis(frame.page_timeout_ms);
                        render_if_allowed(
                            lcd,
                            frame,
                            &mut last_render,
                            min_render_interval,
                            (scroll_offsets.top, scroll_offsets.bottom),
                            heartbeat_on,
                        )?;
                    }
                }
            }
        }

        if port.is_none() && !reconnect_displayed {
            render_reconnecting(lcd, config.cols)?;
            reconnect_displayed = true;
        }

        if port.is_none() && backoff.should_retry(now) {
            match attempt_serial_connect(logger, &config.device, config.baud) {
                Some(p) => {
                    port = Some(p);
                    backoff.mark_success(now);
                    reconnect_displayed = false;
                    heartbeat_visible = false;
                }
                None => backoff.mark_failure(now),
            }
        }

        if let Some(port_ref) = port.as_mut() {
            buffer.clear();
            match port_ref.read_message_line(&mut buffer) {
                Ok(read) => {
                    if read > 0 {
                        let line = buffer.trim_end_matches(&['\r', '\n'][..]).trim();
                        if !line.is_empty() {
                            match state.ingest(line) {
                                Ok(Some(frame)) if frame.config_reload => {
                                    logger.log("config reload requested".into());
                                    match Config::load_or_default() {
                                        Ok(new_cfg) => {
                                            config.scroll_speed_ms = new_cfg.scroll_speed_ms;
                                            config.page_timeout_ms = new_cfg.page_timeout_ms;
                                            config.backoff_initial_ms =
                                                new_cfg.backoff_initial_ms;
                                            config.backoff_max_ms = new_cfg.backoff_max_ms;
                                            backoff.update(
                                                config.backoff_initial_ms,
                                                config.backoff_max_ms,
                                            );
                                            state.set_defaults(PayloadDefaults {
                                                scroll_speed_ms: config.scroll_speed_ms,
                                                page_timeout_ms: config.page_timeout_ms,
                                            });
                                            logger.log("config reload applied".into());
                                        }
                                        Err(err) => {
                                            logger.log(format!("config reload failed: {err}"));
                                        }
                                    }
                                }
                                Ok(Some(frame)) => {
                                    current_frame = Some(frame.clone());
                                    scroll_offsets = ScrollOffsets::zero();
                                    next_scroll =
                                        now + Duration::from_millis(config.scroll_speed_ms);
                                    lcd.clear()?;
                                    backlight_state = frame.backlight_on;
                                    lcd.set_backlight(backlight_state)?;
                                    lcd.set_blink(frame.blink)?;
                                    next_blink = now + blink_interval;
                                    last_frame_at = now;
                                    heartbeat_visible = false;
                                    if let Some(frame) = current_frame.as_ref() {
                                        next_page =
                                            now + Duration::from_millis(frame.page_timeout_ms);
                                        render_if_allowed(
                                            lcd,
                                            frame,
                                            &mut last_render,
                                            min_render_interval,
                                            (scroll_offsets.top, scroll_offsets.bottom),
                                            heartbeat_on,
                                        )?;
                                    }
                                }
                                Ok(None) => { /* duplicate */ }
                                Err(err) => {
                                    logger.log(format!("frame error: {err}"));
                                    render_parse_error(lcd, config.cols, &err)?;
                                    backlight_state = true;
                                    next_blink = now + blink_interval;
                                    continue;
                                }
                            }
                        }
                    }
                }
                Err(Error::Io(e)) => {
                    logger.log(format!("serial read error: {e}; scheduling reconnect"));
                    port = None;
                    backoff.mark_failure(now);
                    reconnect_displayed = false;
                }
                Err(err) => return Err(err),
            }
        } else {
            thread::sleep(Duration::from_millis(50));
        }

        if state.len() > 1 && now >= next_page {
            if let Some(frame) = state.next_page() {
                current_frame = Some(frame);
                scroll_offsets = ScrollOffsets::zero();
                if let Some(frame) = current_frame.as_ref() {
                    next_page = now + Duration::from_millis(frame.page_timeout_ms);
                    lcd.clear()?;
                    backlight_state = frame.backlight_on;
                    lcd.set_backlight(backlight_state)?;
                    lcd.set_blink(frame.blink)?;
                    next_blink = now + blink_interval;
                    render_if_allowed(
                        lcd,
                        frame,
                        &mut last_render,
                        min_render_interval,
                        (scroll_offsets.top, scroll_offsets.bottom),
                        heartbeat_on,
                    )?;
                }
            }
        }

        if let Some(frame) = current_frame.as_ref() {
            let width = lcd.cols() as usize;
            let needs_scroll = match frame.bar_row {
                Some(0) => frame.scroll_enabled && line_needs_scroll(&frame.line2, width),
                Some(1) => frame.scroll_enabled && line_needs_scroll(&frame.line1, width),
                _ => frame.scroll_enabled
                    && (line_needs_scroll(&frame.line1, width)
                        || line_needs_scroll(&frame.line2, width)),
            };
            if needs_scroll && now >= next_scroll {
                scroll_offsets = scroll_offsets.update(
                    advance_offset(&frame.line1, lcd.cols() as usize, scroll_offsets.top),
                    advance_offset(&frame.line2, lcd.cols() as usize, scroll_offsets.bottom),
                );
                next_scroll =
                    now + Duration::from_millis(frame.scroll_speed_ms);
                render_if_allowed(
                    lcd,
                    frame,
                    &mut last_render,
                    min_render_interval,
                    (scroll_offsets.top, scroll_offsets.bottom),
                    heartbeat_on,
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

    render_shutdown(lcd)?;
    logger.log("daemon exiting".into());
    Ok(())
}
