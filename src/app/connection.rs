use super::Logger;
pub(crate) use crate::serial::backoff::BackoffController;
use crate::serial::SerialPort;

/// Attempt to open the serial port and send the INIT handshake, logging outcomes.
pub(crate) fn attempt_serial_connect(
    logger: &Logger,
    device: &str,
    baud: u32,
) -> Option<SerialPort> {
    match SerialPort::connect(device, baud) {
        Ok(mut serial_connection) => {
            if let Err(err) = serial_connection.send_command_line("INIT") {
                logger.log(format!("serial init failed: {err}; will retry"));
                None
            } else {
                logger.log("serial connected".into());
                Some(serial_connection)
            }
        }
        Err(err) => {
            logger.log(format!("serial connect failed: {err}; will retry"));
            None
        }
    }
}
