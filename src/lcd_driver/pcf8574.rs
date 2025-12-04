use crate::{lcd_driver::I2cBus, Error, Result};
#[cfg(target_os = "linux")]
use embedded_hal_1::i2c::{I2c as EmbeddedHal1I2c, SevenBitAddress};
#[cfg(target_os = "linux")]
use std::path::Path;

#[cfg(target_os = "linux")]
pub(crate) fn map_i2c_err(err: rppal::i2c::Error) -> Error {
    // Wrap rppal errors so the caller sees a standard IO error payload.
    Error::Io(std::io::Error::other(err.to_string()))
}

#[cfg(target_os = "linux")]
pub(crate) fn map_i2cdev_err(err: linux_embedded_hal::I2CError) -> Error {
    Error::Io(std::io::Error::other(err.to_string()))
}

/// Linux implementation using rppal's I2C.
#[cfg(target_os = "linux")]
pub struct RppalBus {
    inner: rppal::i2c::I2c,
}

#[cfg(target_os = "linux")]
impl RppalBus {
    /// Open the default I2C bus (e.g., /dev/i2c-1).
    pub fn new_default() -> Result<Self> {
        let inner = rppal::i2c::I2c::new().map_err(map_i2c_err)?;
        Ok(Self { inner })
    }

    pub fn from_inner(inner: rppal::i2c::I2c) -> Self {
        Self { inner }
    }

    /// Open a specific bus by number (e.g., bus 1 => /dev/i2c-1).
    pub fn new_with_bus(bus: u8) -> Result<Self> {
        let inner = rppal::i2c::I2c::with_bus(bus).map_err(map_i2c_err)?;
        Ok(Self { inner })
    }

    /// Auto-detect a PCF8574 address by probing common backpack ranges (0x20-0x27).
    /// Returns the bus and the detected address (or the fallback if none respond).
    pub fn autodetect_default() -> Result<(Self, u8)> {
        let mut inner = rppal::i2c::I2c::new().map_err(map_i2c_err)?;
        let addr = detect_address(
            &mut inner,
            &[0x27, 0x26, 0x25, 0x24, 0x23, 0x22, 0x21, 0x20],
            0x27,
        );
        Ok((Self { inner }, addr))
    }

    pub fn detect_address(&mut self, candidates: &[u8], fallback: u8) -> u8 {
        detect_address(&mut self.inner, candidates, fallback)
    }

    pub fn into_inner(self) -> rppal::i2c::I2c {
        self.inner
    }
}

#[cfg(target_os = "linux")]
impl I2cBus for RppalBus {
    fn write_byte(&mut self, addr: u8, byte: u8) -> Result<()> {
        self.inner
            .set_slave_address(addr.into())
            .map_err(map_i2c_err)?;
        self.inner.block_write(byte, &[]).map_err(map_i2c_err)
    }
}

/// Linux `I2cdev` implementation so non-Raspberry Pi hosts can exercise the LCD path.
#[cfg(target_os = "linux")]
pub struct I2cdevBus {
    inner: linux_embedded_hal::I2cdev,
}

#[cfg(target_os = "linux")]
impl I2cdevBus {
    pub fn from_path(path: impl AsRef<Path>) -> Result<Self> {
        let inner =
            linux_embedded_hal::I2cdev::new(path).map_err(|err| map_i2cdev_err(err.into()))?;
        Ok(Self { inner })
    }

    pub fn from_inner(inner: linux_embedded_hal::I2cdev) -> Self {
        Self { inner }
    }

    pub fn into_inner(self) -> linux_embedded_hal::I2cdev {
        self.inner
    }

    pub fn detect_address(&mut self, candidates: &[u8], fallback: u8) -> u8 {
        for &addr in candidates {
            if EmbeddedHal1I2c::<SevenBitAddress>::write(&mut self.inner, addr.into(), &[0]).is_ok()
            {
                return addr;
            }
        }
        fallback
    }
}

#[cfg(target_os = "linux")]
impl I2cBus for I2cdevBus {
    fn write_byte(&mut self, addr: u8, byte: u8) -> Result<()> {
        EmbeddedHal1I2c::<SevenBitAddress>::write(&mut self.inner, addr.into(), &[byte])
            .map_err(map_i2cdev_err)
    }
}

/// Non-Linux stub to satisfy builds on dev hosts; returns errors at runtime.
#[cfg(not(target_os = "linux"))]
pub struct RppalBus;

#[cfg(not(target_os = "linux"))]
impl RppalBus {
    pub fn new_default() -> Result<Self> {
        Err(Error::InvalidArgs(
            "RppalBus is only available on Linux targets".into(),
        ))
    }

    pub fn new_with_bus(_bus: u8) -> Result<Self> {
        Err(Error::InvalidArgs(
            "RppalBus is only available on Linux targets".into(),
        ))
    }

    pub fn autodetect_default() -> Result<(Self, u8)> {
        Err(Error::InvalidArgs(
            "RppalBus is only available on Linux targets".into(),
        ))
    }
}

#[cfg(not(target_os = "linux"))]
impl I2cBus for RppalBus {
    fn write_byte(&mut self, _addr: u8, _byte: u8) -> Result<()> {
        Err(Error::InvalidArgs(
            "RppalBus is only available on Linux targets".into(),
        ))
    }
}

#[cfg(target_os = "linux")]
fn detect_address(bus: &mut rppal::i2c::I2c, candidates: &[u8], fallback: u8) -> u8 {
    for &addr in candidates {
        if bus.set_slave_address(addr as u16).is_ok() && bus.block_write(0, &[]).is_ok() {
            return addr;
        }
    }
    fallback
}
