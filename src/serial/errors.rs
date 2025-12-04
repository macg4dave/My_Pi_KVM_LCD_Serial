use crate::Error;
use serde::Serialize;
use std::fmt;
use std::io::ErrorKind;

/// High-level reason for a serial transport failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SerialFailureKind {
    PermissionDenied,
    DeviceMissing,
    Disconnected,
    Timeout,
    Framing,
    Busy,
    Config,
    Unknown,
}

impl SerialFailureKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            SerialFailureKind::PermissionDenied => "permission_denied",
            SerialFailureKind::DeviceMissing => "device_missing",
            SerialFailureKind::Disconnected => "disconnected",
            SerialFailureKind::Timeout => "timeout",
            SerialFailureKind::Framing => "framing",
            SerialFailureKind::Busy => "busy",
            SerialFailureKind::Config => "config",
            SerialFailureKind::Unknown => "unknown",
        }
    }
}

impl fmt::Display for SerialFailureKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Classify a crate-level error into a serial failure reason when possible.
pub fn classify_error(err: &Error) -> SerialFailureKind {
    match err {
        Error::InvalidArgs(_) => SerialFailureKind::Config,
        Error::Io(io_err) => classify_io_error(io_err),
        Error::Parse(_) | Error::ChecksumMismatch => SerialFailureKind::Framing,
    }
}

/// Classify an std::io::Error into a serial failure reason.
pub fn classify_io_error(err: &std::io::Error) -> SerialFailureKind {
    match err.kind() {
        ErrorKind::PermissionDenied => SerialFailureKind::PermissionDenied,
        ErrorKind::NotFound => SerialFailureKind::DeviceMissing,
        ErrorKind::TimedOut | ErrorKind::WouldBlock => SerialFailureKind::Timeout,
        ErrorKind::BrokenPipe | ErrorKind::ConnectionReset | ErrorKind::ConnectionAborted => {
            SerialFailureKind::Disconnected
        }
        ErrorKind::InvalidInput => SerialFailureKind::Config,
        ErrorKind::InvalidData => SerialFailureKind::Framing,
        _ => {
            if let Some(code) = err.raw_os_error() {
                match code {
                    // 110 = ETIMEDOUT, 5 = EIO, 6 = ENXIO, 19 = ENODEV, 13 = EACCES, 16 = EBUSY
                    13 => SerialFailureKind::PermissionDenied,
                    16 => SerialFailureKind::Busy,
                    19 | 6 => SerialFailureKind::DeviceMissing,
                    5 => SerialFailureKind::Disconnected,
                    110 => SerialFailureKind::Timeout,
                    _ => SerialFailureKind::Unknown,
                }
            } else {
                SerialFailureKind::Unknown
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn classify_permission_denied() {
        let err = std::io::Error::new(ErrorKind::PermissionDenied, "denied");
        assert_eq!(classify_io_error(&err), SerialFailureKind::PermissionDenied);
    }

    #[test]
    fn classify_timeout_and_broken_pipe() {
        let timeout = std::io::Error::new(ErrorKind::TimedOut, "timeout");
        assert_eq!(classify_io_error(&timeout), SerialFailureKind::Timeout);
        let broken = std::io::Error::new(ErrorKind::BrokenPipe, "broken");
        assert_eq!(classify_io_error(&broken), SerialFailureKind::Disconnected);
    }

    #[test]
    fn classify_crate_errors() {
        let err = Error::InvalidArgs("bad".into());
        assert_eq!(classify_error(&err), SerialFailureKind::Config);
        let parse = Error::Parse("oops".into());
        assert_eq!(classify_error(&parse), SerialFailureKind::Framing);
    }
}
