use crate::{state::MAX_FRAME_BYTES, Error, Result};
use std::io;

/// Lightweight serial placeholder. Replace with a real transport later.
#[derive(Debug)]
pub struct SerialPort {
    #[allow(dead_code)]
    device: String,
    #[allow(dead_code)]
    baud: u32,
    port: Option<Box<dyn serialport::SerialPort>>,
}

impl SerialPort {
    pub fn connect(device: &str, baud: u32) -> Result<Self> {
        if device.is_empty() {
            return Err(Error::InvalidArgs(
                "device path cannot be empty".to_string(),
            ));
        }

        let port = serialport::new(device, baud)
            .timeout(std::time::Duration::from_millis(500))
            .open()
            .map_err(|e| Error::Io(std::io::Error::other(e.to_string())))?;

        Ok(Self {
            device: device.to_string(),
            baud,
            port: Some(port),
        })
    }

    /// Send a single newline-terminated command line to the serial port.
    pub fn send_command_line(&mut self, line: &str) -> Result<()> {
        let port = self
            .port
            .as_mut()
            .ok_or_else(|| Error::InvalidArgs("serial port not connected".into()))?;

        let mut buf = line.as_bytes().to_vec();
        buf.push(b'\n');
        port.write_all(&buf)?;
        port.flush()?;
        Ok(())
    }

    /// Read a single newline-terminated message. Returns 0 on timeout.
    pub fn read_message_line(&mut self, line_buffer: &mut String) -> Result<usize> {
        line_buffer.clear();
        let port = self
            .port
            .as_deref_mut()
            .ok_or_else(|| Error::InvalidArgs("serial port not connected".into()))?;

        let mut byte = [0u8; 1];
        let mut total = 0;
        // Read byte-by-byte until newline while enforcing a size guard.
        loop {
            match port.read(&mut byte) {
                Ok(0) => return Ok(total),
                Ok(_) => {
                    total += 1;
                    if total > MAX_FRAME_BYTES {
                        // Drain until newline to avoid contaminating the next frame.
                        while port.read(&mut byte).is_ok() {
                            if byte[0] == b'\n' {
                                break;
                            }
                        }
                        return Err(Error::Parse(format!(
                            "frame exceeds {MAX_FRAME_BYTES} bytes"
                        )));
                    }
                    let b = byte[0];
                    if b == b'\n' {
                        return Ok(total);
                    }
                    if b != b'\r' {
                        line_buffer.push(b as char);
                    }
                }
                Err(e) if e.kind() == io::ErrorKind::TimedOut => return Ok(0),
                Err(e) => return Err(Error::Io(e)),
            }
        }
    }

    /// Provide a temporary reader over the serial port.
    pub fn borrow_reader(&mut self) -> Result<SerialReader<'_>> {
        let port = self
            .port
            .as_deref_mut()
            .ok_or_else(|| Error::InvalidArgs("serial port not connected".into()))?;
        Ok(SerialReader { port })
    }
}

pub struct SerialReader<'a> {
    port: &'a mut dyn serialport::SerialPort,
}

impl<'a> std::io::Read for SerialReader<'a> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        self.port.read(buf)
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
    fn connects_or_returns_io_error() {
        let res = SerialPort::connect("/dev/ttyUSB0", 9600);
        match res {
            Ok(port) => {
                assert_eq!(port.device, "/dev/ttyUSB0");
                assert_eq!(port.baud, 9600);
            }
            Err(Error::Io(_)) => { /* acceptable in test env without device */ }
            Err(other) => panic!("unexpected error: {other}"),
        }
    }
}
