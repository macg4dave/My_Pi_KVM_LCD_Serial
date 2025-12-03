//! Optional async serial helpers behind the `async-serial` feature.
#![cfg(feature = "async-serial")]

use crate::{
    serial::{DtrBehavior, FlowControlMode, ParityMode, SerialOptions, StopBitsMode},
    Error, Result,
};
use std::{io, time::Duration};
use tokio_serial::{DataBits, FlowControl, Parity, SerialPortBuilderExt, StopBits};

pub async fn connect(device: &str, options: SerialOptions) -> Result<tokio_serial::SerialStream> {
    if device.is_empty() {
        return Err(Error::InvalidArgs(
            "device path cannot be empty".to_string(),
        ));
    }

    let builder = tokio_serial::new(device, options.baud)
        .data_bits(DataBits::Eight)
        .parity(to_tokio_parity(options.parity))
        .stop_bits(to_tokio_stop_bits(options.stop_bits))
        .flow_control(to_tokio_flow(options.flow_control))
        .timeout(Duration::from_millis(options.timeout_ms));

    let mut port = builder
        .open_native_async()
        .map_err(|e| Error::Io(io::Error::from(e)))?;

    if let Some(level) = desired_dtr(options.dtr) {
        port.set_data_terminal_ready(level).map_err(Error::Io)?;
    }

    Ok(port)
}

fn to_tokio_flow(mode: FlowControlMode) -> FlowControl {
    match mode {
        FlowControlMode::None => FlowControl::None,
        FlowControlMode::Software => FlowControl::Software,
        FlowControlMode::Hardware => FlowControl::Hardware,
    }
}

fn to_tokio_parity(mode: ParityMode) -> Parity {
    match mode {
        ParityMode::None => Parity::None,
        ParityMode::Odd => Parity::Odd,
        ParityMode::Even => Parity::Even,
    }
}

fn to_tokio_stop_bits(mode: StopBitsMode) -> StopBits {
    match mode {
        StopBitsMode::One => StopBits::One,
        StopBitsMode::Two => StopBits::Two,
    }
}

fn desired_dtr(mode: DtrBehavior) -> Option<bool> {
    match mode {
        DtrBehavior::Preserve => None,
        DtrBehavior::Assert => Some(true),
        DtrBehavior::Deassert => Some(false),
    }
}
