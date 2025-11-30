use crate::{Error, Result};

#[cfg(target_os = "linux")]
use crate::lcd_driver::{self, pcf8574::RppalBus};

pub const BAR_LEFT: char = '\u{0}';
pub const BAR_FULL: char = '\u{1}';
pub const BAR_EMPTY: char = '\u{2}';
pub const BAR_RIGHT: char = '\u{3}';

/// LCD facade that drives the HD44780 over I2C on Linux and falls back to a
/// stub on other platforms (or when hardware init fails).
pub struct Lcd {
    cols: u8,
    rows: u8,
    #[cfg(target_os = "linux")]
    driver: lcd_driver::Hd44780<RppalBus>,
}

impl Lcd {
    pub fn new(cols: u8, rows: u8) -> Result<Self> {
        #[cfg(target_os = "linux")]
        {
            let (bus, addr) = RppalBus::autodetect_default()?;
            let mut driver = lcd_driver::Hd44780::new(bus, addr, cols, rows)?;
            load_bar_glyphs(&mut driver)?;
            return Ok(Self { cols, rows, driver });
        }

        #[cfg(not(target_os = "linux"))]
        {
            Ok(Self { cols, rows })
        }
    }

    pub fn render_boot_message(&mut self) -> Result<()> {
        self.clear()?;
        self.write_line(0, "SerialLCD ready")
    }

    pub fn clear(&mut self) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            return self.driver.clear();
        }
        #[cfg(not(target_os = "linux"))]
        {
            Ok(())
        }
    }

    pub fn set_backlight(&mut self, on: bool) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            if on {
                self.driver.backlight_on()
            } else {
                self.driver.backlight_off()
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = on;
            Ok(())
        }
    }

    pub fn set_blink(&mut self, on: bool) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            if on {
                self.driver.blink_cursor_on()
            } else {
                self.driver.blink_cursor_off()
            }
        }
        #[cfg(not(target_os = "linux"))]
        {
            let _ = on;
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

        let trimmed = content
            .chars()
            .take(self.cols as usize)
            .collect::<String>();

        #[cfg(target_os = "linux")]
        {
            return self.driver.write_line(row, &trimmed);
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = trimmed;
            Ok(())
        }
    }

    pub fn cols(&self) -> u8 {
        self.cols
    }

    pub fn rows(&self) -> u8 {
        self.rows
    }
}

#[cfg(target_os = "linux")]
fn load_bar_glyphs<B: lcd_driver::I2cBus>(driver: &mut lcd_driver::Hd44780<B>) -> Result<()> {
    // 0: left cap, 1: full block, 2: empty block, 3: right cap
    let glyphs = [
        [
            "11110",
            "10000",
            "10000",
            "10000",
            "10000",
            "10000",
            "10000",
            "11110",
        ],
        ["11111", "11111", "11111", "11111", "11111", "11111", "11111", "11111"],
        [
            "11111",
            "10001",
            "10001",
            "10001",
            "10001",
            "10001",
            "10001",
            "11111",
        ],
        [
            "01111",
            "00001",
            "00001",
            "00001",
            "00001",
            "00001",
            "00001",
            "01111",
        ],
    ];
    driver.load_custom_bitmaps(&glyphs)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_out_of_bounds_row() {
        let mut lcd = Lcd::new(16, 2).unwrap();
        let err = lcd.write_line(2, "oops").unwrap_err();
        assert!(format!("{err}").contains("out of bounds"));
    }

    #[test]
    fn accepts_in_bounds_row() {
        let mut lcd = Lcd::new(16, 2).unwrap();
        lcd.write_line(1, "ok").unwrap();
    }
}
