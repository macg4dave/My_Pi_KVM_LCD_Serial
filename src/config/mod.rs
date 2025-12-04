use crate::{
    compression::CompressionCodec,
    negotiation::RolePreference,
    serial::{DtrBehavior, FlowControlMode, ParityMode, StopBitsMode},
    Error, Result,
};
use std::path::Path;

pub mod loader;
pub mod profiles;

pub const DEFAULT_DEVICE: &str = "/dev/ttyUSB0";
pub const DEFAULT_BAUD: u32 = 9_600;
pub const MIN_BAUD: u32 = 9_600;
pub const DEFAULT_COLS: u8 = 20;
pub const DEFAULT_ROWS: u8 = 4;
pub const DEFAULT_LCD_PRESENT: bool = true;
pub const MIN_COLS: u8 = 8;
pub const MAX_COLS: u8 = 40;
pub const MIN_ROWS: u8 = 1;
pub const MAX_ROWS: u8 = 4;
pub const DEFAULT_SCROLL_MS: u64 = 250;
pub const DEFAULT_PAGE_TIMEOUT_MS: u64 = 4000;
pub const MIN_SCROLL_MS: u64 = 100;
pub const MIN_PAGE_TIMEOUT_MS: u64 = 500;
pub const DEFAULT_POLLING_ENABLED: bool = false;
pub const DEFAULT_POLL_INTERVAL_MS: u64 = 5000;
pub const MIN_POLL_INTERVAL_MS: u64 = 1000;
pub const MAX_POLL_INTERVAL_MS: u64 = 60000;
pub const DEFAULT_PCF8574_ADDR: Pcf8574Addr = Pcf8574Addr::Auto;
pub const DEFAULT_DISPLAY_DRIVER: DisplayDriver = DisplayDriver::Auto;
pub const DEFAULT_BACKOFF_INITIAL_MS: u64 = 500;
pub const DEFAULT_BACKOFF_MAX_MS: u64 = 10_000;
pub const DEFAULT_SERIAL_TIMEOUT_MS: u64 = 500;
pub const MIN_SERIAL_TIMEOUT_MS: u64 = 50;
pub const MAX_SERIAL_TIMEOUT_MS: u64 = 60_000;
pub const DEFAULT_WATCHDOG_SERIAL_TIMEOUT_MS: u64 = 12_000;
pub const DEFAULT_WATCHDOG_TUNNEL_TIMEOUT_MS: u64 = 5_000;
pub const MIN_WATCHDOG_TIMEOUT_MS: u64 = 1_000;
pub const MAX_WATCHDOG_TIMEOUT_MS: u64 = 120_000;
pub const DEFAULT_NEGOTIATION_NODE_ID: u32 = 42;
pub const DEFAULT_NEGOTIATION_TIMEOUT_MS: u64 = 1_000;
pub const MIN_NEGOTIATION_TIMEOUT_MS: u64 = 250;
pub const MAX_NEGOTIATION_TIMEOUT_MS: u64 = 5_000;
pub const NEGOTIATION_SECTION_NAME: &str = "negotiation";
pub const DEFAULT_PROTOCOL_SCHEMA_VERSION: u8 = 1;
pub const DEFAULT_PROTOCOL_COMPRESSION_ENABLED: bool = false;
pub const DEFAULT_PROTOCOL_COMPRESSION_CODEC: CompressionCodec = CompressionCodec::Lz4;
const CONFIG_DIR_NAME: &str = ".serial_lcd";
const CONFIG_FILE_NAME: &str = "config.toml";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtocolConfig {
    pub schema_version: u8,
    pub compression_enabled: bool,
    pub compression_codec: CompressionCodec,
}

impl Default for ProtocolConfig {
    fn default() -> Self {
        Self {
            schema_version: DEFAULT_PROTOCOL_SCHEMA_VERSION,
            compression_enabled: DEFAULT_PROTOCOL_COMPRESSION_ENABLED,
            compression_codec: DEFAULT_PROTOCOL_COMPRESSION_CODEC,
        }
    }
}

/// Settings that control how this node participates in auto-negotiation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NegotiationConfig {
    pub node_id: u32,
    pub preference: RolePreference,
    pub timeout_ms: u64,
}

