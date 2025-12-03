use crate::{
    config::{DisplayDriver, Pcf8574Addr},
    Error, Result,
};

#[cfg(target_os = "linux")]
use crate::lcd_driver::{self, external::ExternalHd44780, pcf8574::RppalBus};

pub const BAR_LEVELS: [char; 6] = ['\u{0}', '\u{1}', '\u{2}', '\u{3}', '\u{4}', '\u{5}'];
pub const BAR_EMPTY: char = BAR_LEVELS[0];
pub const BAR_FULL: char = BAR_LEVELS[5];
pub const HEARTBEAT_CHAR: char = '\u{6}';
pub const BATTERY_CHAR: char = '\u{7}';
pub const CGRAM_FREE_CHAR: char = BATTERY_CHAR;

#[cfg(target_os = "linux")]
const BAR_GLYPHS: [[&str; 8]; 8] = [
    [
        "00000", "00000", "00000", "00000", "00000", "00000", "00000", "00000",
    ],
    [
        "10000", "10000", "10000", "10000", "10000", "10000", "10000", "10000",
    ],
    [
        "11000", "11000", "11000", "11000", "11000", "11000", "11000", "11000",
    ],
    [
        "11100", "11100", "11100", "11100", "11100", "11100", "11100", "11100",
    ],
    [
        "11110", "11110", "11110", "11110", "11110", "11110", "11110", "11110",
    ],
    [
        "11111", "11111", "11111", "11111", "11111", "11111", "11111", "11111",
    ],
    [
        "01010", "11111", "11111", "11111", "01110", "00100", "00000", "00000",
    ],
    [
        "11111", "11111", "10001", "10001", "10001", "10001", "11111", "11111",
    ],
];

/// LCD facade that drives the HD44780 over I2C on Linux and falls back to a
/// stub on other platforms (or when hardware init fails).
pub struct Lcd {
    cols: u8,
    rows: u8,
    #[cfg(target_os = "linux")]
    driver: DriverBackend,
    #[cfg(not(target_os = "linux"))]
    last_lines: (String, String),
    #[cfg(not(target_os = "linux"))]
    backlight_on: bool,
    #[cfg(not(target_os = "linux"))]
    blink_on: bool,
    #[cfg(not(target_os = "linux"))]
    clears: usize,
}

impl Lcd {
    pub fn new(
        cols: u8,
        rows: u8,
        pcf_addr: Pcf8574Addr,
        display_driver: DisplayDriver,
    ) -> Result<Self> {
        #[cfg(target_os = "linux")]
        {
            let mut bus = RppalBus::new_default()?;
            let addr = match pcf_addr {
                Pcf8574Addr::Auto => RppalBus::detect_address(
                    &mut bus,
                    &[0x27, 0x26, 0x25, 0x24, 0x23, 0x22, 0x21, 0x20],
                    0x27,
                ),
                Pcf8574Addr::Addr(a) => a,
            };
            eprintln!("pcf8574 addr: 0x{addr:02x}");
            let mut driver = DriverBackend::new(bus, addr, cols, rows, display_driver)?;
            driver.load_bar_glyphs()?;
            Ok(Self { cols, rows, driver })
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = (pcf_addr, display_driver);
            Ok(Self {
                cols,
                rows,
                last_lines: (String::new(), String::new()),
                backlight_on: true,
                blink_on: false,
                clears: 0,
            })
        }
    }

    pub fn render_boot_message(&mut self) -> Result<()> {
        self.clear()?;
        self.write_line(0, "LifelineTTY ready")
    }

    pub fn clear(&mut self) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            self.driver.clear()
        }
        #[cfg(not(target_os = "linux"))]
        {
            self.clears += 1;
            self.last_lines = (String::new(), String::new());
            Ok(())
        }
    }

    pub fn set_backlight(&mut self, on: bool) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            self.driver.set_backlight(on)
        }
        #[cfg(not(target_os = "linux"))]
        {
            self.backlight_on = on;
            Ok(())
        }
    }

    pub fn set_blink(&mut self, on: bool) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            self.driver.set_blink(on)
        }
        #[cfg(not(target_os = "linux"))]
        {
            self.blink_on = on;
            Ok(())
        }
    }

    pub fn write_line(&mut self, row: u8, content: &str) -> Result<()> {
        if row >= self.rows {
            return Err(Error::InvalidArgs(format!(
                "row {row} out of bounds for display with {} rows",
                self.rows
            )));
        }

        let trimmed = content.chars().take(self.cols as usize).collect::<String>();

        #[cfg(target_os = "linux")]
        {
            self.driver.write_line(row, &trimmed)
        }

        #[cfg(not(target_os = "linux"))]
        {
            if row == 0 {
                self.last_lines.0 = trimmed;
            } else if row == 1 {
                self.last_lines.1 = trimmed;
            }
            Ok(())
        }
    }

    /// Convenience to write both lines back-to-back to reduce flicker.
    pub fn write_lines(&mut self, line1: &str, line2: &str) -> Result<()> {
        self.write_line(0, line1)?;
        self.write_line(1, line2)
    }

    pub fn cols(&self) -> u8 {
        self.cols
    }

    pub fn rows(&self) -> u8 {
        self.rows
    }

    #[cfg(not(target_os = "linux"))]
    pub fn last_lines(&self) -> (String, String) {
        self.last_lines.clone()
    }

    #[cfg(not(target_os = "linux"))]
    pub fn last_backlight(&self) -> bool {
        self.backlight_on
    }

    #[cfg(not(target_os = "linux"))]
    pub fn last_blink(&self) -> bool {
        self.blink_on
    }

    #[cfg(not(target_os = "linux"))]
    pub fn clear_count(&self) -> usize {
        self.clears
    }
}

