//! External HD44780 driver that leverages the `hd44780-driver` crate when running on Linux.

use crate::{Error, Result};

#[cfg(target_os = "linux")]
use {
    crate::lcd_driver::pcf8574,
    embedded_hal::blocking::{delay::DelayUs, i2c::Write},
    embedded_hal_1::i2c::{I2c as EmbeddedHal1I2c, SevenBitAddress},
    hd44780_driver::{
        bus::{DataBus, I2CBus},
        Cursor, CursorBlink, HD44780,
    },
    linux_embedded_hal::I2cdev,
    std::{
        sync::{Arc, Mutex},
        thread,
        time::Duration,
    },
};

#[cfg(target_os = "linux")]
pub struct ExternalHd44780 {
    inner: HD44780<I2CBus<BacklightAwareAdapter>>,
    adapter_state: Arc<Mutex<AdapterState>>,
    addr: u8,
    cols: u8,
    rows: u8,
    cursor_x: u8,
    cursor_y: u8,
    implied_newline: bool,
}

#[cfg(not(target_os = "linux"))]
pub struct ExternalHd44780;

#[cfg(target_os = "linux")]
impl ExternalHd44780 {
    pub fn new_from_rppal(bus: rppal::i2c::I2c, addr: u8, cols: u8, rows: u8) -> Result<Self> {
        Self::new_with_state(AdapterState::new_rppal(bus), addr, cols, rows)
    }

    pub fn new_from_i2cdev(bus: I2cdev, addr: u8, cols: u8, rows: u8) -> Result<Self> {
        Self::new_with_state(AdapterState::new_i2cdev(bus), addr, cols, rows)
    }

    fn new_with_state(state: AdapterState, addr: u8, cols: u8, rows: u8) -> Result<Self> {
        let state = Arc::new(Mutex::new(state));
        let mut delay = ThreadDelay;
        let adapter = BacklightAwareAdapter::from_state(state.clone());
        let inner = HD44780::new_i2c(adapter, addr, &mut delay).map_err(map_hd_error)?;
        Ok(Self {
            inner,
            adapter_state: state,
            addr,
            cols: cols.min(40),
            rows: rows.min(4),
            cursor_x: 0,
            cursor_y: 0,
            implied_newline: false,
        })
    }

    pub fn clear(&mut self) -> Result<()> {
        let mut delay = ThreadDelay;
        self.inner.clear(&mut delay).map_err(map_hd_error)?;
        self.cursor_x = 0;
        self.cursor_y = 0;
        Ok(())
    }

    pub fn backlight_on(&mut self) -> Result<()> {
        self.set_backlight_state(true)?;
        self.refresh_backlight()
    }

    pub fn backlight_off(&mut self) -> Result<()> {
        self.set_backlight_state(false)?;
        self.refresh_backlight()
    }

    pub fn blink_cursor_on(&mut self) -> Result<()> {
        let mut delay = ThreadDelay;
        self.inner
            .set_cursor_visibility(Cursor::Visible, &mut delay)
            .map_err(map_hd_error)?;
        self.inner
            .set_cursor_blink(CursorBlink::On, &mut delay)
            .map_err(map_hd_error)
    }

    pub fn blink_cursor_off(&mut self) -> Result<()> {
        let mut delay = ThreadDelay;
        self.inner
            .set_cursor_blink(CursorBlink::Off, &mut delay)
            .map_err(map_hd_error)?;
        self.inner
            .set_cursor_visibility(Cursor::Invisible, &mut delay)
            .map_err(map_hd_error)
    }

    pub fn write_line(&mut self, row: u8, text: &str) -> Result<()> {
        self.move_to(0, row)?;
        self.putstr(text)
    }

    pub fn load_custom_bitmap(&mut self, location: u8, rows: [&str; 8]) -> Result<()> {
        let mut pattern = [0u8; 8];
        for (idx, row) in rows.iter().enumerate() {
            pattern[idx] = super::parse_bitmap_row(row)?;
        }
        self.custom_char(location, &pattern)
    }

    pub fn load_custom_bitmaps(&mut self, bitmaps: &[[&str; 8]]) -> Result<()> {
        for (idx, rows) in bitmaps.iter().enumerate().take(8) {
            self.load_custom_bitmap(idx as u8, *rows)?;
        }
        Ok(())
    }

    pub fn custom_char(&mut self, location: u8, pattern: &[u8; 8]) -> Result<()> {
        self.write_cgram(location, pattern)?;
        self.move_to(self.cursor_x, self.cursor_y)
    }