impl Default for NegotiationConfig {
    fn default() -> Self {
        Self {
            node_id: DEFAULT_NEGOTIATION_NODE_ID,
            preference: RolePreference::default(),
            timeout_ms: DEFAULT_NEGOTIATION_TIMEOUT_MS,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pcf8574Addr {
    Auto,
    Addr(u8),
}

impl std::str::FromStr for Pcf8574Addr {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        parse_pcf_addr(s)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayDriver {
    Auto,
    InTree,
    Hd44780Driver,
}

impl Default for DisplayDriver {
    fn default() -> Self {
        DisplayDriver::Auto
    }
}

impl std::str::FromStr for DisplayDriver {
    type Err = String;

    fn from_str(s: &str) -> std::result::Result<Self, Self::Err> {
        match s.trim().to_ascii_lowercase().as_str() {
            "auto" => Ok(DisplayDriver::Auto),
            "in-tree" | "intree" => Ok(DisplayDriver::InTree),
            "hd44780-driver" | "hd44780" => Ok(DisplayDriver::Hd44780Driver),
            other => Err(format!(
                "expected 'auto', 'in-tree', or 'hd44780-driver', got '{other}'"
            )),
        }
    }
}

impl std::fmt::Display for DisplayDriver {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self {
            DisplayDriver::Auto => "auto",
            DisplayDriver::InTree => "in-tree",
            DisplayDriver::Hd44780Driver => "hd44780-driver",
        })
    }
}

/// User-supplied settings loaded from the config file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WatchdogConfig {
    pub serial_timeout_ms: u64,
    pub tunnel_timeout_ms: u64,
}

impl Default for WatchdogConfig {
    fn default() -> Self {
        Self {
            serial_timeout_ms: DEFAULT_WATCHDOG_SERIAL_TIMEOUT_MS,
            tunnel_timeout_ms: DEFAULT_WATCHDOG_TUNNEL_TIMEOUT_MS,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
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
    pub polling_enabled: bool,
    pub poll_interval_ms: u64,
    pub button_gpio_pin: Option<u8>,
    pub pcf8574_addr: Pcf8574Addr,
    pub display_driver: DisplayDriver,
    pub lcd_present: bool,
    pub backoff_initial_ms: u64,
    pub backoff_max_ms: u64,
    pub negotiation: NegotiationConfig,
    pub command_allowlist: Vec<String>,
    pub protocol: ProtocolConfig,
    pub watchdog: WatchdogConfig,
}

impl Default for Config {
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
            scroll_speed_ms: DEFAULT_SCROLL_MS,
            page_timeout_ms: DEFAULT_PAGE_TIMEOUT_MS,
            polling_enabled: DEFAULT_POLLING_ENABLED,
            poll_interval_ms: DEFAULT_POLL_INTERVAL_MS,
            button_gpio_pin: None,
            pcf8574_addr: DEFAULT_PCF8574_ADDR,
            display_driver: DEFAULT_DISPLAY_DRIVER,
            lcd_present: DEFAULT_LCD_PRESENT,
            backoff_initial_ms: DEFAULT_BACKOFF_INITIAL_MS,
            backoff_max_ms: DEFAULT_BACKOFF_MAX_MS,
            negotiation: NegotiationConfig::default(),
            command_allowlist: Vec::new(),
            protocol: ProtocolConfig::default(),
            watchdog: WatchdogConfig::default(),
        }
    }
}

impl Config {
    pub fn load_or_default() -> Result<Self> {
        loader::load_or_default()
    }

    pub fn load_from_path(path: &Path) -> Result<Self> {
        loader::load_from_path(path)
    }

    pub fn save(&self) -> Result<()> {
        loader::save(self)
    }

    pub fn save_to_path(&self, path: &Path) -> Result<()> {
        loader::save_to_path(self, path)
    }

