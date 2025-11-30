use crate::{cli::RunOptions, lcd::Lcd, serial::SerialPort, Result};

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
        RunOptions::default().into()
    }
}

impl From<RunOptions> for AppConfig {
    fn from(opts: RunOptions) -> Self {
        Self {
            device: opts.device,
            baud: opts.baud,
            cols: opts.cols,
            rows: opts.rows,
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

    pub fn from_options(opts: RunOptions) -> Self {
        Self::new(opts.into())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn config_from_options() {
        let opts = RunOptions {
            device: "/dev/ttyUSB1".into(),
            baud: 57_600,
            cols: 16,
            rows: 2,
        };
        let cfg: AppConfig = opts.clone().into();
        assert_eq!(cfg.device, "/dev/ttyUSB1");
        assert_eq!(cfg.baud, 57_600);
        assert_eq!(cfg.cols, 16);
        assert_eq!(cfg.rows, 2);

        let app = App::from_options(opts);
        assert_eq!(app.config.device, "/dev/ttyUSB1");
    }
}