    fn set_backlight_state(&self, on: bool) -> Result<()> {
        let mut guard = self
            .adapter_state
            .lock()
            .map_err(|_| Error::Io(std::io::Error::other("i2c mutex poisoned")))?;
        guard.backlight_on = on;
        Ok(())
    }

    fn refresh_backlight(&self) -> Result<()> {
        let mut adapter = BacklightAwareAdapter::from_state(self.adapter_state.clone());
        adapter.write(self.addr, &[0]).map_err(|e| e)
    }

    fn move_to(&mut self, cursor_x: u8, cursor_y: u8) -> Result<()> {
        if cursor_y >= self.rows.max(1) {
            return Err(Error::InvalidArgs(format!(
                "row {cursor_y} out of bounds for display with {} rows",
                self.rows
            )));
        }
        self.cursor_x = cursor_x;
        self.cursor_y = cursor_y;
        let mut addr = cursor_x & 0x3f;
        if cursor_y & 1 == 1 {
            addr += 0x40;
        }
        if cursor_y & 2 == 2 {
            addr += self.cols;
        }
        self.set_ddram_address(addr)
    }

    fn set_ddram_address(&mut self, addr: u8) -> Result<()> {
        let mut delay = ThreadDelay;
        self.inner
            .set_cursor_pos(addr, &mut delay)
            .map_err(map_hd_error)
    }

    fn putstr(&mut self, text: &str) -> Result<()> {
        for ch in text.chars() {
            self.putchar(ch)?;
        }
        Ok(())
    }

    fn putchar(&mut self, ch: char) -> Result<()> {
        if ch == '\n' {
            if self.implied_newline {
                self.implied_newline = false;
            } else {
                self.cursor_x = self.cols;
            }
        } else {
            self.write_data(ch as u8)?;
            self.cursor_x += 1;
            self.implied_newline = false;
        }

        if self.cursor_x >= self.cols {
            self.cursor_x = 0;
            self.cursor_y = (self.cursor_y + 1) % self.rows.max(1);
            self.implied_newline = ch != '\n';
            self.move_to(self.cursor_x, self.cursor_y)?;
        } else {
            self.implied_newline = false;
        }
        Ok(())
    }

    fn write_data(&mut self, data: u8) -> Result<()> {
        let mut delay = ThreadDelay;
        self.inner
            .write_byte(data, &mut delay)
            .map_err(map_hd_error)
    }

    fn write_cgram(&mut self, location: u8, pattern: &[u8; 8]) -> Result<()> {
        let mut delay = ThreadDelay;
        let adapter = BacklightAwareAdapter::from_state(self.adapter_state.clone());
        let mut bus = I2CBus::new(adapter, self.addr);
        bus.write(
            super::LCD_CGRAM | ((location & 0x7) << 3),
            false,
            &mut delay,
        )
        .map_err(map_hd_error)?;
        for byte in pattern {
            bus.write(*byte, true, &mut delay).map_err(map_hd_error)?;
        }
        Ok(())
    }
}

#[cfg(not(target_os = "linux"))]
impl ExternalHd44780 {
    pub fn new_from_rppal(_bus: rppal::i2c::I2c, _addr: u8, _cols: u8, _rows: u8) -> Result<Self> {
        Err(Error::InvalidArgs(
            "external hd44780 driver is only supported on Linux".into(),
        ))
    }
}

#[cfg(target_os = "linux")]
#[derive(Clone)]
struct BacklightAwareAdapter {
    state: Arc<Mutex<AdapterState>>,
}

#[cfg(target_os = "linux")]
impl BacklightAwareAdapter {
    fn from_state(state: Arc<Mutex<AdapterState>>) -> Self {
        Self { state }
    }

    fn backlight_mask(on: bool) -> u8 {
        if on {
            1 << super::SHIFT_BACKLIGHT
        } else {
            0
        }
    }
}

#[cfg(target_os = "linux")]
impl Write for BacklightAwareAdapter {
    type Error = Error;

    fn write(&mut self, addr: u8, bytes: &[u8]) -> std::result::Result<(), Self::Error> {
        let mut guard = self
            .state
            .lock()
            .map_err(|_| Error::Io(std::io::Error::other("i2c mutex poisoned")))?;
        if bytes.is_empty() {
            return Ok(());
        }
        let mask = Self::backlight_mask(guard.backlight_on);
        let mut buffer = Vec::with_capacity(bytes.len());
        for &byte in bytes {
            buffer.push((byte & !(1 << super::SHIFT_BACKLIGHT)) | mask);
        }
        guard.write(addr, &buffer)
    }
}

