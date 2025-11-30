use crate::{
    cli::RunOptions,
    config::{Config, DEFAULT_BAUD, DEFAULT_COLS, DEFAULT_DEVICE, DEFAULT_ROWS},
    config::Pcf8574Addr,
    lcd::{Lcd, BAR_LEVELS, HEARTBEAT_CHAR},
    payload::Defaults as PayloadDefaults,
    payload::RenderFrame,
    serial::SerialPort,
    Error, Result,
};
use std::{fs, io::Write, thread, time::{Duration, Instant}};
use std::sync::{Arc, atomic::{AtomicBool, Ordering}};

const SCROLL_GAP: &str = "    |    ";
const HEARTBEAT_GRACE_MS: u64 = 5_000;
const HEARTBEAT_BLINK_MS: u64 = 1_000;
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
    pub backoff_initial_ms: u64,
    pub backoff_max_ms: u64,
    pub pcf8574_addr: Pcf8574Addr,
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
            backoff_initial_ms: crate::config::DEFAULT_BACKOFF_INITIAL_MS,
            backoff_max_ms: crate::config::DEFAULT_BACKOFF_MAX_MS,
            pcf8574_addr: crate::config::DEFAULT_PCF8574_ADDR,
        }
    }
}

pub struct App {
    config: AppConfig,
    logger: Logger,
}

impl App {
    pub fn new(config: AppConfig) -> Self {
        Self {
            config,
            logger: Logger::new(),
        }
    }

    pub fn from_options(opts: RunOptions) -> Result<Self> {
        let cfg_file = Config::load_or_default()?;
        let merged = AppConfig::from_sources(cfg_file, opts);
        Ok(Self::new(merged))
    }

