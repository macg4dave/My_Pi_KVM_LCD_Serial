use crate::CACHE_DIR;
use serde::Serialize;
use std::io::{self, Write};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

const TELEMETRY_FILE: &str = "serial_backoff.log";
static FILE_HANDLE: OnceLock<Mutex<std::fs::File>> = OnceLock::new();

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase"])
pub enum BackoffPhase {
    Attempt,
    Success,
    Failure,
}

#[derive(Serialize)]
struct BackoffEntry<'a> {
    ts_ms: u128,
    event: &'static str,
    phase: BackoffPhase,
    attempt: u64,
    delay_ms: u64,
    max_ms: u64,
    device: &'a str,
    baud: u32,
}

pub fn log_backoff_event(
    phase: BackoffPhase,
    attempt: u64,
    delay_ms: u64,
    max_ms: u64,
    device: &str,
    baud: u32,
) -> io::Result<()> {
    let entry = BackoffEntry {
        ts_ms: SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap_or_default()
            .as_millis(),
        event: "serial_backoff",
        phase,
        attempt,
        delay_ms,
        max_ms,
        device,
        baud,
    };

    let line = serde_json::to_string(&entry)
        .map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
    let handle = get_file()?;
    if let Ok(mut file) = handle.lock() {
        writeln!(file, "{line}")?;
    }
    Ok(())
}

fn get_file() -> io::Result<&'static Mutex<std::fs::File>> {
    FILE_HANDLE.get_or_try_init(|| {
        let path = PathBuf::from(CACHE_DIR).join(TELEMETRY_FILE);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)?;
        Ok(Mutex::new(file))
    })
}
