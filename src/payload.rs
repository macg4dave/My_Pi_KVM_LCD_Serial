use crate::{Error, Result};
use crc32fast::Hasher;
use serde::{Deserialize, Serialize};

pub const DEFAULT_SCROLL_MS: u64 = 250;
pub const DEFAULT_PAGE_TIMEOUT_MS: u64 = 4000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Defaults {
    pub scroll_speed_ms: u64,
    pub page_timeout_ms: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub struct Payload {
    pub line1: String,
    pub line2: String,

    #[serde(default)]
    pub version: Option<u8>,
    #[serde(default)]
    pub bar: Option<u8>,
    #[serde(default)]
    pub bar_value: Option<u32>,
    #[serde(default)]
    pub bar_max: Option<u32>,
    #[serde(default)]
    pub bar_label: Option<String>,
    #[serde(default)]
    pub bar_line1: Option<bool>,
    #[serde(default)]
    pub bar_line2: Option<bool>,

    #[serde(default)]
    pub backlight: Option<bool>, // only sent when false to turn off
    #[serde(default)]
    pub blink: Option<bool>,
    #[serde(default)]
    pub scroll: Option<bool>,
    #[serde(default)]
    pub scroll_speed_ms: Option<u64>,
    #[serde(default, alias = "ttl_ms")]
    pub duration_ms: Option<u64>,
    #[serde(default)]
    pub page_timeout_ms: Option<u64>,
    #[serde(default)]
    pub clear: Option<bool>,
    #[serde(default)]
    pub test: Option<bool>,
    #[serde(default)]
    pub mode: Option<String>,
    #[serde(default)]
    pub icons: Option<Vec<String>>,
    #[serde(default)]
    pub checksum: Option<String>,
    #[serde(default)]
    pub config_reload: Option<bool>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DisplayMode {
    Normal,
    Dashboard,
    Banner,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Icon {
    Battery,
    Arrow,
    Heart,
}

#[derive(Debug, Clone, PartialEq)]
pub struct RenderFrame {
    pub line1: String,
    pub line2: String,
    pub backlight_on: bool,
    pub blink: bool,
    pub bar_percent: Option<u8>,
    pub bar_label: Option<String>,
    pub bar_row: Option<u8>, // 0 = top, 1 = bottom
    pub scroll_speed_ms: u64,
    pub scroll_enabled: bool,
    pub duration_ms: Option<u64>,
    pub page_timeout_ms: u64,
    pub clear: bool,
    pub test: bool,
    pub mode: DisplayMode,
    pub icons: Vec<Icon>,
    pub config_reload: bool,
}

impl RenderFrame {
    pub fn from_payload_json(raw: &str) -> Result<Self> {
        Self::from_payload_json_with_defaults(
            raw,
            Defaults {
                scroll_speed_ms: DEFAULT_SCROLL_MS,
                page_timeout_ms: DEFAULT_PAGE_TIMEOUT_MS,
            },
        )
    }

    pub fn from_payload_json_with_defaults(raw: &str, defaults: Defaults) -> Result<Self> {
        let payload: Payload =
            serde_json::from_str(raw).map_err(|e| Error::Parse(format!("json: {e}")))?;

        if let Some(version) = payload.version {
            const SUPPORTED_VERSION: u8 = 1;
            if version != SUPPORTED_VERSION {
                return Err(Error::Parse(format!(
                    "unsupported version {version}, expected {SUPPORTED_VERSION}"
                )));
            }
        }

        if let Some(checksum_hex) = &payload.checksum {
            let canonical = Payload {
                checksum: None,
                ..payload.clone()
            };
            let mut hasher = Hasher::new();
            let bytes = serde_json::to_vec(&canonical)
                .map_err(|e| Error::Parse(format!("serialize for checksum: {e}")))?;
            hasher.update(&bytes);
            let computed = hasher.finalize();
            let expected = u32::from_str_radix(checksum_hex.trim_start_matches("0x"), 16)
                .map_err(|_| Error::Parse("invalid checksum hex".into()))?;
            if computed != expected {
                return Err(Error::ChecksumMismatch);
            }
        }

        Ok(Self::from_payload_with_defaults(payload, defaults))
    }

    pub fn from_payload_with_defaults(payload: Payload, defaults: Defaults) -> Self {
        let backlight_on = payload.backlight.unwrap_or(true);
        let blink = payload.blink.unwrap_or(false);
        let scroll_enabled = payload.scroll.unwrap_or(true);
        let scroll_speed_ms = payload
            .scroll_speed_ms
            .unwrap_or(defaults.scroll_speed_ms);
        let page_timeout_ms = payload
            .page_timeout_ms
            .unwrap_or(defaults.page_timeout_ms);

        let bar_percent = compute_bar_percent(&payload);
        let bar_row = if bar_percent.is_some() {
            if payload.bar_line1.unwrap_or(false) {
                Some(0)
            } else if payload.bar_line2.unwrap_or(true) {
                Some(1)
            } else {
                Some(1)
            }
        } else {
            None
        };

        let mode = match payload.mode.as_deref() {
            Some("dashboard") => DisplayMode::Dashboard,
            Some("banner") => DisplayMode::Banner,
            _ => DisplayMode::Normal,
        };

        let icons = payload
            .icons
            .unwrap_or_default()
            .into_iter()
            .filter_map(|name| match name.to_lowercase().as_str() {
                "battery" => Some(Icon::Battery),
                "arrow" => Some(Icon::Arrow),
                "heart" => Some(Icon::Heart),
                _ => None,
            })
            .collect::<Vec<_>>();

        let line1 = payload.line1;
        let mut line2 = payload.line2;
        if matches!(mode, DisplayMode::Banner) {
            line2 = String::new();
        }

        let bar_row = if matches!(mode, DisplayMode::Dashboard) && bar_percent.is_some() {
            Some(1)
        } else {
            bar_row
        };

        RenderFrame {
            line1,
            line2,
            backlight_on,
            blink,
            bar_percent,
            bar_label: payload.bar_label,
            bar_row,
            scroll_speed_ms,
            scroll_enabled,
            duration_ms: payload.duration_ms,
            page_timeout_ms,
            clear: payload.clear.unwrap_or(false),
            test: payload.test.unwrap_or(false),
            mode,
            icons,
            config_reload: payload.config_reload.unwrap_or(false),
        }
    }
}

fn compute_bar_percent(payload: &Payload) -> Option<u8> {
    if let Some(percent) = payload.bar {
        return Some(percent.clamp(0, 100));
    }
    if let Some(value) = payload.bar_value {
        let max = payload.bar_max.unwrap_or(100).max(1);
        let percent = ((value as f64 / max as f64) * 100.0).round() as i32;
        let clamped = percent.clamp(0, 100) as u8;
        return Some(clamped);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_basic_payload_with_defaults() {
        let raw = r#"{"line1":"Hello","line2":"World"}"#;
        let frame = RenderFrame::from_payload_json(raw).unwrap();
        assert_eq!(frame.line1, "Hello");
        assert_eq!(frame.line2, "World");
        assert!(frame.backlight_on);
        assert_eq!(frame.scroll_speed_ms, DEFAULT_SCROLL_MS);
        assert_eq!(frame.page_timeout_ms, DEFAULT_PAGE_TIMEOUT_MS);
        assert!(frame.scroll_enabled);
        assert!(matches!(frame.mode, DisplayMode::Normal));
    }

    #[test]
    fn bar_percent_from_value_and_max() {
        let raw = r#"{"line1":"","line2":"","bar_value":500,"bar_max":1000}"#;
        let frame = RenderFrame::from_payload_json(raw).unwrap();
        assert_eq!(frame.bar_percent, Some(50));
    }

    #[test]
    fn bar_field_takes_priority() {
        let raw = r#"{"line1":"","line2":"","bar":42,"bar_value":10,"bar_max":20}"#;
        let frame = RenderFrame::from_payload_json(raw).unwrap();
        assert_eq!(frame.bar_percent, Some(42));
    }

    #[test]
    fn scroll_can_be_disabled() {
        let raw = r#"{"line1":"LongLineThatWillNotScroll","line2":"","scroll":false}"#;
        let frame = RenderFrame::from_payload_json(raw).unwrap();
        assert!(!frame.scroll_enabled);
    }

    #[test]
    fn checksum_validates() {
        let payload = Payload {
            line1: "Hi".into(),
            line2: "There".into(),
            version: None,
            bar: None,
            bar_value: None,
            bar_max: None,
            bar_label: None,
            bar_line1: None,
            bar_line2: None,
            backlight: None,
            blink: None,
            scroll: None,
            scroll_speed_ms: None,
            duration_ms: None,
            page_timeout_ms: None,
            clear: None,
            test: None,
            mode: None,
            icons: None,
            checksum: None,
            config_reload: None,
        };
        let mut hasher = Hasher::new();
        let canonical = serde_json::to_vec(&payload).unwrap();
        hasher.update(&canonical);
        let crc = hasher.finalize();

        let mut with_checksum = payload.clone();
        with_checksum.checksum = Some(format!("{crc:08x}"));
        let raw = serde_json::to_string(&with_checksum).unwrap();

        let parsed = RenderFrame::from_payload_json(&raw).unwrap();
        assert_eq!(parsed.line1, "Hi");
    }

    #[test]
    fn checksum_rejects_invalid() {
        let raw = r#"{"line1":"A","line2":"B","checksum":"deadbeef"}"#;
        let err = RenderFrame::from_payload_json(raw).unwrap_err();
        assert!(matches!(err, Error::ChecksumMismatch));
    }

    #[test]
    fn rejects_unsupported_version() {
        let raw = r#"{"line1":"A","line2":"B","version":2}"#;
        let err = RenderFrame::from_payload_json(raw).unwrap_err();
        assert!(format!("{err}").contains("unsupported version"));
    }

    #[test]
    fn duration_ms_supports_new_and_legacy_names() {
        let raw_new = r#"{"line1":"","line2":"","duration_ms":1234}"#;
        let frame_new = RenderFrame::from_payload_json(raw_new).unwrap();
        assert_eq!(frame_new.duration_ms, Some(1234));

        let raw_legacy = r#"{"line1":"","line2":"","ttl_ms":2345}"#;
        let frame_legacy = RenderFrame::from_payload_json(raw_legacy).unwrap();
        assert_eq!(frame_legacy.duration_ms, Some(2345));
    }
}