    /// Entry point for the daemon. Wire up serial + LCD here.
    pub fn run(&self) -> Result<()> {
        let mut config = self.config.clone();
        let mut lcd = Lcd::new(
            config.cols,
            config.rows,
            config.pcf8574_addr.clone(),
        )?;
        lcd.render_boot_message()?;
        self.logger.log(format!(
            "daemon start (device={}, baud={}, cols={}, rows={})",
            config.device, config.baud, config.cols, config.rows
        ));

        let mut backoff =
            Duration::from_millis(config.backoff_initial_ms.max(1));
        let mut max_backoff = Duration::from_millis(
            config
                .backoff_max_ms
                .max(config.backoff_initial_ms.max(1)),
        );
        let mut next_retry = Instant::now();

        if let Some(path) = &config.payload_file {
            let defaults = PayloadDefaults {
                scroll_speed_ms: config.scroll_speed_ms,
                page_timeout_ms: config.page_timeout_ms,
            };
            let frame = load_payload_from_file(path, defaults)?;
            lcd.set_backlight(frame.backlight_on)?;
            lcd.set_blink(frame.blink)?;
            return render_frame(&mut lcd, &frame);
        }

        let mut port: Option<SerialPort> =
            match SerialPort::connect(&config.device, config.baud) {
                Ok(mut p) => {
                    if let Err(err) = p.send_line("INIT") {
                        self.logger.log(format!("serial init failed: {err}; will retry"));
                        None
                    } else {
                        self.logger.log("serial connected".into());
                        Some(p)
                    }
                }
                Err(err) => {
                    self.logger
                        .log(format!("serial connect failed: {err}; will retry"));
                    None
                }
            };
        if port.is_none() {
            next_retry = Instant::now() + backoff;
            render_reconnecting(&mut lcd, config.cols)?;
        }

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
        let mut scroll_offsets = (0usize, 0usize);
        let mut button = Button::new(config.button_gpio_pin).ok();
        let mut backlight_state = true;
        let blink_interval = Duration::from_millis(500);
        let mut next_blink = Instant::now();
        let mut reconnect_displayed = port.is_none();
        let mut last_frame_at = Instant::now();
        let heartbeat_grace = Duration::from_millis(HEARTBEAT_GRACE_MS);
        let mut heartbeat_visible = false;
        let mut next_heartbeat = Instant::now() + Duration::from_millis(HEARTBEAT_BLINK_MS);

        let running = Arc::new(AtomicBool::new(true));
        {
            let running = running.clone();
            ctrlc::set_handler(move || {
                running.store(false, Ordering::SeqCst);
            })
            .map_err(|e| Error::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))?;
        }

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
                        scroll_offsets = (0, 0);
                        next_scroll =
                            now + Duration::from_millis(config.scroll_speed_ms);
                        lcd.clear()?;
                        if let Some(frame) = current_frame.as_ref() {
                            next_page = now + Duration::from_millis(frame.page_timeout_ms);
                            render_if_allowed(
                                &mut lcd,
                                frame,
                                &mut last_render,
                                min_render_interval,
                                scroll_offsets,
                                heartbeat_on,
                            )?;
                        }
                    }
                }
            }

            if port.is_none() && !reconnect_displayed {
                render_reconnecting(&mut lcd, config.cols)?;
                reconnect_displayed = true;
            }

            if port.is_none() && now >= next_retry {
                match SerialPort::connect(&config.device, config.baud) {
                    Ok(mut p) => {
                        if let Err(err) = p.send_line("INIT") {
                            self.logger.log(format!("serial init failed: {err}; will retry"));
                            next_retry = now + backoff;
                            backoff = (backoff * 2).min(max_backoff);
                        } else {
                            port = Some(p);
                            backoff = Duration::from_millis(
                                config.backoff_initial_ms.max(1)
                            );
                            reconnect_displayed = false;
                            heartbeat_visible = false;
                            self.logger.log("serial connected".into());
                        }
                    }
                    Err(err) => {
                        self.logger.log(format!("serial reconnect failed: {err}; will retry"));
                        next_retry = now + backoff;
                        backoff = (backoff * 2).min(max_backoff);
                    }
                }
            }

            if let Some(port_ref) = port.as_mut() {
                buffer.clear();
                match port_ref.read_line(&mut buffer) {
                    Ok(read) => {
                        if read > 0 {
                            let line = buffer.trim_end_matches(&['\r', '\n'][..]).trim();
                            if !line.is_empty() {
                                match state.ingest(line) {
                                    Ok(Some(frame)) if frame.config_reload => {
                                        self.logger.log("config reload requested".into());
                                        match Config::load_or_default() {
                                            Ok(new_cfg) => {
                                                config.scroll_speed_ms = new_cfg.scroll_speed_ms;
                                                config.page_timeout_ms = new_cfg.page_timeout_ms;
                                                config.backoff_initial_ms =
                                                    new_cfg.backoff_initial_ms;
                                                config.backoff_max_ms = new_cfg.backoff_max_ms;
                                                backoff = Duration::from_millis(
                                                    config.backoff_initial_ms.max(1),
                                                );
                                                max_backoff = Duration::from_millis(
                                                    config
                                                        .backoff_max_ms
                                                        .max(config.backoff_initial_ms.max(1)),
                                                );
                                                state.set_defaults(PayloadDefaults {
                                                    scroll_speed_ms: config.scroll_speed_ms,
                                                    page_timeout_ms: config.page_timeout_ms,
                                                });
                                                self.logger.log("config reload applied".into());
                                            }
                                            Err(err) => {
                                                self.logger
                                                    .log(format!("config reload failed: {err}"));
                                            }
                                        }
                                    }
                                    Ok(Some(frame)) => {
                                        current_frame = Some(frame.clone());
                                        scroll_offsets = (0, 0);
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
                                                &mut lcd,
                                                frame,
                                                &mut last_render,
                                                min_render_interval,
                                                scroll_offsets,
                                                heartbeat_on,
                                            )?;
                                        }
                                    }
                                    Ok(None) => { /* duplicate */ }
                                    Err(err) => {
                                        self.logger.log(format!("frame error: {err}"));
                                        render_parse_error(&mut lcd, config.cols, &err)?;
                                        backlight_state = true;
                                        next_blink = now + blink_interval;
                                        continue;
                                    }
                                }
                            }
                        }
                    }
                    Err(Error::Io(e)) => {
                        self.logger.log(format!("serial read error: {e}; scheduling reconnect"));
                        port = None;
                        next_retry = now + backoff;
                        backoff = (backoff * 2).min(max_backoff);
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

        render_shutdown(&mut lcd)?;
        self.logger.log("daemon exiting".into());
        Ok(())
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
            backoff_initial_ms: opts
                .backoff_initial_ms
                .unwrap_or(config.backoff_initial_ms),
            backoff_max_ms: opts.backoff_max_ms.unwrap_or(config.backoff_max_ms),
            pcf8574_addr: opts
                .pcf8574_addr
                .unwrap_or_else(|| config.pcf8574_addr.clone()),
        }
    }
}

fn render_frame(lcd: &mut Lcd, frame: &RenderFrame) -> Result<()> {
    render_frame_with_scroll(lcd, frame, (0, 0), false)
}

fn load_payload_from_file(path: &str, defaults: PayloadDefaults) -> Result<RenderFrame> {
    let raw = fs::read_to_string(path)?;
    RenderFrame::from_payload_json_with_defaults(&raw, defaults)
}

fn render_bar(percent: u8, width: usize) -> String {
    if width == 0 {
        return String::new();
    }

    let max_level = (BAR_LEVELS.len() - 1) as usize;
    let total_units = width * max_level;
    let filled_units = (percent as usize * total_units) / 100;
    let mut s = String::with_capacity(width);
    for col in 0..width {
        let remaining = filled_units.saturating_sub(col * max_level);
        let level = remaining.min(max_level);
        s.push(BAR_LEVELS[level]);
    }
    s
}

fn render_if_allowed(
    lcd: &mut Lcd,
    frame: &RenderFrame,
    last_render: &mut Instant,
    min_interval: Duration,
    scroll_offsets: (usize, usize),
    heartbeat_on: bool,
) -> Result<()> {
    let now = Instant::now();
    if now.duration_since(*last_render) < min_interval {
        return Ok(());
    }
    *last_render = now;
    render_frame_with_scroll(lcd, frame, scroll_offsets, heartbeat_on)
}

