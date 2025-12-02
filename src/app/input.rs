#[cfg(target_os = "linux")]
use std::time::{Duration, Instant};

use crate::{Error, Result};

/// Hardware button wrapper; stubbed on non-Linux platforms.
#[cfg(target_os = "linux")]
pub struct Button {
    pin: rppal::gpio::InputPin,
    last: Instant,
    debounce: Duration,
}

#[cfg(target_os = "linux")]
impl Button {
    pub fn new(pin: Option<u8>) -> Result<Self> {
        let pin = match pin {
            Some(p) => p,
            None => return Err(Error::InvalidArgs("no button pin configured".into())),
        };
        let gpio = rppal::gpio::Gpio::new().map_err(|e| Error::Io(std::io::Error::other(e)))?;
        let input = gpio
            .get(pin)
            .map_err(|e| Error::Io(std::io::Error::other(e)))?
            .into_input_pullup();
        Ok(Self {
            pin: input,
            last: Instant::now(),
            debounce: Duration::from_millis(150),
        })
    }

    pub fn is_pressed(&mut self) -> bool {
        let now = Instant::now();
        if self.pin.is_low() && now.duration_since(self.last) > self.debounce {
            self.last = now;
            true
        } else {
            false
        }
    }
}

#[cfg(not(target_os = "linux"))]
pub struct Button;

#[cfg(not(target_os = "linux"))]
impl Button {
    pub fn new(_pin: Option<u8>) -> Result<Self> {
        Err(Error::InvalidArgs(
            "button unsupported on this platform".into(),
        ))
    }

    pub fn is_pressed(&mut self) -> bool {
        false
    }
}
