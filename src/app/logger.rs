use std::io::Write;
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
    pub fn new(level: LogLevel, file_path: Option<String>) -> Self {
        let env_level = std::env::var("LIFELINETTY_LOG_LEVEL")
            .or_else(|_| std::env::var("SERIALLCD_LOG_LEVEL"))
            .ok()
            .and_then(|s| LogLevel::from_str(&s).ok());
        let effective_level = env_level.unwrap_or(level);

        let env_file = std::env::var("LIFELINETTY_LOG_PATH")
            .or_else(|_| std::env::var("SERIALLCD_LOG_PATH"))
            .ok();
        let path = file_path.or(env_file);
        let file = path.and_then(|p| {
            std::fs::OpenOptions::new()
                .create(true)
                .append(true)
                .open(p)
                .ok()
        });
        Self {
            level: effective_level,
            file,
        }
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
