use crate::{
    cli::{RunMode, RunOptions},
    config::Pcf8574Addr,
    config::{
        Config, DisplayDriver, DEFAULT_BAUD, DEFAULT_COLS, DEFAULT_DEVICE, DEFAULT_ROWS,
        DEFAULT_SERIAL_TIMEOUT_MS,
    },
    lcd::Lcd,
    payload::{Defaults as PayloadDefaults, RenderFrame},
    serial::{DtrBehavior, FlowControlMode, ParityMode, SerialOptions, StopBitsMode},
    Result,
};
use std::{fs, str::FromStr, time::Instant};

mod compression;
mod connection;
mod demo;
mod events;
mod file_transfer;
mod input;
mod lifecycle;
mod logger;
mod negotiation;
mod polling;
mod render_loop;
#[cfg(feature = "serialsh-preview")]
pub mod serial_shell;
mod telemetry;
mod tunnel;
mod watchdog;

use crate::display::overlays::{render_frame_once, render_reconnecting};
use crate::serial::backoff::BackoffController;
use connection::attempt_serial_connect;
use demo::run_demo;
pub(crate) use logger::{LogLevel, Logger};
use render_loop::run_render_loop;

/// Config for the daemon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    pub device: String,
    pub baud: u32,
    pub flow_control: FlowControlMode,
    pub parity: ParityMode,
    pub stop_bits: StopBitsMode,
    pub dtr_on_open: DtrBehavior,
    pub serial_timeout_ms: u64,
    pub cols: u8,
    pub rows: u8,
    pub scroll_speed_ms: u64,
    pub page_timeout_ms: u64,
    pub button_gpio_pin: Option<u8>,
    pub payload_file: Option<String>,
    pub backoff_initial_ms: u64,
    pub backoff_max_ms: u64,
    pub pcf8574_addr: Pcf8574Addr,
    pub display_driver: DisplayDriver,
    pub log_level: LogLevel,
    pub log_file: Option<String>,
    pub demo: bool,
    pub command_allowlist: Vec<String>,
    pub serialsh: bool,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            device: DEFAULT_DEVICE.to_string(),
            baud: DEFAULT_BAUD,
            flow_control: FlowControlMode::default(),
            parity: ParityMode::default(),
            stop_bits: StopBitsMode::default(),
            dtr_on_open: DtrBehavior::default(),
            serial_timeout_ms: DEFAULT_SERIAL_TIMEOUT_MS,
            cols: DEFAULT_COLS,
            rows: DEFAULT_ROWS,
            scroll_speed_ms: crate::payload::DEFAULT_SCROLL_MS,
            page_timeout_ms: crate::payload::DEFAULT_PAGE_TIMEOUT_MS,
            button_gpio_pin: None,
            payload_file: None,
            backoff_initial_ms: crate::config::DEFAULT_BACKOFF_INITIAL_MS,
            backoff_max_ms: crate::config::DEFAULT_BACKOFF_MAX_MS,
            pcf8574_addr: crate::config::DEFAULT_PCF8574_ADDR,
            display_driver: crate::config::DEFAULT_DISPLAY_DRIVER,
            log_level: LogLevel::default(),
            log_file: None,
            demo: false,
            command_allowlist: Vec::new(),
            serialsh: false,
        }
    }
}

pub struct App {
    config: AppConfig,
    logger: Logger,
}

impl App {
    pub fn new(config: AppConfig) -> Result<Self> {
        let logger = Logger::new(config.log_level, config.log_file.clone())?;
        Ok(Self { config, logger })
    }

    pub fn from_options(opts: RunOptions) -> Result<Self> {
        let cfg_file = Config::load_or_default()?;
        let merged = AppConfig::from_sources(cfg_file, opts);
        Self::new(merged)
    }

    /// Entry point for the daemon. Wire up serial + LCD here.
    pub fn run(&self) -> Result<()> {
        let mut config = self.config.clone();

        let mut lcd = Lcd::new(
            config.cols,
            config.rows,
            config.pcf8574_addr.clone(),
            config.display_driver,
        )?;
        lcd.render_boot_message()?;
        self.logger.info(format!(
            "daemon start (device={}, baud={}, cols={}, rows={})",
            config.device, config.baud, config.cols, config.rows
        ));

        if config.demo {
            self.logger
                .info("demo mode enabled: cycling built-in pages");
            return run_demo(&mut lcd, &mut config, &self.logger);
        }

        let mut backoff = BackoffController::new(config.backoff_initial_ms, config.backoff_max_ms);

        if let Some(path) = &config.payload_file {
            let defaults = PayloadDefaults {
                scroll_speed_ms: config.scroll_speed_ms,
                page_timeout_ms: config.page_timeout_ms,
            };
            let frame = load_payload_from_file(path, defaults)?;
            lcd.set_backlight(frame.backlight_on)?;
            lcd.set_blink(frame.blink)?;
            return render_frame_once(&mut lcd, &frame);
        }

        let (serial_connection, initial_disconnect_reason) =
            match attempt_serial_connect(&self.logger, &config.device, config.serial_options()) {
                Ok(port) => (Some(port), None),
                Err(reason) => (None, Some(reason)),
            };
        if serial_connection.is_none() {
            let now = Instant::now();
            backoff.mark_failure(now);
            render_reconnecting(&mut lcd, config.cols)?;
        }

        run_render_loop(
            &mut lcd,
            &mut config,
            &self.logger,
            backoff,
            serial_connection,
            initial_disconnect_reason,
        )
    }
}

