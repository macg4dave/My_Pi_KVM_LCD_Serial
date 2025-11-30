use crate::{Error, Result};

/// Minimal placeholder LCD driver.
#[derive(Debug, Clone)]
pub struct Lcd {
    cols: u8,
    rows: u8,
}

impl Lcd {
    pub fn new(cols: u8, rows: u8) -> Self {
        Self { cols, rows }
    }

    pub fn render_boot_message(&mut self) -> Result<()> {
        self.write_line(0, "SerialLCD ready")
    }

    pub fn write_line(&self, row: u8, content: &str) -> Result<()> {
        if row >= self.rows {
            return Err(Error::InvalidArgs(format!(
                "row {row} out of bounds for display with {} rows",
                self.rows
            )));
        }

        let _trimmed = content
            .chars()
            .take(self.cols as usize)
            .collect::<String>();

        // TODO: send trimmed content to the LCD over serial.
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_out_of_bounds_row() {
        let lcd = Lcd::new(16, 2);
        let err = lcd.write_line(2, "oops").unwrap_err();
        assert!(format!("{err}").contains("out of bounds"));
    }

    #[test]
    fn accepts_in_bounds_row() {
        let lcd = Lcd::new(16, 2);
        lcd.write_line(1, "ok").unwrap();
    }
}
