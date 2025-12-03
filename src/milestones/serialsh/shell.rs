//! Shell loop stub for serialsh mode. Will eventually read commands, send them across the tunnel, and present stdout/stderr.

use crate::Result;

/// Placeholder shell context describing command history management.
pub struct ShellContext;

impl ShellContext {
    /// Construct a preview shell context. Future implementations will carry channel handles
    /// and terminal state, but for now we only use this to signal feature-gated availability.
    pub fn preview() -> Self {
        Self
    }

    /// Placeholder method that would run the main loop once wired to the tunnel channel.
    /// For the preview feature we implement a non-interactive no-op that returns success.
    /// Future implementations will run an interactive loop and multiplex with the serial
    /// command tunnel.
    pub fn run(self) -> Result<()> {
        // For now, simply print a clear, non-cryptic notice and return Ok so the run-path
        // can be exercised in tests without interacting with serial hardware.
        eprintln!("serialsh-preview: serial shell is enabled but still in preview mode; exiting");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    #[cfg(feature = "serialsh-preview")]
    #[test]
    fn preview_run_returns_ok() {
        use super::*;
        let shell = ShellContext::preview();
        assert!(
            shell.run().is_ok(),
            "expected preview shell run to return Ok"
        );
    }
}