impl AppConfig {
    pub fn from_sources(config: Config, opts: RunOptions) -> Self {
        Self {
            device: opts.device.unwrap_or_else(|| config.device.clone()),
            baud: opts.baud.unwrap_or(config.baud),
            flow_control: opts.flow_control.unwrap_or(config.flow_control),
            parity: opts.parity.unwrap_or(config.parity),
            stop_bits: opts.stop_bits.unwrap_or(config.stop_bits),
            dtr_on_open: opts.dtr_on_open.unwrap_or(config.dtr_on_open),
            serial_timeout_ms: opts.serial_timeout_ms.unwrap_or(config.serial_timeout_ms),
            cols: opts.cols.unwrap_or(config.cols),
            rows: opts.rows.unwrap_or(config.rows),
            scroll_speed_ms: config.scroll_speed_ms,
            page_timeout_ms: config.page_timeout_ms,
            button_gpio_pin: config.button_gpio_pin,
            payload_file: opts.payload_file,
            backoff_initial_ms: opts.backoff_initial_ms.unwrap_or(config.backoff_initial_ms),
            backoff_max_ms: opts.backoff_max_ms.unwrap_or(config.backoff_max_ms),
            pcf8574_addr: opts
                .pcf8574_addr
                .unwrap_or_else(|| config.pcf8574_addr.clone()),
            display_driver: config.display_driver,
            log_level: opts
                .log_level
                .as_deref()
                .and_then(|s| LogLevel::from_str(s).ok())
                .unwrap_or_default(),
            log_file: opts.log_file,
            demo: opts.demo,
            command_allowlist: config.command_allowlist.clone(),
            serialsh: matches!(opts.mode, RunMode::SerialShell),
        }
    }

    pub fn serial_options(&self) -> SerialOptions {
        SerialOptions {
            baud: self.baud,
            timeout_ms: self.serial_timeout_ms,
            flow_control: self.flow_control,
            parity: self.parity,
            stop_bits: self.stop_bits,
            dtr: self.dtr_on_open,
        }
    }
}

fn load_payload_from_file(path: &str, defaults: PayloadDefaults) -> Result<RenderFrame> {
    let raw = fs::read_to_string(path)?;
    RenderFrame::from_payload_json_with_defaults(&raw, defaults)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::cli::RunMode;
    use crate::config::Config;
    use std::time::{SystemTime, UNIX_EPOCH};

    fn set_temp_home() -> std::path::PathBuf {
        let mut dir = std::env::temp_dir();
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("time went backwards")
            .as_nanos();
        dir.push(format!("lifelinetty_app_test_home_{stamp}"));
        std::fs::create_dir_all(&dir).unwrap();
        std::env::set_var("HOME", &dir);
        dir
    }

    #[test]
    fn config_from_options() {
        let home = set_temp_home();
        let mut opts = RunOptions::default();
        opts.mode = RunMode::Daemon;
        opts.device = Some("/dev/ttyUSB1".into());
        opts.baud = Some(57_600);
        opts.cols = Some(16);
        opts.rows = Some(2);
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
            flow_control: FlowControlMode::default(),
            parity: ParityMode::default(),
            stop_bits: StopBitsMode::default(),
            dtr_on_open: DtrBehavior::default(),
            serial_timeout_ms: DEFAULT_SERIAL_TIMEOUT_MS,
            cols: 16,
            rows: 2,
            scroll_speed_ms: crate::config::DEFAULT_SCROLL_MS,
            page_timeout_ms: crate::config::DEFAULT_PAGE_TIMEOUT_MS,
            button_gpio_pin: None,
            backoff_initial_ms: crate::config::DEFAULT_BACKOFF_INITIAL_MS,
            backoff_max_ms: crate::config::DEFAULT_BACKOFF_MAX_MS,
            pcf8574_addr: crate::config::DEFAULT_PCF8574_ADDR,
            display_driver: crate::config::DEFAULT_DISPLAY_DRIVER,
            command_allowlist: Vec::new(),
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
}