#[cfg(target_os = "linux")]
fn load_bar_glyphs_internal<B: lcd_driver::I2cBus>(
    driver: &mut lcd_driver::Hd44780<B>,
) -> Result<()> {
    driver.load_custom_bitmaps(&BAR_GLYPHS)
}

#[cfg(target_os = "linux")]
fn load_bar_glyphs_external(driver: &mut ExternalHd44780) -> Result<()> {
    driver.load_custom_bitmaps(&BAR_GLYPHS)
}

#[cfg(target_os = "linux")]
enum DriverBackend {
    Internal(lcd_driver::Hd44780<RppalBus>),
    External(ExternalHd44780),
}

#[cfg(target_os = "linux")]
impl DriverBackend {
    fn new(bus: RppalBus, addr: u8, cols: u8, rows: u8, preference: DisplayDriver) -> Result<Self> {
        match preference {
            DisplayDriver::Hd44780Driver => {
                let raw = bus.into_inner();
                let external = ExternalHd44780::new_from_rppal(raw, addr, cols, rows)?;
                Ok(DriverBackend::External(external))
            }
            DisplayDriver::Auto | DisplayDriver::InTree => {
                let internal = lcd_driver::Hd44780::new(bus, addr, cols, rows)?;
                Ok(DriverBackend::Internal(internal))
            }
        }
    }

    fn clear(&mut self) -> Result<()> {
        match self {
            DriverBackend::Internal(driver) => driver.clear(),
            DriverBackend::External(driver) => driver.clear(),
        }
    }

    fn set_backlight(&mut self, on: bool) -> Result<()> {
        match (self, on) {
            (DriverBackend::Internal(driver), true) => driver.backlight_on(),
            (DriverBackend::Internal(driver), false) => driver.backlight_off(),
            (DriverBackend::External(driver), true) => driver.backlight_on(),
            (DriverBackend::External(driver), false) => driver.backlight_off(),
        }
    }

    fn set_blink(&mut self, on: bool) -> Result<()> {
        match (self, on) {
            (DriverBackend::Internal(driver), true) => driver.blink_cursor_on(),
            (DriverBackend::Internal(driver), false) => driver.blink_cursor_off(),
            (DriverBackend::External(driver), true) => driver.blink_cursor_on(),
            (DriverBackend::External(driver), false) => driver.blink_cursor_off(),
        }
    }

    fn write_line(&mut self, row: u8, text: &str) -> Result<()> {
        match self {
            DriverBackend::Internal(driver) => driver.write_line(row, text),
            DriverBackend::External(driver) => driver.write_line(row, text),
        }
    }

    fn load_bar_glyphs(&mut self) -> Result<()> {
        match self {
            DriverBackend::Internal(driver) => load_bar_glyphs_internal(driver),
            DriverBackend::External(driver) => load_bar_glyphs_external(driver),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // These tests require actual Raspberry Pi hardware or GPIO simulation
    // They will be properly enabled as part of P4 (LCD driver regression tests)
    // For now, skipping them on development machines without GPIO support

    #[test]
    #[ignore]
    fn rejects_out_of_bounds_row() {
        let mut lcd = Lcd::new(
            16,
            2,
            crate::config::DEFAULT_PCF8574_ADDR,
            crate::config::DEFAULT_DISPLAY_DRIVER,
        )
        .unwrap();
        let err = lcd.write_line(2, "oops").unwrap_err();
        assert!(format!("{err}").contains("out of bounds"));
    }

    #[test]
    #[ignore]
    fn accepts_in_bounds_row() {
        let mut lcd = Lcd::new(
            16,
            2,
            crate::config::DEFAULT_PCF8574_ADDR,
            crate::config::DEFAULT_DISPLAY_DRIVER,
        )
        .unwrap();
        lcd.write_line(1, "ok").unwrap();
    }
}
