//! HD44780 over PCF8574 driver translated from dhylands/python_lcd.
//! This keeps the HAL split and init sequence from the reference Python code.

use std::time::Duration;

use crate::{Error, Result};

pub mod external;
pub mod pcf8574;

/// Backlight state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Backlight {
    On,
    Off,
}

/// Minimal trait to allow swapping the I2C backend (for tests or rppal).
pub trait I2cBus {
    fn write_byte(&mut self, addr: u8, byte: u8) -> Result<()>;
}

/// HD44780 driver that targets a PCF8574 backpack in 4-bit mode.
pub struct Hd44780<B: I2cBus> {
    bus: B,
    addr: u8,
    cols: u8,
    rows: u8,
    cursor_x: u8,
    cursor_y: u8,
    implied_newline: bool,
    backlight: Backlight,
}

// Bit masks from python_lcd.
const MASK_RS: u8 = 0x01;
#[allow(dead_code)]
const MASK_RW: u8 = 0x02;
const MASK_E: u8 = 0x04;
pub(super) const SHIFT_BACKLIGHT: u8 = 3;
const SHIFT_DATA: u8 = 4;

// Commands (mirrors lcd_api.py).
const LCD_CLR: u8 = 0x01;
const LCD_HOME: u8 = 0x02;
const LCD_ENTRY_MODE: u8 = 0x04;
const LCD_ENTRY_INC: u8 = 0x02;
const LCD_ON_CTRL: u8 = 0x08;
const LCD_ON_DISPLAY: u8 = 0x04;
const LCD_ON_CURSOR: u8 = 0x02;
const LCD_ON_BLINK: u8 = 0x01;
const LCD_FUNCTION: u8 = 0x20;
const LCD_FUNCTION_2LINES: u8 = 0x08;
const LCD_FUNCTION_RESET: u8 = 0x30;
pub(super) const LCD_DDRAM: u8 = 0x80;
pub(super) const LCD_CGRAM: u8 = 0x40;

pub const DEFAULT_I2C_ADDR: u8 = 0x27;

impl<B: I2cBus> Hd44780<B> {
    /// Create and initialize the display. Defaults backlight to on.
    pub fn new(bus: B, addr: u8, cols: u8, rows: u8) -> Result<Self> {
        let mut driver = Hd44780 {
            bus,
            addr,
            cols: cols.min(40),
            rows: rows.min(4),
            cursor_x: 0,
            cursor_y: 0,
            implied_newline: false,
            backlight: Backlight::On,
        };

        driver.bus.write_byte(driver.addr, 0)?;
        sleep_ms(20);
        // Reset sequence: 3x reset nibble, then function nibble.
        driver.write_init_nibble(LCD_FUNCTION_RESET)?;
        sleep_ms(5);
        driver.write_init_nibble(LCD_FUNCTION_RESET)?;
        sleep_ms(1);
        driver.write_init_nibble(LCD_FUNCTION_RESET)?;
        sleep_ms(1);
        driver.write_init_nibble(LCD_FUNCTION)?;
        sleep_ms(1);

        // Function set.
        let mut cmd = LCD_FUNCTION;
        if rows > 1 {
            cmd |= LCD_FUNCTION_2LINES;
        }
        driver.write_command(cmd)?;

        // Mirror python_lcd init: display off, clear/home, entry mode, display on.
        driver.write_command(LCD_ON_CTRL)?; // display off
        driver.clear()?;
        driver.write_command(LCD_ENTRY_MODE | LCD_ENTRY_INC)?;
        driver.display_on()?;
        Ok(driver)
    }

    /// Clear display and home cursor. Requires the longer delay.
    pub fn clear(&mut self) -> Result<()> {
        self.write_command(LCD_CLR)?;
        self.write_command(LCD_HOME)?;
        self.cursor_x = 0;
        self.cursor_y = 0;
        Ok(())
    }

    pub fn display_on(&mut self) -> Result<()> {
        self.write_command(LCD_ON_CTRL | LCD_ON_DISPLAY)
    }

    pub fn display_off(&mut self) -> Result<()> {
        self.write_command(LCD_ON_CTRL)
    }

