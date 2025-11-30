use crate::{config::Pcf8574Addr, Error, Result};

#[cfg(target_os = "linux")]
use crate::lcd_driver::{self, pcf8574::RppalBus};

pub const BAR_LEVELS: [char; 6] = ['\u{0}', '\u{1}', '\u{2}', '\u{3}', '\u{4}', '\u{5}'];
pub const BAR_EMPTY: char = BAR_LEVELS[0];
pub const BAR_FULL: char = BAR_LEVELS[5];
pub const HEARTBEAT_CHAR: char = '\u{6}';
pub const BATTERY_CHAR: char = '\u{7}';
pub const CGRAM_FREE_CHAR: char = BATTERY_CHAR;

/// LCD facade that drives the HD44780 over I2C on Linux and falls back to a
/// stub on other platforms (or when hardware init fails).
pub struct Lcd {
    cols: u8,
    rows: u8,
    #[cfg(target_os = "linux")]
    driver: lcd_driver::Hd44780<RppalBus>,
    #[cfg(target_os = "linux")]
    addr: u8,
}

impl Lcd {
    pub fn new(cols: u8, rows: u8, pcf_addr: Pcf8574Addr) -> Result<Self> {
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
            let mut driver = lcd_driver::Hd44780::new(bus, addr, cols, rows)?;
            load_bar_glyphs(&mut driver)?;
            return Ok(Self { cols, rows, driver, addr });
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = pcf_addr;
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
}

#[cfg(target_os = "linux")]
fn load_bar_glyphs<B: lcd_driver::I2cBus>(driver: &mut lcd_driver::Hd44780<B>) -> Result<()> {
    // 0-5: progressive bar fill (0 empty -> 5 full), 6: heartbeat, 7: battery
    let glyphs = [
        ["00000", "00000", "00000", "00000", "00000", "00000", "00000", "00000"], // empty
        ["10000", "10000", "10000", "10000", "10000", "10000", "10000", "10000"], // 20%
        ["11000", "11000", "11000", "11000", "11000", "11000", "11000", "11000"], // 40%
        ["11100", "11100", "11100", "11100", "11100", "11100", "11100", "11100"], // 60%
        ["11110", "11110", "11110", "11110", "11110", "11110", "11110", "11110"], // 80%
        ["11111", "11111", "11111", "11111", "11111", "11111", "11111", "11111"], // 100%
        [
            "01010",
            "11111",
            "11111",
            "11111",
            "01110",
            "00100",
            "00000",
            "00000",
        ], // heartbeat
        [
            "11111",
            "11111",
            "10001",
            "10001",
            "10001",
            "10001",
            "11111",
            "11111",
        ], // battery
    ];
    driver.load_custom_bitmaps(&glyphs)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_out_of_bounds_row() {
        let mut lcd = Lcd::new(16, 2, crate::config::DEFAULT_PCF8574_ADDR).unwrap();
        let err = lcd.write_line(2, "oops").unwrap_err();
        assert!(format!("{err}").contains("out of bounds"));
    }

    #[test]
    fn accepts_in_bounds_row() {
        let mut lcd = Lcd::new(16, 2, crate::config::DEFAULT_PCF8574_ADDR).unwrap();
        lcd.write_line(1, "ok").unwrap();
    }
}
