use crate::{
    config::{DisplayDriver, Pcf8574Addr},
    Error, Result,
};

#[cfg(target_os = "linux")]
use crate::lcd_driver::{
    self,
    external::ExternalHd44780,
    pcf8574::{I2cdevBus, RppalBus},
};
#[cfg(target_os = "linux")]
use linux_embedded_hal::I2cdev;
#[cfg(target_os = "linux")]
use rppal::i2c::I2c as RppalI2c;

pub const BAR_LEVELS: [char; 6] = ['\u{0}', '\u{1}', '\u{2}', '\u{3}', '\u{4}', '\u{5}'];
pub const BAR_EMPTY: char = BAR_LEVELS[0];
pub const BAR_FULL: char = BAR_LEVELS[5];
pub const HEARTBEAT_CHAR: char = '\u{6}';
pub const BATTERY_CHAR: char = '\u{7}';
pub const CGRAM_FREE_CHAR: char = BATTERY_CHAR;
pub const WIFI_CHAR: char = 'w';

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

#[cfg(target_os = "linux")]
const PCF8574_ADDR_CANDIDATES: [u8; 8] = [0x27, 0x26, 0x25, 0x24, 0x23, 0x22, 0x21, 0x20];

#[cfg(target_os = "linux")]
const I2CDEV_PATHS: [&str; 2] = ["/dev/i2c-1", "/dev/i2c-0"];

struct StubState {
    last_lines: (String, String),
    backlight_on: bool,
    blink_on: bool,
    clears: usize,
    custom_chars: [[u8; 8]; 8],
}

impl StubState {
    fn new() -> Self {
        Self {
            last_lines: (String::new(), String::new()),
            backlight_on: true,
            blink_on: false,
            clears: 0,
            custom_chars: [[0u8; 8]; 8],
        }
    }

    fn clear(&mut self) -> Result<()> {
        self.clears = self.clears.saturating_add(1);
        self.last_lines = (String::new(), String::new());
        Ok(())
    }

    fn set_backlight(&mut self, on: bool) -> Result<()> {
        self.backlight_on = on;
        Ok(())
    }

    fn set_blink(&mut self, on: bool) -> Result<()> {
        self.blink_on = on;
        Ok(())
    }

    fn write_line(&mut self, row: u8, line: &str) -> Result<()> {
        match row {
            0 => self.last_lines.0 = line.to_string(),
            1 => self.last_lines.1 = line.to_string(),
            _ => (),
        }
        Ok(())
    }

    fn custom_char(&mut self, slot: u8, bitmap: &[u8; 8]) -> Result<()> {
        let idx = (slot as usize).min(self.custom_chars.len().saturating_sub(1));
        self.custom_chars[idx] = *bitmap;
        Ok(())
    }
}

#[cfg(target_os = "linux")]
pub enum LcdBus {
    Rppal(RppalI2c),
    I2cdev(I2cdev),
}

/// LCD facade that drives the HD44780 over I2C on Linux and falls back to a
/// stub on other platforms (or when hardware init fails).
pub struct Lcd {
    cols: u8,
    rows: u8,
    stub: StubState,
    #[cfg(target_os = "linux")]
    driver: Option<DriverBackend>,
}

impl Lcd {
    pub fn new_stub(cols: u8, rows: u8) -> Self {
        Self {
            cols,
            rows,
            stub: StubState::new(),
            #[cfg(target_os = "linux")]
            driver: None,
        }
    }

    pub fn new(
        cols: u8,
        rows: u8,
        pcf_addr: Pcf8574Addr,
        display_driver: DisplayDriver,
    ) -> Result<Self> {
        #[cfg(target_os = "linux")]
        {
            let stub = StubState::new();
            match DriverBackend::new(cols, rows, pcf_addr, display_driver) {
                Ok((mut driver, addr)) => {
                    eprintln!("pcf8574 addr: 0x{addr:02x}");
                    driver.load_bar_glyphs()?;
                    Ok(Self {
                        cols,
                        rows,
                        stub,
                        driver: Some(driver),
                    })
                }
                Err(err) => {
                    eprintln!("warning: lcd init failed ({err}); falling back to stub display");
                    Ok(Self {
                        cols,
                        rows,
                        stub,
                        driver: None,
                    })
                }
            }
        }

        #[cfg(not(target_os = "linux"))]
        {
            let _ = (pcf_addr, display_driver);
            Ok(Self {
                cols,
                rows,
                stub: StubState::new(),
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
            if let Some(driver) = &mut self.driver {
                return driver.clear();
            }
        }
        self.stub.clear()
    }

    pub fn set_backlight(&mut self, on: bool) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            if let Some(driver) = &mut self.driver {
                return driver.set_backlight(on);
            }
        }
        self.stub.set_backlight(on)
    }

