use crate::{Error, Result};

/// Lightweight serial placeholder. Replace with a real transport later.
#[derive(Debug, Clone)]
pub struct SerialPort {
    device: String,
    baud: u32,
}

impl SerialPort {
    pub fn connect(device: &str, baud: u32) -> Result<Self> {
        if device.is_empty() {
            return Err(Error::InvalidArgs(
                "device path cannot be empty".to_string(),
            ));
        }

        // TODO: open and configure the real serial device.
        Ok(Self {
            device: device.to_string(),
            baud,
        })
    }

    pub fn send_line(&self, line: &str) -> Result<()> {
        let _ = (self, line);
        // TODO: write the line (plus framing) to the serial port.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_device() {
        let err = SerialPort::connect("", 9600).unwrap_err();
        assert!(format!("{err}").contains("device path cannot be empty"));
    }

    #[test]
    fn builds_serial_port() {
        let port = SerialPort::connect("/dev/ttyUSB0", 9600).unwrap();
        assert_eq!(port.device, "/dev/ttyUSB0");
        assert_eq!(port.baud, 9600);
    }
}
