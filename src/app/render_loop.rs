use std::fs::{self, OpenOptions};
use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::{
    io::Write,
    thread,
    time::{Duration, Instant},
};

use super::connection::attempt_serial_connect;
use super::events::{CommandBridge, CommandExecutor, ScrollOffsets};
use super::input::Button;
use super::lifecycle::{create_shutdown_flag, render_shutdown};
use super::negotiation::NegotiationLog;
use super::polling::{start_polling, PollEvent, PollSnapshot, PollingHandle};
use super::tunnel::TunnelController;
use super::{AppConfig, LogLevel, Logger};
use crate::{
    config::Config,
    display::{
        icon_bank::{IconBank, IconPalette},
        overlays::{
            advance_offset, line_needs_scroll, render_if_allowed, render_offline_message,
            render_parse_error, render_reconnecting,
        },
    },
    lcd::Lcd,
    payload::{
        decode_tunnel_frame, encode_command_frame, encode_tunnel_msg, CommandMessage,
        Defaults as PayloadDefaults, RenderFrame, TunnelMsgOwned,
    },
    serial::{
        backoff::BackoffController,
        classify_io_error,
        telemetry::{log_backoff_event, BackoffPhase},
        SerialFailureKind, SerialPort,
    },
    Error, Result, CACHE_DIR,
};
use crc32fast::Hasher;

const HEARTBEAT_GRACE_MS: u64 = 5_000;
const HEARTBEAT_BLINK_MS: u64 = 1_000;
const POLLING_OVERLAY_MIN_INTERVAL_MS: u64 = 1_500;
const PROTOCOL_ERROR_LOG_MAX_BYTES: u64 = 256 * 1024;

struct PollingState {
    handle: PollingHandle,
    latest: Option<PollSnapshot>,
    latest_seq: u64,
    last_rendered_seq: u64,
    last_overlay_at: Instant,
    log: PollingLog,
}

impl PollingState {
    fn new(handle: PollingHandle) -> Self {
        Self {
            handle,
            latest: None,
            latest_seq: 0,
            last_rendered_seq: 0,
            last_overlay_at: Instant::now(),
            log: PollingLog::new(),
        }
    }

    fn record_snapshot(&mut self, snapshot: PollSnapshot, logger: &Logger) {
        self.latest_seq = self.latest_seq.wrapping_add(1);
        if let Err(err) = self.log.snapshot(self.latest_seq, &snapshot) {
            logger.debug(format!("polling log append failed: {err}"));
        }
        self.latest = Some(snapshot);
    }

    fn record_error(&self, err: &str, logger: &Logger) {
        if let Err(write_err) = self.log.error(err) {
            logger.debug(format!("polling log error append failed: {write_err}"));
        }
    }
}

struct PollingLog {
    path: PathBuf,
}

impl PollingLog {
    fn new() -> Self {
        let path = PathBuf::from(CACHE_DIR).join("polling").join("events.log");
        Self { path }
    }

    fn snapshot(&self, seq: u64, snapshot: &PollSnapshot) -> std::io::Result<()> {
        let mut line = format!(
            "seq={seq} cpu={:.1} mem_used_kb={} mem_total_kb={} disk_used_pct={:.1}",
            snapshot.cpu_percent,
            snapshot.mem_used_kb,
            snapshot.mem_total_kb,
            snapshot.disk_used_pct
        );
        if let Some(available) = snapshot.disk_available_kb {
            line.push_str(&format!(" disk_available_kb={available}"));
        }
        if let Some(temp) = snapshot.temperature_c {
            line.push_str(&format!(" temp_c={temp:.1}"));
        }
        line.push_str(" kind=snapshot");
        self.append_line(&line)
    }

    fn error(&self, err: &str) -> std::io::Result<()> {
        let line = format!("kind=error message={err}");
        self.append_line(&line)
    }

    fn append_line(&self, line: &str) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        writeln!(file, "{line}")
    }
}

struct ProtocolErrorLog {
    path: PathBuf,
}

