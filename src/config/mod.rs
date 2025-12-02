use crate::Result;
use std::path::Path;

pub mod loader;

pub const DEFAULT_DEVICE: &str = "/dev/ttyUSB0";
pub const DEFAULT_BAUD: u32 = 9_600;
pub const DEFAULT_COLS: u8 = 20;
pub const DEFAULT_ROWS: u8 = 4;
pub const DEFAULT_SCROLL_MS: u64 = 250;
pub const DEFAULT_PAGE_TIMEOUT_MS: u64 = 4000;
pub const DEFAULT_PCF8574_ADDR: Pcf8574Addr = Pcf8574Addr::Auto;
pub const DEFAULT_BACKOFF_INITIAL_MS: u64 = 500;
pub const DEFAULT_BACKOFF_MAX_MS: u64 = 10_000;
const CONFIG_DIR_NAME: &str = ".serial_lcd";
const CONFIG_FILE_NAME: &str = "config.toml";

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

/// User-supplied settings loaded from the config file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub device: String,
    pub baud: u32,
    pub cols: u8,
    pub rows: u8,
    pub scroll_speed_ms: u64,
    pub page_timeout_ms: u64,
    pub button_gpio_pin: Option<u8>,
    pub pcf8574_addr: Pcf8574Addr,
    pub backoff_initial_ms: u64,
    pub backoff_max_ms: u64,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            device: DEFAULT_DEVICE.to_string(),
            baud: DEFAULT_BAUD,
            cols: DEFAULT_COLS,
            rows: DEFAULT_ROWS,
            scroll_speed_ms: DEFAULT_SCROLL_MS,
            page_timeout_ms: DEFAULT_PAGE_TIMEOUT_MS,
            button_gpio_pin: None,
            pcf8574_addr: DEFAULT_PCF8574_ADDR,
            backoff_initial_ms: DEFAULT_BACKOFF_INITIAL_MS,
            backoff_max_ms: DEFAULT_BACKOFF_MAX_MS,
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

fn format_pcf_addr(addr: &Pcf8574Addr) -> String {
    match addr {
        Pcf8574Addr::Auto => "\"auto\"".into(),
        Pcf8574Addr::Addr(a) => format!("{a:#04x}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
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
            scroll_speed_ms = 300
            page_timeout_ms = 4500
            button_gpio_pin = 17
            pcf8574_addr = "0x23"
            backoff_initial_ms = 750
            backoff_max_ms = 9000
        "#;
        fs::write(&path, contents).unwrap();
        let cfg = Config::load_from_path(&path).unwrap();
        assert_eq!(cfg.device, "/dev/ttyUSB0");
        assert_eq!(cfg.baud, 9600);
        assert_eq!(cfg.cols, 16);
        assert_eq!(cfg.rows, 2);
        assert_eq!(cfg.scroll_speed_ms, 300);
        assert_eq!(cfg.page_timeout_ms, 4500);
        assert_eq!(cfg.button_gpio_pin, Some(17));
        assert_eq!(cfg.pcf8574_addr, Pcf8574Addr::Addr(0x23));
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
            cols: 20,
            rows: 4,
            scroll_speed_ms: 250,
            page_timeout_ms: 4000,
            button_gpio_pin: Some(22),
            pcf8574_addr: Pcf8574Addr::Auto,
            backoff_initial_ms: DEFAULT_BACKOFF_INITIAL_MS,
            backoff_max_ms: DEFAULT_BACKOFF_MAX_MS,
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