#[cfg(target_os = "linux")]
enum AdapterBackend {
    Rppal(rppal::i2c::I2c),
    I2cdev(I2cdev),
    #[cfg(test)]
    Mock(MockBackend),
}

#[cfg(target_os = "linux")]
struct AdapterState {
    backend: AdapterBackend,
    backlight_on: bool,
}

#[cfg(target_os = "linux")]
impl AdapterState {
    fn new_rppal(bus: rppal::i2c::I2c) -> Self {
        Self {
            backend: AdapterBackend::Rppal(bus),
            backlight_on: true,
        }
    }

    fn new_i2cdev(bus: I2cdev) -> Self {
        Self {
            backend: AdapterBackend::I2cdev(bus),
            backlight_on: true,
        }
    }

    #[cfg(test)]
    fn new_mock(mock: MockBackend) -> Self {
        Self {
            backend: AdapterBackend::Mock(mock),
            backlight_on: true,
        }
    }

    fn write(&mut self, addr: u8, bytes: &[u8]) -> Result<()> {
        match &mut self.backend {
            AdapterBackend::Rppal(bus) => {
                bus.set_slave_address(addr.into())
                    .map_err(pcf8574::map_i2c_err)?;
                let (first, rest) = bytes.split_first().unwrap();
                bus.block_write(*first, rest).map_err(pcf8574::map_i2c_err)
            }
            AdapterBackend::I2cdev(dev) => {
                EmbeddedHal1I2c::<SevenBitAddress>::write(dev, addr.into(), bytes)
                    .map_err(pcf8574::map_i2cdev_err)
            }
            #[cfg(test)]
            AdapterBackend::Mock(mock) => mock.write(addr, bytes),
        }
    }

    #[cfg(test)]
    fn take_mock_writes(&mut self) -> Vec<(u8, Vec<u8>)> {
        match &mut self.backend {
            AdapterBackend::Mock(mock) => mock.take_writes(),
            _ => Vec::new(),
        }
    }
}

#[cfg(test)]
#[derive(Default, Clone)]
struct MockBackend {
    writes: Vec<(u8, Vec<u8>)>,
}

#[cfg(test)]
impl MockBackend {
    fn write(&mut self, addr: u8, bytes: &[u8]) -> Result<()> {
        self.writes.push((addr, bytes.to_vec()));
        Ok(())
    }

    fn take_writes(&mut self) -> Vec<(u8, Vec<u8>)> {
        std::mem::take(&mut self.writes)
    }
}

#[cfg(target_os = "linux")]
struct ThreadDelay;

#[cfg(target_os = "linux")]
impl DelayUs<u16> for ThreadDelay {
    fn delay_us(&mut self, us: u16) {
        thread::sleep(Duration::from_micros(us as u64));
    }
}

#[cfg(target_os = "linux")]
impl embedded_hal::blocking::delay::DelayMs<u8> for ThreadDelay {
    fn delay_ms(&mut self, ms: u8) {
        thread::sleep(Duration::from_millis(ms as u64));
    }
}

#[cfg(target_os = "linux")]
fn map_hd_error(_err: hd44780_driver::error::Error) -> Error {
    Error::Io(std::io::Error::other("hd44780-driver error"))
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::*;

    #[test]
    fn adapter_sets_backlight_bit_when_on() {
        let state = Arc::new(Mutex::new(AdapterState::new_mock(MockBackend::default())));
        state.lock().unwrap().backlight_on = true;
        let mut adapter = BacklightAwareAdapter::from_state(state.clone());
        adapter.write(0x27, &[0x00]).unwrap();
        let mut guard = state.lock().unwrap();
        let writes = guard.take_mock_writes();
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0].0, 0x27);
        assert!(writes[0].1[0] & (1 << crate::lcd_driver::SHIFT_BACKLIGHT) != 0);
    }

    #[test]
    fn adapter_clears_backlight_bit_when_off() {
        let state = Arc::new(Mutex::new(AdapterState::new_mock(MockBackend::default())));
        state.lock().unwrap().backlight_on = false;
        let mut adapter = BacklightAwareAdapter::from_state(state.clone());
        adapter.write(0x27, &[0xFF]).unwrap();
        let mut guard = state.lock().unwrap();
        let writes = guard.take_mock_writes();
        assert_eq!(writes.len(), 1);
        assert_eq!(writes[0].0, 0x27);
        assert_eq!(
            writes[0].1[0] & (1 << crate::lcd_driver::SHIFT_BACKLIGHT),
            0
        );
    }
}
