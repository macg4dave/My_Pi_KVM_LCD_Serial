use crate::Result;

#[cfg(test)]
use crate::Error;
use std::collections::VecDeque;

/// Minimal fake serial port used in tests to script reads/writes.
#[derive(Default)]
pub struct FakeSerialPort {
    script: VecDeque<Result<String>>,
    writes: Vec<String>,
}

impl FakeSerialPort {
    pub fn new(script: Vec<Result<String>>) -> Self {
        Self {
            script: script.into(),
            writes: Vec::new(),
        }
    }

    pub fn send_command_line(&mut self, line: &str) -> Result<()> {
        self.writes.push(line.to_string());
        Ok(())
    }

    pub fn read_message_line(&mut self, line_buffer: &mut String) -> Result<usize> {
        match self.script.pop_front() {
            Some(Ok(line)) => {
                *line_buffer = line;
                Ok(line_buffer.len())
            }
            Some(Err(e)) => Err(e),
            None => Ok(0),
        }
    }

    pub fn writes(&self) -> &[String] {
        &self.writes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fake_serial_scripts_reads_and_writes() {
        let mut fake =
            FakeSerialPort::new(vec![Ok("first\n".into()), Err(Error::Parse("boom".into()))]);
        let mut buf = String::new();
        let read = fake.read_message_line(&mut buf).unwrap();
        assert_eq!(read, "first\n".len());
        assert!(fake.read_message_line(&mut buf).is_err());
        fake.send_command_line("PING").unwrap();
        assert_eq!(fake.writes(), &["PING".to_string()]);
    }
}