    pub fn show_cursor(&mut self) -> Result<()> {
        self.write_command(LCD_ON_CTRL | LCD_ON_DISPLAY | LCD_ON_CURSOR)
    }

    pub fn hide_cursor(&mut self) -> Result<()> {
        self.write_command(LCD_ON_CTRL | LCD_ON_DISPLAY)
    }

    pub fn blink_cursor_on(&mut self) -> Result<()> {
        self.write_command(LCD_ON_CTRL | LCD_ON_DISPLAY | LCD_ON_CURSOR | LCD_ON_BLINK)
    }

    pub fn blink_cursor_off(&mut self) -> Result<()> {
        self.hide_cursor()
    }

    pub fn backlight_on(&mut self) -> Result<()> {
        self.backlight = Backlight::On;
        self.bus.write_byte(self.addr, 1 << SHIFT_BACKLIGHT)
    }

    pub fn backlight_off(&mut self) -> Result<()> {
        self.backlight = Backlight::Off;
        self.bus.write_byte(self.addr, 0)
    }

    /// Position cursor and write a line (wraps using putchar logic).
    pub fn write_line(&mut self, row: u8, text: &str) -> Result<()> {
        self.move_to(0, row)?;
        self.putstr(text)
    }

    pub fn move_to(&mut self, cursor_x: u8, cursor_y: u8) -> Result<()> {
        self.cursor_x = cursor_x;
        self.cursor_y = cursor_y % self.rows.max(1);
        let mut addr = cursor_x & 0x3f;
        if self.cursor_y & 1 == 1 {
            addr += 0x40;
        }
        if self.cursor_y & 2 == 2 {
            addr += self.cols;
        }
        self.write_command(LCD_DDRAM | addr)
    }

    pub fn putchar(&mut self, ch: char) -> Result<()> {
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

    pub fn putstr(&mut self, text: &str) -> Result<()> {
        for ch in text.chars() {
            self.putchar(ch)?;
        }
        Ok(())
    }

    /// Extended string: supports `{0xNN}` placeholders to emit raw bytes (e.g., custom chars).
    pub fn putstr_extended(&mut self, text: &str) -> Result<()> {
        let mut idx = 0;
        let bytes = text.as_bytes();
        while idx < bytes.len() {
            if bytes[idx] == b'{'
                && idx + 6 <= bytes.len()
                && bytes[idx + 1] == b'0'
                && (bytes[idx + 2] == b'x' || bytes[idx + 2] == b'X')
                && bytes[idx + 5] == b'}'
            {
                if let (Some(h1), Some(h2)) = (from_hex(bytes[idx + 3]), from_hex(bytes[idx + 4])) {
                    let value = (h1 << 4) | h2;
                    self.putchar(value as char)?;
                    idx += 6;
                    continue;
                }
            }
            let ch = bytes[idx] as char;
            self.putchar(ch)?;
            idx += 1;
        }
        Ok(())
    }

    /// Write a custom character pattern into CGRAM (location 0-7).
    pub fn custom_char(&mut self, location: u8, pattern: &[u8; 8]) -> Result<()> {
        let loc = location & 0x7;
        self.write_command(LCD_CGRAM | (loc << 3))?;
        sleep_us(40);
        for byte in pattern {
            self.write_data(*byte)?;
            sleep_us(40);
        }
        self.move_to(self.cursor_x, self.cursor_y)?;
        Ok(())
    }

    /// Convenience helper: load a 5x8 bitmap expressed as strings of '1'/'0'/'#'/'.'.
    pub fn load_custom_bitmap(&mut self, location: u8, rows: [&str; 8]) -> Result<()> {
        let mut pattern = [0u8; 8];
        for (i, row) in rows.iter().enumerate() {
            pattern[i] = parse_bitmap_row(row)?;
        }
        self.custom_char(location, &pattern)
    }

    /// Load multiple bitmaps sequentially starting at CGRAM address 0.
    pub fn load_custom_bitmaps(&mut self, bitmaps: &[[&str; 8]]) -> Result<()> {
        for (idx, rows) in bitmaps.iter().enumerate().take(8) {
            self.load_custom_bitmap(idx as u8, *rows)?;
        }
        Ok(())
    }

    fn write_init_nibble(&mut self, nibble: u8) -> Result<()> {
        let byte = ((nibble >> 4) & 0x0f) << SHIFT_DATA;
        self.bus.write_byte(self.addr, byte | MASK_E)?;
        self.bus.write_byte(self.addr, byte)?;
        Ok(())
    }

    fn write_command(&mut self, cmd: u8) -> Result<()> {
        self.write_nibble(cmd, false)?;
        self.write_nibble(cmd << 4, false)?;
        if cmd <= 3 {
            // HOME/CLEAR need extra delay.
            sleep_ms(5);
        }
        Ok(())
    }

    fn write_data(&mut self, data: u8) -> Result<()> {
        self.write_nibble(data, true)?;
        self.write_nibble(data << 4, true)?;
        Ok(())
    }

    fn write_nibble(&mut self, nibble: u8, is_data: bool) -> Result<()> {
        let mut byte = self.backlight_mask();
        if is_data {
            byte |= MASK_RS;
        }
        byte |= (nibble >> 4) << SHIFT_DATA;

        self.bus.write_byte(self.addr, byte | MASK_E)?;
        self.bus.write_byte(self.addr, byte)?;
        Ok(())
    }

    fn backlight_mask(&self) -> u8 {
        match self.backlight {
            Backlight::On => 1 << SHIFT_BACKLIGHT,
            Backlight::Off => 0,
        }
    }
}

fn sleep_ms(ms: u64) {
    std::thread::sleep(Duration::from_millis(ms));
}

fn sleep_us(us: u64) {
    std::thread::sleep(Duration::from_micros(us));
}

pub(super) fn from_hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(10 + byte - b'a'),
        b'A'..=b'F' => Some(10 + byte - b'A'),
        _ => None,
    }
}

