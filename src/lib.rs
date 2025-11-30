pub mod app;
pub mod cli;
pub mod config;
pub mod lcd;
pub mod lcd_driver;
pub mod payload;
pub mod state;
pub mod serial;
#[cfg(feature = "async-serial")]
pub mod serial_async;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
pub enum Error {
    InvalidArgs(String),
    Io(std::io::Error),
    Parse(String),
    ChecksumMismatch,
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::InvalidArgs(msg) => write!(f, "invalid arguments: {msg}"),
            Error::Io(err) => write!(f, "io error: {err}"),
            Error::Parse(msg) => write!(f, "parse error: {msg}"),
            Error::ChecksumMismatch => write!(f, "checksum mismatch"),
        }
    }
}

impl std::error::Error for Error {}

impl From<std::io::Error> for Error {
    fn from(value: std::io::Error) -> Self {
        Error::Io(value)
    }
}
