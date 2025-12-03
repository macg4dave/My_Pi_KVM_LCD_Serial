use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{Error, Result};

use super::{Config, CONFIG_DIR_NAME, CONFIG_FILE_NAME};

pub fn load_or_default() -> Result<Config> {
    let path = config_path()?;
    if !path.exists() {
        let cfg = Config::default();
        cfg.save_to_path(&path)?;
        super::validate(&cfg)?;
        return Ok(cfg);
    }
    load_from_path(&path)
}

pub fn load_from_path(path: &Path) -> Result<Config> {
    if !path.exists() {
        let cfg = Config::default();
        super::validate(&cfg)?;
        return Ok(cfg);
    }

    let raw = fs::read_to_string(path)?;
    parse(&raw)
}

pub fn save(config: &Config) -> Result<()> {
    let path = config_path()?;
    save_to_path(config, &path)
}

pub fn save_to_path(config: &Config, path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let allowlist = format_string_array(&config.command_allowlist);

    let contents = format!(
        "# lifelinetty config\n\
device = \"{}\"\n\
baud = {}\n\
flow_control = \"{}\"\n\
parity = \"{}\"\n\
stop_bits = \"{}\"\n\
dtr_on_open = \"{}\"\n\
serial_timeout_ms = {}\n\
cols = {}\n\
rows = {}\n\
scroll_speed_ms = {}\n\
page_timeout_ms = {}\n\
button_gpio_pin = {}\n\
pcf8574_addr = {}\n\
display_driver = {}\n\
backoff_initial_ms = {}\n\
backoff_max_ms = {}\n",
        config.device,
        config.baud,
        config.flow_control,
        config.parity,
        config.stop_bits,
        config.dtr_on_open,
        config.serial_timeout_ms,
        config.cols,
        config.rows,
        config.scroll_speed_ms,
        config.page_timeout_ms,
        config
            .button_gpio_pin
            .map(|p| p.to_string())
            .unwrap_or_else(|| "null".into()),
        super::format_pcf_addr(&config.pcf8574_addr),
        super::format_display_driver(&config.display_driver),
        config.backoff_initial_ms,
        config.backoff_max_ms
    );
    let contents = format!("{contents}command_allowlist = {}\n", allowlist);
    fs::write(path, contents)?;
    Ok(())
}

pub fn parse(raw: &str) -> Result<Config> {
    let mut cfg = Config::default();

    for (idx, line) in raw.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let (key, value) = trimmed.split_once('=').ok_or_else(|| {
            Error::InvalidArgs(format!("invalid config line {}: '{}'", idx + 1, line))
        })?;

        let key = key.trim();
        let value = value.trim().trim_matches('"');
        match key {
            "device" => cfg.device = value.to_string(),
            "baud" => {
                cfg.baud = value.parse().map_err(|_| {
                    Error::InvalidArgs(format!("invalid baud value on line {}", idx + 1))
                })?;
            }
            "flow_control" => {
                cfg.flow_control = value.parse().map_err(|e: String| {
                    Error::InvalidArgs(format!("invalid flow_control on line {}: {e}", idx + 1))
                })?;
            }
            "parity" => {
                cfg.parity = value.parse().map_err(|e: String| {
                    Error::InvalidArgs(format!("invalid parity on line {}: {e}", idx + 1))
                })?;
            }
            "stop_bits" => {
                cfg.stop_bits = value.parse().map_err(|e: String| {
                    Error::InvalidArgs(format!("invalid stop_bits on line {}: {e}", idx + 1))
                })?;
            }
            "dtr_on_open" => {
                cfg.dtr_on_open = value.parse().map_err(|e: String| {
                    Error::InvalidArgs(format!("invalid dtr_on_open on line {}: {e}", idx + 1))
                })?;
            }
            "serial_timeout_ms" => {
                cfg.serial_timeout_ms = value.parse().map_err(|_| {
                    Error::InvalidArgs(format!("invalid serial_timeout_ms on line {}", idx + 1))
                })?;
            }
            "cols" => {
                cfg.cols = value.parse().map_err(|_| {
                    Error::InvalidArgs(format!("invalid cols value on line {}", idx + 1))
                })?;
            }
            "rows" => {
                cfg.rows = value.parse().map_err(|_| {
                    Error::InvalidArgs(format!("invalid rows value on line {}", idx + 1))
                })?;
            }
            "scroll_speed_ms" => {
                cfg.scroll_speed_ms = value.parse().map_err(|_| {
                    Error::InvalidArgs(format!("invalid scroll_speed_ms on line {}", idx + 1))
                })?;
            }
            "page_timeout_ms" => {
                cfg.page_timeout_ms = value.parse().map_err(|_| {
                    Error::InvalidArgs(format!("invalid page_timeout_ms on line {}", idx + 1))
                })?;
            }
            "pcf8574_addr" => {
                cfg.pcf8574_addr = super::parse_pcf_addr(value).map_err(|e| {
                    Error::InvalidArgs(format!("invalid pcf8574_addr on line {}: {e}", idx + 1))
                })?;
            }
            "display_driver" => {
                cfg.display_driver = value.parse().map_err(|e: String| {
                    Error::InvalidArgs(format!("invalid display_driver on line {}: {e}", idx + 1))
                })?;
            }
            "backoff_initial_ms" => {
                cfg.backoff_initial_ms = value.parse().map_err(|_| {
                    Error::InvalidArgs(format!("invalid backoff_initial_ms on line {}", idx + 1))
                })?;
            }
            "backoff_max_ms" => {
                cfg.backoff_max_ms = value.parse().map_err(|_| {
                    Error::InvalidArgs(format!("invalid backoff_max_ms on line {}", idx + 1))
                })?;
            }
            "button_gpio_pin" => {
                if value == "null" {
                    cfg.button_gpio_pin = None;
                } else {
                    cfg.button_gpio_pin = Some(value.parse().map_err(|_| {
                        Error::InvalidArgs(format!("invalid button_gpio_pin on line {}", idx + 1))
                    })?);
                }
            }
            "command_allowlist" => {
                cfg.command_allowlist = parse_string_array(value).map_err(|e| {
                    Error::InvalidArgs(format!(
                        "invalid command_allowlist on line {}: {e}",
                        idx + 1
                    ))
                })?;
            }
            other => {
                return Err(Error::InvalidArgs(format!(
                    "unknown config key '{}' on line {}",
                    other,
                    idx + 1
                )));
            }
        }
    }

    super::validate(&cfg)?;
    Ok(cfg)
}

