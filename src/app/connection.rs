use super::Logger;
pub(crate) use crate::serial::backoff::BackoffController;
use crate::serial::{SerialOptions, SerialPort};

/// Attempt to open the serial port and send the INIT handshake, logging outcomes.
pub(crate) fn attempt_serial_connect(
    logger: &Logger,
    device: &str,
    options: SerialOptions,
) -> Option<SerialPort> {
    match SerialPort::connect(device, options) {
        Ok(mut serial_connection) => {
            if let Err(err) = serial_connection.send_command_line("INIT") {
                logger.warn(format!("serial init failed: {err}; will retry"));
                None
            } else {
                logger.info("serial connected");
                Some(serial_connection)
            }
        }
        Err(err) => {
            logger.warn(format!("serial connect failed: {err}; will retry"));
            None
        }
    }
}
