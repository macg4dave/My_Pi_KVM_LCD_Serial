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
        return Ok(cfg);
    }
    load_from_path(&path)
}

pub fn load_from_path(path: &Path) -> Result<Config> {
    if !path.exists() {
        return Ok(Config::default());
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

    let contents = format!(
        "# lifelinetty config\n\
device = \"{}\"\n\
baud = {}\n\
cols = {}\n\
rows = {}\n\
scroll_speed_ms = {}\n\
page_timeout_ms = {}\n\
button_gpio_pin = {}\n\
pcf8574_addr = {}\n\
backoff_initial_ms = {}\n\
backoff_max_ms = {}\n",
        config.device,
        config.baud,
        config.cols,
        config.rows,
        config.scroll_speed_ms,
        config.page_timeout_ms,
        config
            .button_gpio_pin
            .map(|p| p.to_string())
            .unwrap_or_else(|| "null".into()),
        super::format_pcf_addr(&config.pcf8574_addr),
        config.backoff_initial_ms,
        config.backoff_max_ms
    );
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
            other => {
                return Err(Error::InvalidArgs(format!(
                    "unknown config key '{}' on line {}",
                    other,
                    idx + 1
                )));
            }
        }
    }

    Ok(cfg)
}

fn config_path() -> Result<PathBuf> {
    let home = std::env::var_os("HOME")
        .map(PathBuf::from)
        .ok_or_else(|| Error::InvalidArgs("HOME not set; cannot locate config directory".into()))?;
    Ok(home.join(CONFIG_DIR_NAME).join(CONFIG_FILE_NAME))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::{Config, Pcf8574Addr, DEFAULT_BACKOFF_INITIAL_MS, DEFAULT_BACKOFF_MAX_MS};
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
        let cfg = load_from_path(&path).unwrap();
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
            cols: 20,
            rows: 4,
            scroll_speed_ms: 250,
            page_timeout_ms: 4000,
            button_gpio_pin: Some(22),
            pcf8574_addr: Pcf8574Addr::Auto,
            backoff_initial_ms: DEFAULT_BACKOFF_INITIAL_MS,
            backoff_max_ms: DEFAULT_BACKOFF_MAX_MS,
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
}
