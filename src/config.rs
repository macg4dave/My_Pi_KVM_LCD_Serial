use std::{
    fs,
    path::{Path, PathBuf},
};

use crate::{Error, Result};

pub const DEFAULT_DEVICE: &str = "/dev/ttyAMA0";
pub const DEFAULT_BAUD: u32 = 115_200;
pub const DEFAULT_COLS: u8 = 20;
pub const DEFAULT_ROWS: u8 = 4;
const CONFIG_DIR_NAME: &str = ".serial_lcd";
const CONFIG_FILE_NAME: &str = "config.toml";

/// User-supplied settings loaded from the config file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub device: String,
    pub baud: u32,
    pub cols: u8,
    pub rows: u8,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            device: DEFAULT_DEVICE.to_string(),
            baud: DEFAULT_BAUD,
            cols: DEFAULT_COLS,
            rows: DEFAULT_ROWS,
        }
    }
}

impl Config {
    pub fn load_or_default() -> Result<Self> {
        let path = config_path()?;
        Self::load_from_path(&path)
    }

    pub fn load_from_path(path: &Path) -> Result<Self> {
        if !path.exists() {
            return Ok(Self::default());
        }

        let raw = fs::read_to_string(path)?;
        Self::parse(&raw)
    }

    pub fn save(&self) -> Result<()> {
        let path = config_path()?;
        self.save_to_path(&path)
    }

    pub fn save_to_path(&self, path: &Path) -> Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }

        let contents = format!(
            "# seriallcd config\n\
device = \"{}\"\n\
baud = {}\n\
cols = {}\n\
rows = {}\n",
            self.device, self.baud, self.cols, self.rows
        );
        fs::write(path, contents)?;
        Ok(())
    }

    fn parse(raw: &str) -> Result<Self> {
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
    use std::time::{SystemTime, UNIX_EPOCH};

    fn temp_path(name: &str) -> PathBuf {
        let stamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis();
        std::env::temp_dir().join(format!("seriallcd_test_{name}_{stamp}"))
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
        "#;
        fs::write(&path, contents).unwrap();
        let cfg = Config::load_from_path(&path).unwrap();
        assert_eq!(cfg.device, "/dev/ttyUSB0");
        assert_eq!(cfg.baud, 9600);
        assert_eq!(cfg.cols, 16);
        assert_eq!(cfg.rows, 2);
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
        };
        cfg.save_to_path(&path).unwrap();
        let loaded = Config::load_from_path(&path).unwrap();
        assert_eq!(cfg, loaded);
        let _ = fs::remove_file(&path);
        if let Some(parent) = path.parent() {
            let _ = fs::remove_dir(parent);
        }
    }
}
