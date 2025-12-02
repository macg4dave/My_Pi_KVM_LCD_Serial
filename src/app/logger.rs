use crate::{Error, Result as AppResult, CACHE_DIR};
use std::io::Write;
use std::path::{Component, Path, PathBuf};
use std::str::FromStr;
use std::time::{SystemTime, UNIX_EPOCH};

/// Log verbosity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub enum LogLevel {
    Error = 0,
    Warn = 1,
    #[default]
    Info = 2,
    Debug = 3,
    Trace = 4,
}

impl FromStr for LogLevel {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_ascii_lowercase().as_str() {
            "error" => Ok(LogLevel::Error),
            "warn" | "warning" => Ok(LogLevel::Warn),
            "info" => Ok(LogLevel::Info),
            "debug" => Ok(LogLevel::Debug),
            "trace" => Ok(LogLevel::Trace),
            _ => Err(()),
        }
    }
}

/// Simple stderr/file logger with levels and optional file sink.
pub struct Logger {
    level: LogLevel,
    file: Option<std::fs::File>,
}

impl Logger {
    pub fn new(level: LogLevel, file_path: Option<String>) -> AppResult<Self> {
        let env_level = std::env::var("LIFELINETTY_LOG_LEVEL")
            .ok()
            .and_then(|s| LogLevel::from_str(&s).ok());
        let effective_level = env_level.unwrap_or(level);

        let env_file = std::env::var("LIFELINETTY_LOG_PATH").ok();
        let resolved_path = resolve_log_path(file_path.or(env_file))?;
        let file = match resolved_path {
            Some(path) => std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(&path)
                .ok(),
            None => None,
        };
        Ok(Self {
            level: effective_level,
            file,
        })
    }

    pub fn level(&self) -> LogLevel {
        self.level
    }

    pub fn log(&self, level: LogLevel, msg: impl AsRef<str>) {
        if level > self.level {
            return;
        }
        let ts = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs_f32())
            .unwrap_or(0.0);
        let line = format!("[{ts:.3}] [{level:?}] {}", msg.as_ref());
        eprintln!("{line}");
        if let Some(file) = self.file.as_ref() {
            if let Ok(mut clone) = file.try_clone() {
                let _ = writeln!(clone, "{line}");
            }
        }
    }

    #[allow(dead_code)]
    pub fn error(&self, msg: impl AsRef<str>) {
        self.log(LogLevel::Error, msg);
    }

    pub fn warn(&self, msg: impl AsRef<str>) {
        self.log(LogLevel::Warn, msg);
    }

    pub fn info(&self, msg: impl AsRef<str>) {
        self.log(LogLevel::Info, msg);
    }

    pub fn debug(&self, msg: impl AsRef<str>) {
        self.log(LogLevel::Debug, msg);
    }

    #[allow(dead_code)]
    pub fn trace(&self, msg: impl AsRef<str>) {
        self.log(LogLevel::Trace, msg);
    }
}

fn resolve_log_path(raw: Option<String>) -> AppResult<Option<PathBuf>> {
    let Some(raw) = raw else {
        return Ok(None);
    };

    let candidate = PathBuf::from(&raw);
    let resolved = if candidate.is_absolute() {
        candidate
    } else {
        Path::new(CACHE_DIR).join(candidate)
    };

    validate_cache_path(&resolved)?;
    Ok(Some(resolved))
}

fn validate_cache_path(path: &Path) -> AppResult<()> {
    if path
        .components()
        .any(|component| matches!(component, Component::ParentDir))
    {
        return Err(Error::InvalidArgs(
            "log file path must not contain '..' components".to_string(),
        ));
    }

    let cache_root = Path::new(CACHE_DIR);
    if !path.starts_with(cache_root) {
        return Err(Error::InvalidArgs(format!(
            "log file path must live inside {}",
            CACHE_DIR
        )));
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_relative_paths_into_cache() {
        let path = resolve_log_path(Some("logs/demo.log".into()))
            .unwrap()
            .unwrap();
        assert!(path.starts_with(CACHE_DIR));
        assert!(path.ends_with(Path::new("logs/demo.log")));
    }

    #[test]
    fn rejects_paths_outside_cache() {
        let err = resolve_log_path(Some("/tmp/out.log".into())).unwrap_err();
        assert!(format!("{err}").contains(CACHE_DIR));
    }

    #[test]
    fn rejects_parent_dir_components() {
        let err = resolve_log_path(Some("../escape.log".into())).unwrap_err();
        assert!(format!("{err}").contains(".."));
    }
}