fn render_frame_with_scroll(
    lcd: &mut Lcd,
    frame: &RenderFrame,
    offsets: (usize, usize),
    heartbeat_on: bool,
) -> Result<()> {
    lcd.set_blink(frame.blink)?;

    if frame.clear {
        lcd.clear()?;
    }

    let width = lcd.cols() as usize;
    let bar_row = frame.bar_row;
    let mut line1 = if bar_row == Some(0) && frame.bar_percent.is_some() {
        render_bar(frame.bar_percent.unwrap(), width)
    } else {
        view_line(&frame.line1, width, offsets.0, frame.scroll_enabled)
    };
    let mut line2 = if bar_row == Some(1) && frame.bar_percent.is_some() {
        render_bar(frame.bar_percent.unwrap(), width)
    } else {
        view_line(&frame.line2, width, offsets.1, frame.scroll_enabled)
    };

    if heartbeat_on && width > 0 {
        if bar_row == Some(0) {
            overlay_heartbeat(&mut line2, width);
        } else {
            overlay_heartbeat(&mut line1, width);
        }
    }

    overlay_icons(&mut line1, &mut line2, width, &frame.icons, bar_row);

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

    lcd.write_lines(out1, out2)
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

fn truncate_to_width(text: &str, width: usize) -> String {
    text.chars().take(width).collect()
}

fn view_line(text: &str, width: usize, offset: usize, scroll_enabled: bool) -> String {
    if scroll_enabled {
        return view_with_scroll(text, width, offset);
    }
    truncate_to_width(text, width)
}

fn overlay_heartbeat(text: &mut String, width: usize) {
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
        *last = HEARTBEAT_CHAR;
    }
    *text = chars.into_iter().collect();
}

fn overlay_icons(
    line1: &mut String,
    line2: &mut String,
    width: usize,
    icons: &[crate::payload::Icon],
    bar_row: Option<u8>,
) {
    if icons.is_empty() || width == 0 {
        return;
    }
    let target = if bar_row == Some(1) { line1 } else { line2 };
    let icon_char = match icons[0] {
        crate::payload::Icon::Battery => crate::lcd::BATTERY_CHAR,
        crate::payload::Icon::Arrow => '>',
        crate::payload::Icon::Heart => HEARTBEAT_CHAR,
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

fn render_parse_error(lcd: &mut Lcd, cols: u8, err: &Error) -> Result<()> {
    let width = cols as usize;
    let mut msg = format!("{err}");
    if msg.chars().count() > width {
        msg = msg.chars().take(width).collect();
    }
    lcd.set_backlight(true)?;
    lcd.set_blink(true)?;
    lcd.write_line(0, "ERR PARSE")?;
    lcd.write_line(1, &msg)?;
    Ok(())
}

fn render_reconnecting(lcd: &mut Lcd, cols: u8) -> Result<()> {
    let width = cols as usize;
    let title: String = "RECONNECTING".chars().take(width).collect();
    let mut detail = "retrying...".to_string();
    if detail.chars().count() > width {
        detail = detail.chars().take(width).collect();
    }
    lcd.clear()?;
    lcd.set_backlight(true)?;
    lcd.set_blink(false)?;
    lcd.write_line(0, &title)?;
    lcd.write_line(1, &detail)?;
    Ok(())
}

fn render_shutdown(lcd: &mut Lcd) -> Result<()> {
    lcd.clear()?;
    lcd.set_blink(false)?;
    lcd.write_line(0, "offline")?;
    lcd.write_line(1, "")?;
    Ok(())
}

struct Logger {
    file: Option<std::fs::File>,
}

impl Logger {
    fn new() -> Self {
        let path = std::env::var("SERIALLCD_LOG_PATH").ok();
        let file = path.and_then(|p| {
            fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(p)
                .ok()
        });
        Self { file }
    }

    fn log(&self, msg: String) {
        eprintln!("{msg}");
        if let Some(file) = self.file.as_ref() {
            if let Ok(mut clone) = file.try_clone() {
                let _ = writeln!(clone, "{msg}");
            }
        }
    }
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
            backoff_initial_ms: None,
            backoff_max_ms: None,
            pcf8574_addr: None,
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
            backoff_initial_ms: crate::config::DEFAULT_BACKOFF_INITIAL_MS,
            backoff_max_ms: crate::config::DEFAULT_BACKOFF_MAX_MS,
            pcf8574_addr: crate::config::DEFAULT_PCF8574_ADDR,
        };
        let opts = RunOptions::default();
        let merged = AppConfig::from_sources(cfg_file.clone(), opts);
        assert_eq!(merged.device, cfg_file.device);
        assert_eq!(merged.baud, cfg_file.baud);
        assert_eq!(merged.cols, cfg_file.cols);
        assert_eq!(merged.rows, cfg_file.rows);
        assert_eq!(merged.backoff_initial_ms, cfg_file.backoff_initial_ms);
        assert_eq!(merged.backoff_max_ms, cfg_file.backoff_max_ms);
        assert_eq!(merged.pcf8574_addr, cfg_file.pcf8574_addr);
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