    pub fn set_blink(&mut self, on: bool) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            if let Some(driver) = &mut self.driver {
                return driver.set_blink(on);
            }
        }
        self.stub.set_blink(on)
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
            if let Some(driver) = &mut self.driver {
                return driver.write_line(row, &trimmed);
            }
        }
        self.stub.write_line(row, &trimmed)
    }

    /// Convenience to write both lines back-to-back to reduce flicker.
    pub fn write_lines(&mut self, line1: &str, line2: &str) -> Result<()> {
        self.write_line(0, line1)?;
        self.write_line(1, line2)
    }

    pub(crate) fn write_custom_char(&mut self, slot: u8, bitmap: &[u8; 8]) -> Result<()> {
        #[cfg(target_os = "linux")]
        {
            if let Some(driver) = &mut self.driver {
                return driver.custom_char(slot, bitmap);
            }
        }
        self.stub.custom_char(slot, bitmap)
    }

    pub fn cols(&self) -> u8 {
        self.cols
    }

    pub fn rows(&self) -> u8 {
        self.rows
    }

    #[cfg(target_os = "linux")]
    pub fn new_with_bus(
        cols: u8,
        rows: u8,
        addr: u8,
        display_driver: DisplayDriver,
        bus: LcdBus,
    ) -> Result<Self> {
        let mut driver = match bus {
            LcdBus::Rppal(raw) => DriverBackend::from_rppal_bus(
                RppalBus::from_inner(raw),
                addr,
                cols,
                rows,
                display_driver,
            )?,
            LcdBus::I2cdev(dev) => DriverBackend::from_i2cdev_bus(
                I2cdevBus::from_inner(dev),
                addr,
                cols,
                rows,
                display_driver,
            )?,
        };
        driver.load_bar_glyphs()?;
        Ok(Self {
            cols,
            rows,
            stub: StubState::new(),
            driver: Some(driver),
        })
    }

    pub fn last_lines(&self) -> (String, String) {
        self.stub.last_lines.clone()
    }

    pub fn last_backlight(&self) -> bool {
        self.stub.backlight_on
    }

    pub fn last_blink(&self) -> bool {
        self.stub.blink_on
    }

    pub fn clear_count(&self) -> usize {
        self.stub.clears
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
    Internal(InternalDriver),
    External(ExternalHd44780),
}

#[cfg(target_os = "linux")]
enum InternalDriver {
    Rppal(lcd_driver::Hd44780<RppalBus>),
    I2cdev(lcd_driver::Hd44780<I2cdevBus>),
}

#[cfg(target_os = "linux")]
impl InternalDriver {
    fn from_rppal(bus: RppalBus, addr: u8, cols: u8, rows: u8) -> Result<Self> {
        let driver = lcd_driver::Hd44780::new(bus, addr, cols, rows)?;
        Ok(Self::Rppal(driver))
    }

    fn from_i2cdev(bus: I2cdevBus, addr: u8, cols: u8, rows: u8) -> Result<Self> {
        let driver = lcd_driver::Hd44780::new(bus, addr, cols, rows)?;
        Ok(Self::I2cdev(driver))
    }

    fn clear(&mut self) -> Result<()> {
        match self {
            InternalDriver::Rppal(driver) => driver.clear(),
            InternalDriver::I2cdev(driver) => driver.clear(),
        }
    }

    fn set_backlight(&mut self, on: bool) -> Result<()> {
        match self {
            InternalDriver::Rppal(driver) => {
                if on {
                    driver.backlight_on()
                } else {
                    driver.backlight_off()
                }
            }
            InternalDriver::I2cdev(driver) => {
                if on {
                    driver.backlight_on()
                } else {
                    driver.backlight_off()
                }
            }
        }
    }

    fn set_blink(&mut self, on: bool) -> Result<()> {
        match self {
            InternalDriver::Rppal(driver) => {
                if on {
                    driver.blink_cursor_on()
                } else {
                    driver.blink_cursor_off()
                }
            }
            InternalDriver::I2cdev(driver) => {
                if on {
                    driver.blink_cursor_on()
                } else {
                    driver.blink_cursor_off()
                }
            }
        }
    }

    fn write_line(&mut self, row: u8, text: &str) -> Result<()> {
        match self {
            InternalDriver::Rppal(driver) => driver.write_line(row, text),
            InternalDriver::I2cdev(driver) => driver.write_line(row, text),
        }
    }

    fn load_bar_glyphs(&mut self) -> Result<()> {
        match self {
            InternalDriver::Rppal(driver) => load_bar_glyphs_internal(driver),
            InternalDriver::I2cdev(driver) => load_bar_glyphs_internal(driver),
        }
    }

    fn custom_char(&mut self, slot: u8, bitmap: &[u8; 8]) -> Result<()> {
        match self {
            InternalDriver::Rppal(driver) => driver.custom_char(slot, bitmap),
            InternalDriver::I2cdev(driver) => driver.custom_char(slot, bitmap),
        }
    }
}

