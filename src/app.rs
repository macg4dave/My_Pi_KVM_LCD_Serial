use crate::{
    cli::RunOptions,
    config::{Config, DEFAULT_BAUD, DEFAULT_COLS, DEFAULT_DEVICE, DEFAULT_ROWS},
    lcd::Lcd,
    serial::SerialPort,
    Result,
};

/// Config for the daemon.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppConfig {
    pub device: String,
    pub baud: u32,
    pub cols: u8,
    pub rows: u8,
}

impl Default for AppConfig {
    fn default() -> Self {
        Self {
            device: DEFAULT_DEVICE.to_string(),
            baud: DEFAULT_BAUD,
            cols: DEFAULT_COLS,
            rows: DEFAULT_ROWS,
        }
    }
}

pub struct App {
    config: AppConfig,
}

impl App {
    pub fn new(config: AppConfig) -> Self {
        Self { config }
    }

    pub fn from_options(opts: RunOptions) -> Result<Self> {
        let cfg_file = Config::load_or_default()?;
        let merged = AppConfig::from_sources(cfg_file, opts);
        Ok(Self::new(merged))
    }

    /// Entry point for the daemon. Wire up serial + LCD here.
    pub fn run(&self) -> Result<()> {
        let port = SerialPort::connect(&self.config.device, self.config.baud)?;
        let mut lcd = Lcd::new(self.config.cols, self.config.rows);

        lcd.render_boot_message()?;
        port.send_line("INIT")?;

        // TODO: add event loop that reads from data source and refreshes LCD.
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
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    #[test]
    fn config_from_options() {
        let opts = RunOptions {
            device: Some("/dev/ttyUSB1".into()),
            baud: Some(57_600),
            cols: Some(16),
            rows: Some(2),
        };
        let cfg = AppConfig::from_sources(Config::default(), opts.clone());
        assert_eq!(cfg.device, "/dev/ttyUSB1");
        assert_eq!(cfg.baud, 57_600);
        assert_eq!(cfg.cols, 16);
        assert_eq!(cfg.rows, 2);

        let app = App::from_options(opts).unwrap();
        assert_eq!(app.config.device, "/dev/ttyUSB1");
    }

    #[test]
    fn config_prefers_file_values_when_cli_missing() {
        let cfg_file = Config {
            device: "/dev/ttyS0".into(),
            baud: 9_600,
            cols: 16,
            rows: 2,
        };
        let opts = RunOptions::default();
        let merged = AppConfig::from_sources(cfg_file.clone(), opts);
        assert_eq!(merged.device, cfg_file.device);
        assert_eq!(merged.baud, cfg_file.baud);
        assert_eq!(merged.cols, cfg_file.cols);
        assert_eq!(merged.rows, cfg_file.rows);
    }
}
