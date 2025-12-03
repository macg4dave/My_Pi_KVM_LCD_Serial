use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::{
    thread,
    time::{Duration, Instant},
};

use super::connection::{attempt_serial_connect, BackoffController};
use super::events::ScrollOffsets;
use super::input::Button;
use super::lifecycle::{create_shutdown_flag, render_shutdown};
use super::tunnel::TunnelController;
use super::{AppConfig, LogLevel, Logger};
use crate::{
    config::Config,
    display::overlays::{
        advance_offset, line_needs_scroll, render_if_allowed, render_offline_message,
        render_parse_error, render_reconnecting,
    },
    lcd::Lcd,
    payload::{
        decode_tunnel_frame, encode_tunnel_msg, Defaults as PayloadDefaults, RenderFrame,
        TunnelMsgOwned,
    },
    serial::{
        telemetry::{log_backoff_event, BackoffPhase},
        SerialPort,
    },
    Error, Result,
};
use crc32fast::Hasher;

const HEARTBEAT_GRACE_MS: u64 = 5_000;
const HEARTBEAT_BLINK_MS: u64 = 1_000;

#[derive(Default)]
struct LoopStats {
    frames_accepted: u64,
    frames_rejected: u64,
    checksum_failures: u64,
    duplicates: u64,
    reconnects: u64,
}

fn log_backoff(
    logger: &Logger,
    phase: BackoffPhase,
    attempt: u64,
    delay_ms: u64,
    backoff: &BackoffController,
    config: &AppConfig,
) {
    if let Err(err) = log_backoff_event(
        phase,
        attempt,
        delay_ms,
        backoff.max_delay_ms(),
        &config.device,
        config.baud,
    ) {
        logger.debug(format!("telemetry write failed: {err}"));
    }
}

