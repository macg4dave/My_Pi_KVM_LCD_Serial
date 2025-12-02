#[cfg(feature = "async-serial")]
pub mod r#async;
pub mod backoff;
pub mod fake;
pub mod sync;
pub mod telemetry;

pub use sync::SerialPort;
