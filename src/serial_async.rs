//! Optional async serial helpers behind the `async-serial` feature.
#![cfg(feature = "async-serial")]

use crate::{Error, Result};
use tokio_serial::{DataBits, FlowControl, Parity, SerialPortBuilderExt, StopBits};

pub async fn connect(device: &str, baud: u32) -> Result<tokio_serial::SerialStream> {
    if device.is_empty() {
        return Err(Error::InvalidArgs(
            "device path cannot be empty".to_string(),
        ));
    }

    tokio_serial::new(device, baud)
        .data_bits(DataBits::Eight)
        .parity(Parity::None)
        .stop_bits(StopBits::One)
        .flow_control(FlowControl::None)
        .open_native_async()
        .map_err(|e| Error::Io(std::io::Error::new(std::io::ErrorKind::Other, e.to_string())))
}