/// Drive the main render loop: reads serial, rotates pages, scrolls text, handles reconnects.
pub(super) fn run_render_loop(
    lcd: &mut Lcd,
    config: &mut AppConfig,
    logger: &Logger,
    mut backoff: BackoffController,
    mut serial_connection: Option<SerialPort>,
) -> Result<()> {
    let mut state = crate::state::RenderState::new(Some(PayloadDefaults {
        scroll_speed_ms: config.scroll_speed_ms,
        page_timeout_ms: config.page_timeout_ms,
    }));
    let mut incoming_line = String::new();
    let mut last_render = Instant::now();
    let min_render_interval = Duration::from_millis(200);
    let mut current_frame: Option<RenderFrame> = None;
    let mut next_page = Instant::now();
    let mut next_scroll = Instant::now();
    let mut scroll_offsets = ScrollOffsets::zero();
    let mut button_input = Button::new(config.button_gpio_pin).ok();
    let mut backlight_state = true;
    let blink_interval = Duration::from_millis(500);
    let mut next_blink = Instant::now();
    let mut reconnect_displayed = serial_connection.is_none();
    let mut last_frame_at = Instant::now();
    let heartbeat_grace = Duration::from_millis(HEARTBEAT_GRACE_MS);
    let mut heartbeat_visible = false;
    let mut next_heartbeat = Instant::now() + Duration::from_millis(HEARTBEAT_BLINK_MS);
    let mut stats = LoopStats::default();
    let mut offline_displayed = false;
    let mut max_backoff_warned = false;
    let mut tunnel = TunnelController::new(config.command_allowlist.clone())?;

    if reconnect_displayed {
        render_reconnecting(lcd, config.cols)?;
    }

    let running: Arc<AtomicBool> = create_shutdown_flag()?;

    while running.load(Ordering::SeqCst) {
        // Track heartbeat visibility when frames stop arriving for a grace period.
        let current_time = Instant::now();
        if let Some(serial_ref) = serial_connection.as_mut() {
            flush_tunnel_messages(serial_ref, &mut tunnel, logger);
        }
        let heartbeat_active = current_time.duration_since(last_frame_at) >= heartbeat_grace;
        if heartbeat_active && current_time >= next_heartbeat {
            heartbeat_visible = !heartbeat_visible;
            next_heartbeat = current_time + Duration::from_millis(HEARTBEAT_BLINK_MS);
        } else if !heartbeat_active {
            heartbeat_visible = false;
            next_heartbeat = current_time + Duration::from_millis(HEARTBEAT_BLINK_MS);
        }
        let heartbeat_on = heartbeat_active && heartbeat_visible;

        // Manual page advance via GPIO button when configured.
        if let Some(button) = button_input.as_mut() {
            if button.is_pressed() {
                if let Some(frame) = state.next_page() {
                    current_frame = Some(frame);
                    scroll_offsets = ScrollOffsets::zero();
                    next_scroll = current_time + Duration::from_millis(config.scroll_speed_ms);
                    lcd.clear()?;
                    if let Some(frame) = current_frame.as_ref() {
                        next_page = current_time + Duration::from_millis(frame.page_timeout_ms);
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

        // Show reconnect status as soon as we know the serial link is gone.
        if serial_connection.is_none() && !reconnect_displayed {
            render_reconnecting(lcd, config.cols)?;
            reconnect_displayed = true;
        }

        // Attempt reconnect when backoff allows; reset indicators on success.
        if serial_connection.is_none() && backoff.should_retry(current_time) {
            let delay = backoff.current_delay_ms();
            stats.reconnects += 1;
            log_backoff(
                logger,
                BackoffPhase::Attempt,
                stats.reconnects,
                delay,
                &backoff,
                config,
            );
            logger.info(format!(
                "reconnect attempt #{}, delay={}ms device={} baud={}",
                stats.reconnects, delay, config.device, config.baud
            ));
            if delay >= backoff.max_delay_ms() && !max_backoff_warned {
                logger.warn(format!(
                    "backoff saturated at {}ms; staying in cooldown",
                    backoff.max_delay_ms()
                ));
                max_backoff_warned = true;
            }
            match attempt_serial_connect(logger, &config.device, config.serial_options()) {
                Some(p) => {
                    log_backoff(
                        logger,
                        BackoffPhase::Success,
                        stats.reconnects,
                        delay,
                        &backoff,
                        config,
                    );
                    serial_connection = Some(p);
                    backoff.mark_success(current_time);
                    lcd.clear()?;
                    reconnect_displayed = false;
                    offline_displayed = false;
                    heartbeat_visible = false;
                    max_backoff_warned = false;
                }
                None => {
                    log_backoff(
                        logger,
                        BackoffPhase::Failure,
                        stats.reconnects,
                        delay,
                        &backoff,
                        config,
                    );
                    backoff.mark_failure(current_time)
                }
            }
        }

        // Read the next frame from serial; handle config reloads or parse failures.
        if let Some(serial_connection_ref) = serial_connection.as_mut() {
            incoming_line.clear();
            match serial_connection_ref.read_message_line(&mut incoming_line) {
                Ok(read) => {
                    if read > 0 {
                        let line = incoming_line.trim_end_matches(&['\r', '\n'][..]).trim();
                        if !line.is_empty() {
                            if looks_like_tunnel_frame(line) {
                                match decode_tunnel_frame(line) {
                                    Ok(msg) => {
                                        if let Some(response) = tunnel.handle_msg(msg, logger) {
                                            send_tunnel_frame(
                                                serial_connection_ref,
                                                response,
                                                logger,
                                            );
                                        }
                                        flush_tunnel_messages(
                                            serial_connection_ref,
                                            &mut tunnel,
                                            logger,
                                        );
                                    }
                                    Err(err) => {
                                        logger.warn(format!("tunnel frame error: {err}"));
                                        tunnel.log_frame_error(&format!("tunnel: {err}"), line);
                                    }
                                }
                                continue;
                            }
                            let mut hasher = Hasher::new();
                            hasher.update(line.as_bytes());
                            let crc = hasher.finalize();
                            if logger.level() >= LogLevel::Debug {
                                logger.debug(format!("frame crc={crc:08x} len={}", line.len()));
                            }
                            match state.ingest(line) {
                                Ok(Some(frame)) if frame.config_reload => {
                                    stats.frames_accepted += 1;
                                    logger.info("config reload requested");
                                    match Config::load_or_default() {
                                        Ok(new_cfg) => {
                                            let old_device = config.device.clone();
                                            let old_serial = config.serial_options();
                                            let old_scroll = config.scroll_speed_ms;
                                            let old_page = config.page_timeout_ms;

                                            config.scroll_speed_ms = new_cfg.scroll_speed_ms;
                                            config.page_timeout_ms = new_cfg.page_timeout_ms;
                                            config.backoff_initial_ms = new_cfg.backoff_initial_ms;
                                            config.backoff_max_ms = new_cfg.backoff_max_ms;
                                            config.device = new_cfg.device;
                                            config.baud = new_cfg.baud;
                                            config.flow_control = new_cfg.flow_control;
                                            config.parity = new_cfg.parity;
                                            config.stop_bits = new_cfg.stop_bits;
                                            config.dtr_on_open = new_cfg.dtr_on_open;
                                            config.serial_timeout_ms = new_cfg.serial_timeout_ms;

                                            let new_serial = config.serial_options();

                                            if old_device != config.device
                                                || old_serial != new_serial
                                            {
                                                logger.info(format!(
                                                    "config reload updating serial to {} @ {} (flow={}, parity={}, stop_bits={}, dtr={}, timeout={}ms)",
                                                    config.device,
                                                    config.baud,
                                                    config.flow_control,
                                                    config.parity,
                                                    config.stop_bits,
                                                    config.dtr_on_open,
                                                    config.serial_timeout_ms
                                                ));
                                                serial_connection = None;
                                                reconnect_displayed = false;
                                                offline_displayed = false;
                                            }
                                            if old_scroll != new_cfg.scroll_speed_ms
                                                || old_page != new_cfg.page_timeout_ms
                                            {
                                                logger.debug(format!(
                                                    "updated defaults: scroll={}ms page_timeout={}ms",
                                                    config.scroll_speed_ms, config.page_timeout_ms
                                                ));
                                            }
                                            backoff.update(
                                                config.backoff_initial_ms,
                                                config.backoff_max_ms,
                                            );
                                            state.set_defaults(PayloadDefaults {
                                                scroll_speed_ms: config.scroll_speed_ms,
                                                page_timeout_ms: config.page_timeout_ms,
                                            });
                                            logger.info("config reload applied");
                                        }
                                        Err(err) => {
                                            logger.warn(format!("config reload failed: {err}"));
                                        }
                                    }
                                }
                                Ok(Some(frame)) => {
                                    stats.frames_accepted += 1;
                                    current_frame = Some(frame.clone());
                                    scroll_offsets = ScrollOffsets::zero();
                                    next_scroll = current_time
                                        + Duration::from_millis(config.scroll_speed_ms);
                                    lcd.clear()?;
                                    backlight_state = frame.backlight_on;
                                    lcd.set_backlight(backlight_state)?;
                                    lcd.set_blink(frame.blink)?;
                                    next_blink = current_time + blink_interval;
                                    last_frame_at = current_time;
                                    heartbeat_visible = false;
                                    if let Some(frame) = current_frame.as_ref() {
                                        next_page = current_time
                                            + Duration::from_millis(frame.page_timeout_ms);
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
                                Ok(None) => {
                                    stats.duplicates += 1;
                                    logger.debug(format!("duplicate frame ignored crc={crc:08x}"));
                                }
                                Err(err) => {
                                    stats.frames_rejected += 1;
                                    if matches!(err, Error::ChecksumMismatch) {
                                        stats.checksum_failures += 1;
                                    }
                                    logger.warn(format!("frame error: {err}"));
                                    render_parse_error(lcd, config.cols, &err)?;
                                    backlight_state = true;
                                    next_blink = current_time + blink_interval;
                                    continue;
                                }
                            }
                        }
                    }
                }
                Err(Error::Io(e)) => {
                    logger.warn(format!("serial read error: {e}; scheduling reconnect"));
                    serial_connection = None;
                    backoff.mark_failure(current_time);
                    reconnect_displayed = false;
                    if !offline_displayed {
                        render_offline_message(lcd, config.cols)?;
                        offline_displayed = true;
                    }
                }
                Err(err) => return Err(err),
            }
        } else {
            thread::sleep(Duration::from_millis(50));
        }

        // Rotate to the next queued frame after its page timeout.
        if state.len() > 1 && current_time >= next_page {
            if let Some(frame) = state.next_page() {
                current_frame = Some(frame);
                scroll_offsets = ScrollOffsets::zero();
                if let Some(frame) = current_frame.as_ref() {
                    next_page = current_time + Duration::from_millis(frame.page_timeout_ms);
                    lcd.clear()?;
                    backlight_state = frame.backlight_on;
                    lcd.set_backlight(backlight_state)?;
                    lcd.set_blink(frame.blink)?;
                    next_blink = current_time + blink_interval;
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
                _ => {
                    frame.scroll_enabled
                        && (line_needs_scroll(&frame.line1, width)
                            || line_needs_scroll(&frame.line2, width))
                }
            };
            // Scroll long lines forward when allowed by the frame.
            if needs_scroll && current_time >= next_scroll {
                scroll_offsets = scroll_offsets.update(
                    advance_offset(&frame.line1, lcd.cols() as usize, scroll_offsets.top),
                    advance_offset(&frame.line2, lcd.cols() as usize, scroll_offsets.bottom),
                );
                next_scroll = current_time + Duration::from_millis(frame.scroll_speed_ms);
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
                // Drive periodic blink by toggling backlight.
                if current_time >= next_blink {
                    backlight_state = !backlight_state;
                    lcd.set_backlight(backlight_state)?;
                    next_blink = current_time + blink_interval;
                }
            } else if backlight_state != frame.backlight_on {
                backlight_state = frame.backlight_on;
                lcd.set_backlight(backlight_state)?;
            }
        }
    }

    // Leave the display in a clean shutdown state.
    render_shutdown(lcd)?;
    logger.info(format!(
        "shutdown: frames accepted={} rejected={} checksum_failures={} duplicates={} reconnects={}",
        stats.frames_accepted,
        stats.frames_rejected,
        stats.checksum_failures,
        stats.duplicates,
        stats.reconnects
    ));
    logger.info("daemon exiting");
    Ok(())
}

fn looks_like_tunnel_frame(line: &str) -> bool {
    line.contains("\"msg\"") && line.contains("\"crc32\"")
}

fn flush_tunnel_messages(serial: &mut SerialPort, tunnel: &mut TunnelController, logger: &Logger) {
    while let Some(msg) = tunnel.next_outgoing() {
        send_tunnel_frame(serial, msg, logger);
    }
}

fn send_tunnel_frame(serial: &mut SerialPort, msg: TunnelMsgOwned, logger: &Logger) {
    match encode_tunnel_msg(&msg) {
        Ok(encoded) => {
            if let Err(err) = serial.send_command_line(&encoded) {
                logger.warn(format!("tunnel send failed: {err}"));
            }
        }
        Err(err) => {
            logger.warn(format!("tunnel encode failed: {err}"));
        }
    }
}
