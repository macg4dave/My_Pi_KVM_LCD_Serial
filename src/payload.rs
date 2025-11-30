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
    pub scroll_speed_ms: Option<u64>,
    #[serde(default)]
    pub ttl_ms: Option<u64>,
    #[serde(default)]
    pub page_timeout_ms: Option<u64>,
    #[serde(default)]
    pub clear: Option<bool>,
    #[serde(default)]
    pub test: Option<bool>,
    #[serde(default)]
    pub checksum: Option<String>,
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
    pub ttl_ms: Option<u64>,
    pub page_timeout_ms: u64,
    pub clear: bool,
    pub test: bool,
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

        RenderFrame {
            line1: payload.line1,
            line2: payload.line2,
            backlight_on,
            blink,
            bar_percent,
            bar_label: payload.bar_label,
            bar_row,
            scroll_speed_ms,
            ttl_ms: payload.ttl_ms,
            page_timeout_ms,
            clear: payload.clear.unwrap_or(false),
            test: payload.test.unwrap_or(false),
        }
    }
}

fn compute_bar_percent(payload: &Payload) -> Option<u8> {
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
    }

    #[test]
    fn bar_percent_from_value_and_max() {
        let raw = r#"{"line1":"","line2":"","bar_value":500,"bar_max":1000}"#;
        let frame = RenderFrame::from_payload_json(raw).unwrap();
        assert_eq!(frame.bar_percent, Some(50));
    }

    #[test]
    fn checksum_validates() {
        let payload = Payload {
            line1: "Hi".into(),
            line2: "There".into(),
            bar_value: None,
            bar_max: None,
            bar_label: None,
            bar_line1: None,
            bar_line2: None,
            backlight: None,
            blink: None,
            scroll_speed_ms: None,
            ttl_ms: None,
            page_timeout_ms: None,
            clear: None,
            test: None,
            checksum: None,
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
}