impl ProtocolErrorLog {
    fn new() -> Self {
        let path = PathBuf::from(CACHE_DIR).join("protocol_errors.log");
        Self { path }
    }

    fn log(&self, err: &Error, payload: &str, logger: &Logger) {
        if let Err(write_err) = self.append(err, payload) {
            logger.debug(format!("protocol error log write failed: {write_err}"));
        }
    }

    fn append(&self, err: &Error, payload: &str) -> std::io::Result<()> {
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        if let Ok(meta) = fs::metadata(&self.path) {
            if meta.len() >= PROTOCOL_ERROR_LOG_MAX_BYTES {
                let _ = fs::remove_file(&self.path);
            }
        }
        let mut file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)?;
        writeln!(
            file,
            "error={} payload={}",
            err,
            payload.replace('\n', "\\n")
        )
    }
}

#[derive(Default)]
struct LoopStats {
    frames_accepted: u64,
    frames_rejected: u64,
    checksum_failures: u64,
    duplicates: u64,
    reconnects: u64,
}

fn log_icon_fallbacks(logger: &Logger, palette: Option<IconPalette>) {
    let Some(palette) = palette else {
        return;
    };
    if palette.missing_icons.is_empty() {
        return;
    }
    let joined = palette
        .missing_icons
        .iter()
        .map(|icon| format!("{icon:?}"))
        .collect::<Vec<_>>()
        .join(", ");
    logger.debug(format!(
        "icon bank saturated; falling back to ASCII for [{joined}]"
    ));
}

