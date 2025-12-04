#[cfg(feature = "async-serial")]
pub mod r#async;
pub mod backoff;
pub mod errors;
pub mod fake;
pub mod sync;
pub mod telemetry;

use std::{fmt, str::FromStr};

/// Flow control behavior applied to the UART link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FlowControlMode {
    /// Do not use any flow control toggles.
    None,
    /// Use XON/XOFF software bytes.
    Software,
    /// Use RTS/CTS hardware pins.
    Hardware,
}

impl Default for FlowControlMode {
    fn default() -> Self {
        FlowControlMode::None
    }
}

impl FromStr for FlowControlMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "none" => Ok(Self::None),
            "software" | "xonxoff" | "xon" | "xoff" => Ok(Self::Software),
            "hardware" | "rtscts" => Ok(Self::Hardware),
            other => Err(format!(
                "invalid flow control '{other}', expected none|software|hardware"
            )),
        }
    }
}

impl fmt::Display for FlowControlMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            FlowControlMode::None => write!(f, "none"),
            FlowControlMode::Software => write!(f, "software"),
            FlowControlMode::Hardware => write!(f, "hardware"),
        }
    }
}

/// Parity settings for the UART link.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParityMode {
    /// No parity check.
    None,
    /// Odd parity.
    Odd,
    /// Even parity.
    Even,
}

impl Default for ParityMode {
    fn default() -> Self {
        ParityMode::None
    }
}

impl FromStr for ParityMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "none" => Ok(Self::None),
            "odd" => Ok(Self::Odd),
            "even" => Ok(Self::Even),
            other => Err(format!("invalid parity '{other}', expected none|odd|even")),
        }
    }
}

impl fmt::Display for ParityMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ParityMode::None => write!(f, "none"),
            ParityMode::Odd => write!(f, "odd"),
            ParityMode::Even => write!(f, "even"),
        }
    }
}

/// Number of stop bits appended to each frame.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StopBitsMode {
    /// Single stop bit (default).
    One,
    /// Two stop bits.
    Two,
}

impl Default for StopBitsMode {
    fn default() -> Self {
        StopBitsMode::One
    }
}

impl FromStr for StopBitsMode {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "1" | "one" => Ok(Self::One),
            "2" | "two" => Ok(Self::Two),
            other => Err(format!("invalid stop bits '{other}', expected 1|2")),
        }
    }
}

impl fmt::Display for StopBitsMode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            StopBitsMode::One => write!(f, "1"),
            StopBitsMode::Two => write!(f, "2"),
        }
    }
}

/// Whether to toggle DTR when opening the port.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DtrBehavior {
    /// Leave the driver behavior unchanged (default).
    Preserve,
    /// Force DTR high when opening.
    Assert,
    /// Force DTR low when opening.
    Deassert,
}

impl Default for DtrBehavior {
    fn default() -> Self {
        DtrBehavior::Preserve
    }
}

impl FromStr for DtrBehavior {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "preserve" | "auto" => Ok(Self::Preserve),
            "assert" | "on" | "high" => Ok(Self::Assert),
            "deassert" | "off" | "low" => Ok(Self::Deassert),
            other => Err(format!(
                "invalid dtr behavior '{other}', expected auto|on|off"
            )),
        }
    }
}

impl fmt::Display for DtrBehavior {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DtrBehavior::Preserve => write!(f, "preserve"),
            DtrBehavior::Assert => write!(f, "on"),
            DtrBehavior::Deassert => write!(f, "off"),
        }
    }
}

/// Collection of serial link settings applied whenever a port is opened.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SerialOptions {
    pub baud: u32,
    pub timeout_ms: u64,
    pub flow_control: FlowControlMode,
    pub parity: ParityMode,
    pub stop_bits: StopBitsMode,
    pub dtr: DtrBehavior,
}

impl SerialOptions {
    pub fn new(baud: u32) -> Self {
        Self {
            baud,
            ..Default::default()
        }
    }
}

impl Default for SerialOptions {
    fn default() -> Self {
        Self {
            baud: 9_600,
            timeout_ms: 500,
            flow_control: FlowControlMode::None,
            parity: ParityMode::None,
            stop_bits: StopBitsMode::One,
            dtr: DtrBehavior::Preserve,
        }
    }
}

pub use errors::{classify_error, classify_io_error, SerialFailureKind};
pub use sync::SerialPort;

/// Trait used by `app::connection` to negotiate handshake frames.
pub trait LineIo {
    fn send_command_line(&mut self, line: &str) -> crate::Result<()>;
    fn read_message_line(&mut self, buf: &mut String) -> crate::Result<usize>;
}
