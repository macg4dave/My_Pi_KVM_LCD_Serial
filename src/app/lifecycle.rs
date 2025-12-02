use crate::{lcd::Lcd, Error, Result};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

/// Install a ctrl-c handler that flips the shared running flag instead of exiting immediately.
pub(super) fn create_shutdown_flag() -> Result<Arc<AtomicBool>> {
    let running = Arc::new(AtomicBool::new(true));
    let running_handle = running.clone();

    ctrlc::set_handler(move || {
        running_handle.store(false, Ordering::SeqCst);
    })
    .map_err(|e| Error::Io(std::io::Error::other(e.to_string())))?;

    Ok(running)
}

/// Show the shutdown message before exiting the daemon loop.
pub(super) fn render_shutdown(lcd: &mut Lcd) -> Result<()> {
    lcd.clear()?;
    lcd.set_blink(false)?;
    lcd.write_line(0, "offline")?;
    lcd.write_line(1, "")?;
    Ok(())
}