fn log_backoff(
    logger: &Logger,
    phase: BackoffPhase,
    attempt: u64,
    delay_ms: u64,
    backoff: &BackoffController,
    config: &AppConfig,
    reason: Option<SerialFailureKind>,
) {
    let reason_label = reason.map(|r| r.as_str());
    if let Err(err) = log_backoff_event(
        phase,
        attempt,
        delay_ms,
        backoff.max_delay_ms(),
        &config.device,
        config.baud,
        reason_label,
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
    initial_disconnect_reason: Option<SerialFailureKind>,
    negotiation_log: &mut NegotiationLog,
) -> Result<()> {
    let mut state = crate::state::RenderState::new(Some(PayloadDefaults {
        scroll_speed_ms: config.scroll_speed_ms,
        page_timeout_ms: config.page_timeout_ms,
    }));
    let mut icon_bank = IconBank::new();
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
    let mut last_disconnect_reason = initial_disconnect_reason;
    let mut tunnel = TunnelController::new(config.command_allowlist.clone())?;
    let mut command_bridge = CommandBridge::new();
    let mut command_executor = CommandExecutor::new(config.command_allowlist.clone());
    let protocol_errors = ProtocolErrorLog::new();

    if reconnect_displayed {
        render_reconnecting(lcd, config.cols)?;
    }

    let running: Arc<AtomicBool> = create_shutdown_flag()?;
    let mut polling = if config.polling_enabled {
        Some(PollingState::new(start_polling(
            config.poll_interval_ms,
            running.clone(),
        )))
    } else {
        None
    };

    while running.load(Ordering::SeqCst) {
        if let Some(polling_state) = polling.as_mut() {
            while let Ok(event) = polling_state.handle.receiver().try_recv() {
                match event {
                    PollEvent::Snapshot(snapshot) => {
                        polling_state.record_snapshot(snapshot, logger);
                    }
                    PollEvent::Error(err) => {
                        logger.warn(format!("polling error: {err}"));
                        polling_state.record_error(&err, logger);
                    }
                }
            }
        }

        // Track heartbeat visibility when frames stop arriving for a grace period.
        let current_time = Instant::now();
        if let Some(serial_ref) = serial_connection.as_mut() {
            flush_tunnel_messages(serial_ref, &mut tunnel, logger);
            flush_command_messages(serial_ref, &mut command_executor, logger);
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
                        let palette = render_if_allowed(
                            lcd,
                            frame,
                            &mut last_render,
                            min_render_interval,
                            (scroll_offsets.top, scroll_offsets.bottom),
                            heartbeat_on,
                            &mut icon_bank,
                        )?;
                        log_icon_fallbacks(logger, palette);
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
                last_disconnect_reason,
            );
            let reason_suffix = last_disconnect_reason
                .map(|r| format!(" last_failure={r}"))
                .unwrap_or_default();
            logger.info(format!(
                "reconnect attempt #{}, delay={}ms device={} baud={}{}",
                stats.reconnects, delay, config.device, config.baud, reason_suffix
            ));
            if delay >= backoff.max_delay_ms() && !max_backoff_warned {
                logger.warn(format!(
                    "backoff saturated at {}ms; staying in cooldown",
                    backoff.max_delay_ms()
                ));
                max_backoff_warned = true;
            }
            match attempt_serial_connect(
                logger,
                &config.device,
                config.serial_options(),
                &config.negotiation,
                negotiation_log,
            ) {
                Ok(p) => {
                    log_backoff(
                        logger,
                        BackoffPhase::Success,
                        stats.reconnects,
                        delay,
                        &backoff,
                        config,
                        None,
                    );
                    serial_connection = Some(p);
                    backoff.mark_success(current_time);
                    lcd.clear()?;
                    reconnect_displayed = false;
                    offline_displayed = false;
                    heartbeat_visible = false;
                    max_backoff_warned = false;
                    last_disconnect_reason = None;
                }
                Err(reason) => {
                    log_backoff(
                        logger,
                        BackoffPhase::Failure,
                        stats.reconnects,
                        delay,
                        &backoff,
                        config,
                        Some(reason),
                    );
                    backoff.mark_failure(current_time);
                    last_disconnect_reason = Some(reason);
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
                            if looks_like_command_frame(line) {
                                match command_bridge.ingest_line(line) {
                                    Ok(Some(event)) => {
                                        let label =
                                            if let Some(id) = command_bridge.last_request_id() {
                                                format!("cmd#{id} {}", event.kind())
                                            } else {
                                                event.kind().to_string()
                                            };
                                        logger.debug(format!(
                                            "command frame buffered ({label}), awaiting executor"
                                        ));
                                        if let Some(response) = command_executor.handle_event(event)
                                        {
                                            send_command_frame(
                                                serial_connection_ref,
                                                response,
                                                logger,
                                            );
                                            flush_command_messages(
                                                serial_connection_ref,
                                                &mut command_executor,
                                                logger,
                                            );
                                        }
                                    }
                                    Ok(None) => {}
                                    Err(err) => {
                                        logger.warn(format!("command frame error: {err}"));
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
                                        let palette = render_if_allowed(
                                            lcd,
                                            frame,
                                            &mut last_render,
                                            min_render_interval,
                                            (scroll_offsets.top, scroll_offsets.bottom),
                                            heartbeat_on,
                                            &mut icon_bank,
                                        )?;
                                        log_icon_fallbacks(logger, palette);
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
                                    if matches!(err, Error::Parse(_)) {
                                        protocol_errors.log(&err, line, logger);
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
                    let reason = classify_io_error(&e);
                    logger.warn(format!(
                        "serial read error [{reason}]: {e}; scheduling reconnect"
                    ));
                    serial_connection = None;
                    backoff.mark_failure(current_time);
                    reconnect_displayed = false;
                    last_disconnect_reason = Some(reason);
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
                    let palette = render_if_allowed(
                        lcd,
                        frame,
                        &mut last_render,
                        min_render_interval,
                        (scroll_offsets.top, scroll_offsets.bottom),
                        heartbeat_on,
                        &mut icon_bank,
                    )?;
                    log_icon_fallbacks(logger, palette);
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
                let palette = render_if_allowed(
                    lcd,
                    frame,
                    &mut last_render,
                    min_render_interval,
                    (scroll_offsets.top, scroll_offsets.bottom),
                    heartbeat_on,
                    &mut icon_bank,
                )?;
                log_icon_fallbacks(logger, palette);
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

        let no_frames_available = state.is_empty();
        if let Some(polling_state) = polling.as_mut() {
            maybe_render_polling_overlay(
                polling_state,
                lcd,
                config.cols,
                serial_connection.is_some(),
                current_frame.is_some(),
                no_frames_available,
            )?;
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

fn looks_like_command_frame(line: &str) -> bool {
    line.contains("\"channel\":\"command\"") && line.contains("\"crc32\"")
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

fn flush_command_messages(
    serial: &mut SerialPort,
    executor: &mut CommandExecutor,
    logger: &Logger,
) {
    while let Some(msg) = executor.next_outgoing() {
        send_command_frame(serial, msg, logger);
    }
}

fn send_command_frame(serial: &mut SerialPort, msg: CommandMessage, logger: &Logger) {
    match encode_command_frame(&msg) {
        Ok(encoded) => {
            if let Err(err) = serial.send_command_line(&encoded) {
                logger.warn(format!("command send failed: {err}"));
            }
        }
        Err(err) => {
            logger.warn(format!("command encode failed: {err}"));
        }
    }
}

fn maybe_render_polling_overlay(
    polling: &mut PollingState,
    lcd: &mut Lcd,
    cols: u8,
    serial_active: bool,
    has_frame: bool,
    no_frames_available: bool,
) -> Result<()> {
    if polling.latest.is_none() {
        return Ok(());
    }
    let should_render = if serial_active {
        no_frames_available && !has_frame
    } else {
        true
    };
    if !should_render {
        return Ok(());
    }
    let now = Instant::now();
    let overlay_interval = Duration::from_millis(POLLING_OVERLAY_MIN_INTERVAL_MS);
    if polling.last_rendered_seq == polling.latest_seq
        && now.duration_since(polling.last_overlay_at) < overlay_interval
    {
        return Ok(());
    }
    let snapshot = polling.latest.as_ref().unwrap();
    render_polling_overlay(lcd, cols, snapshot, serial_active)?;
    polling.last_rendered_seq = polling.latest_seq;
    polling.last_overlay_at = now;
    Ok(())
}

fn render_polling_overlay(
    lcd: &mut Lcd,
    cols: u8,
    snapshot: &PollSnapshot,
    serial_active: bool,
) -> Result<()> {
    let width = cols as usize;
    let (line1, line2) = format_polling_lines(snapshot, width, serial_active);
    lcd.clear()?;
    lcd.set_backlight(true)?;
    lcd.set_blink(false)?;
    lcd.write_line(0, &line1)?;
    lcd.write_line(1, &line2)?;
    Ok(())
}

fn format_polling_lines(
    snapshot: &PollSnapshot,
    width: usize,
    serial_active: bool,
) -> (String, String) {
    let cpu = snapshot.cpu_percent.round() as i32;
    let mem_pct = if snapshot.mem_total_kb > 0 {
        ((snapshot.mem_used_kb as f64 / snapshot.mem_total_kb as f64) * 100.0).round() as i32
    } else {
        0
    };
    let disk = snapshot.disk_used_pct.round() as i32;
    let free_mb = snapshot
        .disk_available_kb
        .map(|kb| (kb / 1024) as u64)
        .map(|mb| format!("{mb}M"))
        .unwrap_or_else(|| "--".into());
    let temp = snapshot
        .temperature_c
        .map(|c| format!("{:.0}C", c))
        .unwrap_or_else(|| "--".into());
    let prefix = if serial_active { "" } else { "RC " };
    let line1 = fit_line(format!("{prefix}CPU{cpu:>3}% MEM{mem_pct:>3}%"), width);
    let line2 = fit_line(
        format!("DSK{disk:>3}% TMP{temp:>4} FREE{free_mb:>4}"),
        width,
    );
    (line1, line2)
}

fn fit_line(text: String, width: usize) -> String {
    if width == 0 {
        return String::new();
    }
    let mut chars: Vec<char> = text.chars().collect();
    if chars.len() > width {
        chars.truncate(width);
        return chars.into_iter().collect();
    }
    if chars.len() < width {
        let mut padded = text;
        padded.push_str(&" ".repeat(width - chars.len()));
        return padded;
    }
    text
}