#[cfg(target_os = "linux")]
impl DriverBackend {
    fn new(
        cols: u8,
        rows: u8,
        pcf_addr: Pcf8574Addr,
        preference: DisplayDriver,
    ) -> Result<(Self, u8)> {
        match Self::new_with_rppal(cols, rows, pcf_addr.clone(), preference) {
            Ok(tuple) => Ok(tuple),
            Err(primary_err) => {
                eprintln!(
                    "warning: rppal I2C init failed ({primary_err}); trying linux-embedded-hal"
                );
                match Self::new_with_i2cdev(cols, rows, pcf_addr, preference) {
                    Ok(tuple) => Ok(tuple),
                    Err(fallback_err) => Err(Error::Io(std::io::Error::new(
                        std::io::ErrorKind::Other,
                        format!("lcd init failed: {primary_err}; fallback: {fallback_err}"),
                    ))),
                }
            }
        }
    }

    fn from_rppal_bus(
        bus: RppalBus,
        addr: u8,
        cols: u8,
        rows: u8,
        preference: DisplayDriver,
    ) -> Result<Self> {
        match preference {
            DisplayDriver::Hd44780Driver => {
                let raw = bus.into_inner();
                let external = ExternalHd44780::new_from_rppal(raw, addr, cols, rows)?;
                Ok(DriverBackend::External(external))
            }
            DisplayDriver::Auto | DisplayDriver::InTree => {
                let internal = InternalDriver::from_rppal(bus, addr, cols, rows)?;
                Ok(DriverBackend::Internal(internal))
            }
        }
    }

    fn from_i2cdev_bus(
        bus: I2cdevBus,
        addr: u8,
        cols: u8,
        rows: u8,
        preference: DisplayDriver,
    ) -> Result<Self> {
        match preference {
            DisplayDriver::Hd44780Driver => {
                let raw = bus.into_inner();
                let external = ExternalHd44780::new_from_i2cdev(raw, addr, cols, rows)?;
                Ok(DriverBackend::External(external))
            }
            DisplayDriver::Auto | DisplayDriver::InTree => {
                let internal = InternalDriver::from_i2cdev(bus, addr, cols, rows)?;
                Ok(DriverBackend::Internal(internal))
            }
        }
    }

    fn new_with_rppal(
        cols: u8,
        rows: u8,
        pcf_addr: Pcf8574Addr,
        preference: DisplayDriver,
    ) -> Result<(Self, u8)> {
        let mut bus = RppalBus::new_default()?;
        let addr = match pcf_addr {
            Pcf8574Addr::Auto => bus.detect_address(&PCF8574_ADDR_CANDIDATES, 0x27),
            Pcf8574Addr::Addr(addr) => addr,
        };
        let backend = Self::from_rppal_bus(bus, addr, cols, rows, preference)?;
        Ok((backend, addr))
    }

    fn new_with_i2cdev(
        cols: u8,
        rows: u8,
        pcf_addr: Pcf8574Addr,
        preference: DisplayDriver,
    ) -> Result<(Self, u8)> {
        let mut bus = Self::open_i2cdev_bus()?;
        let addr = match pcf_addr {
            Pcf8574Addr::Auto => bus.detect_address(&PCF8574_ADDR_CANDIDATES, 0x27),
            Pcf8574Addr::Addr(addr) => addr,
        };
        let backend = Self::from_i2cdev_bus(bus, addr, cols, rows, preference)?;
        Ok((backend, addr))
    }

    fn open_i2cdev_bus() -> Result<I2cdevBus> {
        let mut last_error: Option<Error> = None;
        for path in I2CDEV_PATHS {
            match I2cdevBus::from_path(path) {
                Ok(bus) => return Ok(bus),
                Err(err) => last_error = Some(err),
            }
        }
        Err(last_error
            .unwrap_or_else(|| Error::InvalidArgs("no accessible i2c-dev bus found".into())))
    }

    fn clear(&mut self) -> Result<()> {
        match self {
            DriverBackend::Internal(driver) => driver.clear(),
            DriverBackend::External(driver) => driver.clear(),
        }
    }

    fn set_backlight(&mut self, on: bool) -> Result<()> {
        match (self, on) {
            (DriverBackend::Internal(driver), _) => driver.set_backlight(on),
            (DriverBackend::External(driver), true) => driver.backlight_on(),
            (DriverBackend::External(driver), false) => driver.backlight_off(),
        }
    }

    fn set_blink(&mut self, on: bool) -> Result<()> {
        match (self, on) {
            (DriverBackend::Internal(driver), _) => driver.set_blink(on),
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
            DriverBackend::Internal(driver) => driver.load_bar_glyphs(),
            DriverBackend::External(driver) => load_bar_glyphs_external(driver),
        }
    }

    fn custom_char(&mut self, slot: u8, bitmap: &[u8; 8]) -> Result<()> {
        match self {
            DriverBackend::Internal(driver) => driver.custom_char(slot, bitmap),
            DriverBackend::External(driver) => driver.custom_char(slot, bitmap),
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