fn config_path() -> Result<PathBuf> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| Error::InvalidArgs("HOME not set; cannot locate config directory".into()))?;
    Ok(home.join(CONFIG_DIR_NAME).join(CONFIG_FILE_NAME))
}

fn parse_string_array(value: &str) -> std::result::Result<Vec<String>, String> {
    let trimmed = value.trim();
    if !trimmed.starts_with('[') || !trimmed.ends_with(']') {
        return Err("expected array literal (e.g., [\"cmd\", \"other\"])".into());
    }
    let inner = &trimmed[1..trimmed.len() - 1];
    if inner.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut entries = Vec::new();
    for part in inner.split(',') {
        let item = part.trim();
        if item.is_empty() {
            continue;
        }
        let cleaned = if item.len() >= 2
            && ((item.starts_with('"') && item.ends_with('"'))
                || (item.starts_with('\'') && item.ends_with('\'')))
        {
            &item[1..item.len() - 1]
        } else {
            item
        };
        let cleaned = cleaned.trim();
        if cleaned.is_empty() {
            return Err("command entries must not be empty".into());
        }
        entries.push(cleaned.to_string());
    }
    Ok(entries)
}

fn format_string_array(values: &[String]) -> String {
    if values.is_empty() {
        return "[]".into();
    }
    let quoted = values
        .iter()
        .map(|value| {
            let mut encoded = String::new();
            for ch in value.chars() {
                match ch {
                    '\\' => encoded.push_str("\\\\"),
                    '"' => encoded.push_str("\\\""),
                    other => encoded.push(other),
                }
            }
            format!("\"{}\"", encoded)
        })
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{quoted}]")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{
        Config, DisplayDriver, Pcf8574Addr, DEFAULT_BACKOFF_INITIAL_MS, DEFAULT_BACKOFF_MAX_MS,
    };
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
        let cfg = load_from_path(&path).unwrap();
        assert_eq!(cfg, Config::default());
    }

    #[test]
    fn parses_valid_config() {
        let path = temp_path("parse");
        let contents = r#"
            device = "/dev/ttyUSB0"
            baud = 9600
            flow_control = "hardware"
            parity = "even"
            stop_bits = "2"
            dtr_on_open = "on"
            serial_timeout_ms = 1500
            cols = 16
            rows = 2
            scroll_speed_ms = 300
            page_timeout_ms = 4500
            button_gpio_pin = 17
            pcf8574_addr = "0x23"
            display_driver = "in-tree"
            backoff_initial_ms = 750
            backoff_max_ms = 9000
        "#;
        fs::write(&path, contents).unwrap();
        let cfg = load_from_path(&path).unwrap();
        assert_eq!(cfg.device, "/dev/ttyUSB0");
        assert_eq!(cfg.baud, 9600);
        assert_eq!(cfg.flow_control, FlowControlMode::Hardware);
        assert_eq!(cfg.parity, ParityMode::Even);
        assert_eq!(cfg.stop_bits, StopBitsMode::Two);
        assert_eq!(cfg.dtr_on_open, DtrBehavior::Assert);
        assert_eq!(cfg.serial_timeout_ms, 1500);
        assert_eq!(cfg.cols, 16);
        assert_eq!(cfg.rows, 2);
        assert_eq!(cfg.scroll_speed_ms, 300);
        assert_eq!(cfg.page_timeout_ms, 4500);
        assert_eq!(cfg.button_gpio_pin, Some(17));
        assert_eq!(cfg.pcf8574_addr, Pcf8574Addr::Addr(0x23));
        assert_eq!(cfg.display_driver, DisplayDriver::InTree);
        assert_eq!(cfg.backoff_initial_ms, 750);
        assert_eq!(cfg.backoff_max_ms, 9000);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn parses_command_allowlist() {
        let path = temp_path("allowlist");
        fs::write(&path, "command_allowlist = [\"ls\", \"uptime\"]").unwrap();
        let cfg = load_from_path(&path).unwrap();
        assert_eq!(cfg.command_allowlist, vec!["ls", "uptime"]);
        let _ = fs::remove_file(path);
    }

    #[test]
    fn rejects_invalid_command_allowlist_literal() {
        let path = temp_path("bad_allowlist");
        fs::write(&path, "command_allowlist = ls").unwrap();
        let err = load_from_path(&path).unwrap_err();
        assert!(format!("{err}").contains("command_allowlist"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn rejects_unknown_key() {
        let path = temp_path("unknown");
        fs::write(&path, "nope = 1").unwrap();
        let err = load_from_path(&path).unwrap_err();
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
            dtr_on_open: DtrBehavior::Deassert,
            serial_timeout_ms: 1200,
            cols: 20,
            rows: 4,
            scroll_speed_ms: 250,
            page_timeout_ms: 4000,
            button_gpio_pin: Some(22),
            pcf8574_addr: Pcf8574Addr::Auto,
            display_driver: DisplayDriver::Hd44780Driver,
            backoff_initial_ms: DEFAULT_BACKOFF_INITIAL_MS,
            backoff_max_ms: DEFAULT_BACKOFF_MAX_MS,
            command_allowlist: Vec::new(),
        };
        save_to_path(&cfg, &path).unwrap();
        let loaded = load_from_path(&path).unwrap();
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
        let cfg_path = home.join(CONFIG_DIR_NAME).join(CONFIG_FILE_NAME);

        let cfg = load_or_default().unwrap();
        assert_eq!(cfg, Config::default());
        assert!(cfg_path.exists(), "expected config file to be created");

        let contents = fs::read_to_string(&cfg_path).unwrap();
        assert!(contents.contains("device ="));
        assert!(contents.contains("baud ="));

        let _ = fs::remove_dir_all(home);
    }

    #[test]
    fn rejects_cols_outside_range() {
        let path = temp_path("cols_out_of_range");
        fs::write(&path, "cols = 99").unwrap();
        let err = load_from_path(&path).unwrap_err();
        assert!(format!("{err}").contains("cols must"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn rejects_rows_outside_range() {
        let path = temp_path("rows_out_of_range");
        fs::write(&path, "rows = 0").unwrap();
        let err = load_from_path(&path).unwrap_err();
        assert!(format!("{err}").contains("rows must"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn rejects_scroll_speed_below_min() {
        let path = temp_path("scroll_speed_invalid");
        fs::write(&path, "scroll_speed_ms = 10").unwrap();
        let err = load_from_path(&path).unwrap_err();
        assert!(format!("{err}").contains("scroll_speed_ms"));
        let _ = fs::remove_file(path);
    }

    #[test]
    fn rejects_page_timeout_below_min() {
        let path = temp_path("page_timeout_invalid");
        fs::write(&path, "page_timeout_ms = 10").unwrap();
        let err = load_from_path(&path).unwrap_err();
        assert!(format!("{err}").contains("page_timeout_ms"));
        let _ = fs::remove_file(path);
    }
}