    #[allow(dead_code)]
    fn parse(raw: &str) -> Result<Self> {
        loader::parse(raw)
    }
}

fn parse_pcf_addr(raw: &str) -> std::result::Result<Pcf8574Addr, String> {
    if raw.eq_ignore_ascii_case("auto") {
        return Ok(Pcf8574Addr::Auto);
    }
    let cleaned = raw.trim_start_matches("0x");
    let value = u8::from_str_radix(cleaned, 16)
        .or_else(|_| raw.parse::<u8>())
        .map_err(|_| "expected 'auto' or a hex/decimal address (e.g., 0x27)".to_string())?;
    Ok(Pcf8574Addr::Addr(value))
}

pub(crate) fn validate(cfg: &Config) -> Result<()> {
    validate_baud(cfg.baud)?;
    if cfg.cols < MIN_COLS || cfg.cols > MAX_COLS {
        return Err(Error::InvalidArgs(format!(
            "cols must be between {MIN_COLS} and {MAX_COLS}"
        )));
    }
    if cfg.rows < MIN_ROWS || cfg.rows > MAX_ROWS {
        return Err(Error::InvalidArgs(format!(
            "rows must be between {MIN_ROWS} and {MAX_ROWS}"
        )));
    }
    if cfg.scroll_speed_ms < MIN_SCROLL_MS {
        return Err(Error::InvalidArgs(format!(
            "scroll_speed_ms must be at least {MIN_SCROLL_MS}"
        )));
    }
    if cfg.page_timeout_ms < MIN_PAGE_TIMEOUT_MS {
        return Err(Error::InvalidArgs(format!(
            "page_timeout_ms must be at least {MIN_PAGE_TIMEOUT_MS}"
        )));
    }
    if cfg.poll_interval_ms < MIN_POLL_INTERVAL_MS || cfg.poll_interval_ms > MAX_POLL_INTERVAL_MS {
        return Err(Error::InvalidArgs(format!(
            "poll_interval_ms must be between {MIN_POLL_INTERVAL_MS} and {MAX_POLL_INTERVAL_MS}"
        )));
    }
    for entry in &cfg.command_allowlist {
        if entry.trim().is_empty() {
            return Err(Error::InvalidArgs(
                "command_allowlist entries must be non-empty".to_string(),
            ));
        }
    }
    if cfg.protocol.schema_version != DEFAULT_PROTOCOL_SCHEMA_VERSION {
        return Err(Error::InvalidArgs(format!(
            "protocol.schema_version must be {}",
            DEFAULT_PROTOCOL_SCHEMA_VERSION
        )));
    }
    if cfg.serial_timeout_ms < MIN_SERIAL_TIMEOUT_MS
        || cfg.serial_timeout_ms > MAX_SERIAL_TIMEOUT_MS
    {
        return Err(Error::InvalidArgs(format!(
            "serial_timeout_ms must be between {MIN_SERIAL_TIMEOUT_MS} and {MAX_SERIAL_TIMEOUT_MS}"
        )));
    }
    if cfg.negotiation.timeout_ms < MIN_NEGOTIATION_TIMEOUT_MS
        || cfg.negotiation.timeout_ms > MAX_NEGOTIATION_TIMEOUT_MS
    {
        return Err(Error::InvalidArgs(format!(
            "negotiation.timeout_ms must be between {MIN_NEGOTIATION_TIMEOUT_MS} and {MAX_NEGOTIATION_TIMEOUT_MS}"
        )));
    }
    if cfg.watchdog.serial_timeout_ms < MIN_WATCHDOG_TIMEOUT_MS
        || cfg.watchdog.serial_timeout_ms > MAX_WATCHDOG_TIMEOUT_MS
    {
        return Err(Error::InvalidArgs(format!(
            "watchdog.serial_timeout_ms must be between {MIN_WATCHDOG_TIMEOUT_MS} and {MAX_WATCHDOG_TIMEOUT_MS}"
        )));
    }
    if cfg.watchdog.tunnel_timeout_ms < MIN_WATCHDOG_TIMEOUT_MS
        || cfg.watchdog.tunnel_timeout_ms > MAX_WATCHDOG_TIMEOUT_MS
    {
        return Err(Error::InvalidArgs(format!(
            "watchdog.tunnel_timeout_ms must be between {MIN_WATCHDOG_TIMEOUT_MS} and {MAX_WATCHDOG_TIMEOUT_MS}"
        )));
    }
    Ok(())
}

pub fn validate_baud(baud: u32) -> Result<()> {
    if baud < MIN_BAUD {
        return Err(Error::InvalidArgs(format!(
            "baud must be at least {MIN_BAUD}"
        )));
    }
    Ok(())
}

fn format_pcf_addr(addr: &Pcf8574Addr) -> String {
    match addr {
        Pcf8574Addr::Auto => "\"auto\"".into(),
        Pcf8574Addr::Addr(a) => format!("{a:#04x}"),
    }
}

fn format_display_driver(driver: &DisplayDriver) -> String {
    format!("\"{}\"", driver)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::serial::{DtrBehavior, FlowControlMode, ParityMode, StopBitsMode};
    use std::{
        fs,
        path::PathBuf,
        time::{SystemTime, UNIX_EPOCH},
    };

    fn temp_home(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        std::env::temp_dir().join(format!("lifelinetty_home_{name}_{stamp}"))
    }

    fn temp_path(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        std::env::temp_dir().join(format!("lifelinetty_test_{name}_{stamp}"))
    }

    #[test]
    fn loads_default_when_missing() {
        let path = temp_path("missing");
        let cfg = Config::load_from_path(&path).unwrap();
        assert_eq!(cfg, Config::default());
    }

    #[test]
    fn parses_valid_config() {
        let path = temp_path("parse");
        let contents = r#"
            device = "/dev/ttyUSB0"
            baud = 9600
            cols = 16
            rows = 2
            lcd_present = false
            scroll_speed_ms = 300
            page_timeout_ms = 4500
            polling_enabled = true
            poll_interval_ms = 2000
            button_gpio_pin = 17
            pcf8574_addr = "0x23"
            display_driver = "hd44780-driver"
            backoff_initial_ms = 750
            backoff_max_ms = 9000
        "#;
        fs::write(&path, contents).unwrap();
        let cfg = Config::load_from_path(&path).unwrap();
        assert_eq!(cfg.device, "/dev/ttyUSB0");
        assert_eq!(cfg.baud, 9600);
        assert_eq!(cfg.cols, 16);
        assert_eq!(cfg.rows, 2);
        assert!(!cfg.lcd_present);
        assert_eq!(cfg.scroll_speed_ms, 300);
        assert_eq!(cfg.page_timeout_ms, 4500);
        assert!(cfg.polling_enabled);
        assert_eq!(cfg.poll_interval_ms, 2000);
        assert_eq!(cfg.button_gpio_pin, Some(17));
        assert_eq!(cfg.pcf8574_addr, Pcf8574Addr::Addr(0x23));
        assert_eq!(cfg.display_driver, DisplayDriver::Hd44780Driver);
        assert_eq!(cfg.backoff_initial_ms, 750);
        assert_eq!(cfg.backoff_max_ms, 9000);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn rejects_unknown_key() {
        let path = temp_path("unknown");
        fs::write(&path, "nope = 1").unwrap();
        let err = Config::load_from_path(&path).unwrap_err();
        assert!(format!("{err}").contains("unknown config key"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn saves_and_loads_round_trip() {
        let path = temp_path("roundtrip");
        let cfg = Config {
            device: "/dev/ttyS1".into(),
            baud: 57_600,
            flow_control: FlowControlMode::Hardware,
            parity: ParityMode::Even,
            stop_bits: StopBitsMode::Two,
            dtr_on_open: DtrBehavior::Assert,
            serial_timeout_ms: 750,
            cols: 20,
            rows: 4,
            scroll_speed_ms: 250,
            page_timeout_ms: 4000,
            polling_enabled: true,
            poll_interval_ms: 2000,
            button_gpio_pin: Some(22),
            pcf8574_addr: Pcf8574Addr::Auto,
            display_driver: DisplayDriver::InTree,
            backoff_initial_ms: DEFAULT_BACKOFF_INITIAL_MS,
            backoff_max_ms: DEFAULT_BACKOFF_MAX_MS,
            negotiation: NegotiationConfig::default(),
            command_allowlist: Vec::new(),
            protocol: ProtocolConfig::default(),
            lcd_present: DEFAULT_LCD_PRESENT,
            watchdog: WatchdogConfig::default(),
        };
        cfg.save_to_path(&path).unwrap();
        let loaded = Config::load_from_path(&path).unwrap();
        assert_eq!(cfg, loaded);
        let _ = fs::remove_file(&path);
        if let Some(parent) = path.parent() {
            let _ = fs::remove_dir(parent);
        }
    }

    #[test]
    fn load_or_default_creates_file_with_defaults() {
        let home = temp_home("create");
        std::env::set_var("HOME", &home);
        let cfg_path = home.join(".serial_lcd").join("config.toml");

        let cfg = Config::load_or_default().unwrap();
        assert_eq!(cfg, Config::default());
        assert!(cfg_path.exists(), "expected config file to be created");

        let contents = fs::read_to_string(&cfg_path).unwrap();
        assert!(contents.contains("device ="));
        assert!(contents.contains("baud ="));

        let _ = fs::remove_dir_all(home);
    }
}
