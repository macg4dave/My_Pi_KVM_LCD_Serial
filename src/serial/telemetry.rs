use crate::CACHE_DIR;
use serde::Serialize;
use std::io::{self, ErrorKind, Write};
use std::path::PathBuf;
use std::sync::{Mutex, OnceLock};
use std::time::{SystemTime, UNIX_EPOCH};

const TELEMETRY_FILE: &str = "serial_backoff.log";
static FILE_HANDLE: OnceLock<io::Result<Mutex<std::fs::File>>> = OnceLock::new();

#[derive(Debug, Clone, Copy, Serialize)]
#[serde(rename_all = "lowercase")]
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

    let line =
        serde_json::to_string(&entry).map_err(|err| io::Error::new(io::ErrorKind::Other, err))?;
    let handle = get_file()?;
    if let Ok(mut file) = handle.lock() {
        writeln!(file, "{line}")?;
    }
    Ok(())
}

fn get_file() -> io::Result<&'static Mutex<std::fs::File>> {
    if FILE_HANDLE.get().is_none() {
        let handle = create_file_handle()?;
        let _ = FILE_HANDLE.set(Ok(handle));
    }

    let result_ref = FILE_HANDLE.get().ok_or_else(|| {
        io::Error::new(
            ErrorKind::Other,
            "failed to initialize serial telemetry log handle",
        )
    })?;

    match result_ref {
        Ok(m) => Ok(m),
        Err(err) => Err(io::Error::new(io::ErrorKind::Other, err.to_string())),
    }
}

fn create_file_handle() -> io::Result<Mutex<std::fs::File>> {
    let path = PathBuf::from(CACHE_DIR).join(TELEMETRY_FILE);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(path)?;
    Ok(Mutex::new(file))
}