pub(super) fn parse_bitmap_row(row: &str) -> Result<u8> {
    if row.len() > 5 {
        return Err(Error::InvalidArgs(
            "bitmap rows must be at most 5 characters".into(),
        ));
    }
    let mut value = 0u8;
    for (idx, ch) in row.chars().enumerate() {
        let bit = matches!(ch, '1' | '#');
        if bit {
            value |= 1 << (4 - idx);
        }
    }
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Debug, Default)]
    struct MockBus {
        writes: Vec<(u8, u8)>,
        decoded: Vec<DecodedByte>,
        pending_enable: Option<(bool, u8)>,
        partial_byte: Option<(bool, u8)>,
    }

    #[derive(Debug, Clone, Copy)]
    struct DecodedByte {
        rs: bool,
        value: u8,
    }

    impl I2cBus for MockBus {
        fn write_byte(&mut self, addr: u8, byte: u8) -> Result<()> {
            if byte & MASK_E != 0 {
                self.pending_enable = Some((byte & MASK_RS != 0, byte));
            } else if let Some((rs, prev)) = self.pending_enable.take() {
                let nibble = (prev & 0xF0) >> 4;
                self.record_nibble(rs, nibble);
            }
            self.writes.push((addr, byte));
            Ok(())
        }
    }

    impl MockBus {
        fn record_nibble(&mut self, rs: bool, nibble: u8) {
            if let Some((prev_rs, prev)) = self.partial_byte.take() {
                debug_assert_eq!(prev_rs, rs);
                let value = (prev << 4) | nibble;
                self.decoded.push(DecodedByte { rs, value });
            } else {
                self.partial_byte = Some((rs, nibble));
            }
        }

        fn take_decoded_commands(&mut self) -> Vec<u8> {
            let cmds: Vec<u8> = self
                .decoded
                .iter()
                .filter(|d| !d.rs)
                .map(|d| d.value)
                .collect();
            self.decoded.clear();
            cmds
        }
    }

    #[test]
    fn init_sequence_matches_python_order() {
        let bus = MockBus::default();
        let mut driver = Hd44780::new(bus, 0x27, 16, 2).unwrap();
        // Expect initial zero write, then reset sequence and function set.
        let writes = &driver.bus.writes;
        assert_eq!(writes[0], (0x27, 0)); // leading zero byte
                                          // First init nibble 0x30 => 0x34 (E high), 0x30 (E low)
        assert_eq!(writes[1], (0x27, 0x34));
        assert_eq!(writes[2], (0x27, 0x30));
        // Last function set low nibble should carry backlight bit.
        let has_function = writes.iter().any(|&(_, b)| b == 0x8C || b == 0x2C);
        assert!(has_function);
        // Backlight bit set in later writes.
        assert!(writes.iter().any(|&(_, b)| b & (1 << SHIFT_BACKLIGHT) != 0));
        // Clear/home issues commands requiring delay.
        driver.clear().unwrap();
        let after_clear_len = driver.bus.writes.len();
        assert!(after_clear_len > 6);
    }

    #[test]
    fn write_line_wraps() {
        let mut driver = Hd44780::new(MockBus::default(), 0x27, 8, 2).unwrap();
        driver.write_line(0, "abcdefghi").unwrap();
        assert_eq!(driver.cursor_x, 1);
        assert_eq!(driver.cursor_y, 1);
    }

    #[test]
    fn implied_newline_matches_python_behavior() {
        let mut driver = Hd44780::new(MockBus::default(), 0x27, 4, 2).unwrap();
        driver.write_line(0, "abcd").unwrap(); // wraps to next line due to full row
        driver.putchar('\n').unwrap(); // should ignore because implied_newline is true
        assert_eq!(driver.cursor_y, 1);
        let writes_after_first = driver.bus.writes.len();
        driver.putchar('\n').unwrap(); // now treat as explicit newline and wrap again
        assert!(driver.bus.writes.len() > writes_after_first);
    }

    #[test]
    fn parses_extended_placeholders() {
        let mut driver = Hd44780::new(MockBus::default(), 0x27, 8, 2).unwrap();
        driver.putstr_extended("A{0x41}B").unwrap(); // Should emit A, 'A', B
        assert_eq!(driver.cursor_x, 3);
    }

    #[test]
    fn smoke_init_clear_backlight() {
        let mut driver = Hd44780::new(MockBus::default(), 0x27, 16, 2).unwrap();
        let before = driver.bus.writes.len();
        driver.clear().unwrap();
        driver.backlight_off().unwrap();
        driver.backlight_on().unwrap();
        driver.write_line(0, "hi").unwrap();
        assert!(driver.bus.writes.len() > before);
    }

    #[test]
    fn loads_custom_bitmap_rows() {
        let mut driver = Hd44780::new(MockBus::default(), 0x27, 16, 2).unwrap();
        let heart = [
            "01010", "11111", "11111", "11111", "01110", "00100", "00000", "00000",
        ];
        driver.load_custom_bitmap(0, heart).unwrap();
        // Cursor remains valid
        driver.putchar('\n').unwrap();
        assert_eq!(driver.cursor_y, 1); // newline advances to next line
    }

    #[test]
    fn write_line_avoids_clear_between_updates() {
        let mut driver = Hd44780::new(MockBus::default(), 0x27, 16, 2).unwrap();
        driver.bus.decoded.clear();
        driver.write_line(0, "first").unwrap();
        driver.write_line(0, "second").unwrap();
        let commands = driver.bus.take_decoded_commands();
        assert!(
            !commands.iter().any(|&cmd| cmd == LCD_CLR),
            "steady-state writes must not issue LCD_CLR"
        );
    }

    #[test]
    fn blink_cursor_command_emitted() {
        let mut driver = Hd44780::new(MockBus::default(), 0x27, 16, 2).unwrap();
        driver.bus.decoded.clear();
        driver.blink_cursor_on().unwrap();
        let commands = driver.bus.take_decoded_commands();
        let expected = LCD_ON_CTRL | LCD_ON_DISPLAY | LCD_ON_CURSOR | LCD_ON_BLINK;
        assert!(
            commands.iter().any(|&cmd| cmd == expected),
            "blink command missing from decoded stream"
        );
    }

    #[test]
    fn custom_char_restores_cursor_position() {
        let mut driver = Hd44780::new(MockBus::default(), 0x27, 16, 2).unwrap();
        driver.move_to(3, 1).unwrap();
        let pattern = [0u8; 8];
        driver.custom_char(2, &pattern).unwrap();
        assert_eq!(driver.cursor_x, 3);
        assert_eq!(driver.cursor_y, 1);
    }
}
