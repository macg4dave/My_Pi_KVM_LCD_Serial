use crate::{
    compression::{decompress, CompressionCodec},
    config::DEFAULT_PROTOCOL_SCHEMA_VERSION,
    Error, Result, CACHE_DIR,
};
use crc32fast::Hasher;
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;
use std::{borrow::Cow, path::Path};

use super::icons::parse_icons;
use super::{DisplayMode, Icon, DEFAULT_PAGE_TIMEOUT_MS, DEFAULT_SCROLL_MS};

pub const COMMAND_SCHEMA_VERSION: u8 = 1;
pub const COMMAND_MAX_FRAME_BYTES: usize = 4 * 1024;
pub const COMMAND_MAX_COMMAND_CHARS: usize = 512;
pub const COMMAND_MAX_SCRATCH_PATH_BYTES: usize = 256;
pub const COMMAND_MAX_CHUNK_BYTES: usize = 2 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandStream {
    Stdout,
    Stderr,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum CommandMessage {
    Request {
        request_id: u32,
        cmd: String,
        scratch_path: Option<String>,
    },
    Chunk {
        request_id: u32,
        stream: CommandStream,
        seq: u32,
        #[serde(with = "serde_bytes")]
        data: ByteBuf,
    },
    Exit {
        request_id: u32,
        code: i32,
    },
    Ack {
        request_id: u32,
    },
    Busy {
        request_id: u32,
    },
    Error {
        request_id: Option<u32>,
        message: String,
    },
    Heartbeat {
        request_id: Option<u32>,
    },
}

impl CommandMessage {
    fn crc32(&self) -> Result<u32> {
        let bytes = serde_json::to_vec(self).map_err(|e| Error::Parse(format!("json: {e}")))?;
        let mut hasher = Hasher::new();
        hasher.update(&bytes);
        Ok(hasher.finalize())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
struct CommandFrame {
    channel: String,
    schema_version: u8,
    message: CommandMessage,
    crc32: u32,
}

#[derive(Debug, Serialize)]
struct CommandFrameWriter<'a> {
    channel: &'a str,
    schema_version: u8,
    message: &'a CommandMessage,
    crc32: u32,
}

pub fn encode_command_frame(msg: &CommandMessage) -> Result<String> {
    validate_command_message(msg)?;
    let crc32 = msg.crc32()?;
    let frame = CommandFrameWriter {
        channel: "command",
        schema_version: COMMAND_SCHEMA_VERSION,
        message: msg,
        crc32,
    };
    let json = serde_json::to_string(&frame).map_err(|e| Error::Parse(format!("json: {e}")))?;
    if json.as_bytes().len() > COMMAND_MAX_FRAME_BYTES {
        return Err(Error::Parse(format!(
            "command frame exceeds {COMMAND_MAX_FRAME_BYTES} bytes"
        )));
    }
    Ok(json)
}

pub fn decode_command_frame(raw: &str) -> Result<CommandMessage> {
    if raw.as_bytes().len() > COMMAND_MAX_FRAME_BYTES {
        return Err(Error::Parse(format!(
            "command frame exceeds {COMMAND_MAX_FRAME_BYTES} bytes"
        )));
    }
    let frame: CommandFrame =
        serde_json::from_str(raw).map_err(|e| Error::Parse(format!("json: {e}")))?;
    if frame.channel != "command" {
        return Err(Error::Parse("unsupported command channel".into()));
    }
    if frame.schema_version != COMMAND_SCHEMA_VERSION {
        return Err(Error::Parse(format!(
            "unsupported command schema_version={} expected={COMMAND_SCHEMA_VERSION}",
            frame.schema_version
        )));
    }
    let computed = frame.message.crc32()?;
    if computed != frame.crc32 {
        return Err(Error::ChecksumMismatch);
    }
    validate_command_message(&frame.message)?;
    Ok(frame.message)
}

fn validate_command_message(msg: &CommandMessage) -> Result<()> {
    match msg {
        CommandMessage::Request {
            cmd, scratch_path, ..
        } => {
            if cmd.trim().is_empty() {
                return Err(Error::Parse("command must not be empty".into()));
            }
            if cmd.chars().count() > COMMAND_MAX_COMMAND_CHARS {
                return Err(Error::Parse(format!(
                    "command length must be <= {COMMAND_MAX_COMMAND_CHARS} chars"
                )));
            }
            if let Some(path) = scratch_path {
                validate_cache_path(path)?;
            }
        }
        CommandMessage::Chunk { data, .. } => {
            if data.len() > COMMAND_MAX_CHUNK_BYTES {
                return Err(Error::Parse(format!(
                    "chunk exceeds {COMMAND_MAX_CHUNK_BYTES} bytes"
                )));
            }
        }
        CommandMessage::Error { message, .. } => {
            if message.trim().is_empty() {
                return Err(Error::Parse("error message must not be empty".into()));
            }
        }
        CommandMessage::Exit { .. }
        | CommandMessage::Ack { .. }
        | CommandMessage::Busy { .. }
        | CommandMessage::Heartbeat { .. } => {}
    }
    Ok(())
}

fn validate_cache_path(path: &str) -> Result<()> {
    if path.as_bytes().len() > COMMAND_MAX_SCRATCH_PATH_BYTES {
        return Err(Error::Parse(format!(
            "scratch_path must be <= {COMMAND_MAX_SCRATCH_PATH_BYTES} bytes"
        )));
    }
    let candidate = Path::new(path);
    if !candidate.starts_with(CACHE_DIR) {
        return Err(Error::Parse(format!(
            "scratch_path must live under {CACHE_DIR}: {path}"
        )));
    }
    Ok(())
}

#[derive(Debug, Deserialize)]
struct FrameTypeProbe {
    #[serde(rename = "type")]
    kind: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct CompressionEnvelopeOwned {
    #[serde(rename = "type")]
    frame_type: String,
    schema_version: u8,
    codec: String,
    original_len: u32,
    #[serde(with = "serde_bytes")]
    data: ByteBuf,
}

pub fn normalize_payload_json<'a>(raw: &'a str) -> Result<Cow<'a, str>> {
    let probe: FrameTypeProbe =
        serde_json::from_str(raw).map_err(|e| Error::Parse(format!("json: {e}")))?;
    if probe.kind.as_deref() != Some("compressed") {
        return Ok(Cow::Borrowed(raw));
    }

    let envelope: CompressionEnvelopeOwned =
        serde_json::from_str(raw).map_err(|e| Error::Parse(format!("compressed envelope: {e}")))?;
    if envelope.schema_version != DEFAULT_PROTOCOL_SCHEMA_VERSION {
        return Err(Error::Parse(format!(
            "unsupported compressed schema_version={} expected={DEFAULT_PROTOCOL_SCHEMA_VERSION}",
            envelope.schema_version
        )));
    }
    if envelope.frame_type.as_str() != "compressed" {
        return Err(Error::Parse(format!(
            "unsupported envelope type '{}'",
            envelope.frame_type
        )));
    }

    let codec = CompressionCodec::from_name(&envelope.codec).ok_or_else(|| {
        Error::Parse(format!(
            "unsupported compression codec '{}'",
            envelope.codec
        ))
    })?;
    let decompressed = decompress(envelope.data.as_ref(), codec)?;
    if decompressed.len() != envelope.original_len as usize {
        return Err(Error::Parse(format!(
            "compressed original_len={} but decoded={}",
            envelope.original_len,
            decompressed.len()
        )));
    }
    let payload = String::from_utf8(decompressed)
        .map_err(|_| Error::Parse("decompressed payload not utf-8".into()))?;
    Ok(Cow::Owned(payload))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Defaults {
    pub scroll_speed_ms: u64,
    pub page_timeout_ms: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "snake_case", deny_unknown_fields)]
pub struct Payload {
    pub line1: String,
    pub line2: String,
    #[serde(default)]
    pub schema_version: Option<u8>,

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
    #[serde(default)]
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
        let normalized = normalize_payload_json(raw)?;
        Self::from_normalized_payload_with_defaults(&normalized, defaults)
    }

    pub fn from_normalized_payload_with_defaults(raw: &str, defaults: Defaults) -> Result<Self> {
        let payload: Payload =
            serde_json::from_str(raw).map_err(|e| Error::Parse(format!("json: {e}")))?;

        // Schema versioning: require schema_version to be present and enforce
        // strict bounds for lengths, icon counts and labels in version 1+.
        const MAX_LINE_LENGTH: usize = 40; // hardware max columns
        const MAX_ICONS: usize = 4;
        const MAX_BAR_LABEL_LENGTH: usize = 40;

        let schema_version = match payload.schema_version {
            Some(v) => v,
            None => return Err(Error::Parse("schema_version is required".into())),
        };
        if schema_version >= 1 {
            if payload.line1.chars().count() > MAX_LINE_LENGTH {
                return Err(Error::Parse(format!(
                    "line1 must be <= {} chars",
                    MAX_LINE_LENGTH
                )));
            }
            if payload.line2.chars().count() > MAX_LINE_LENGTH {
                return Err(Error::Parse(format!(
                    "line2 must be <= {} chars",
                    MAX_LINE_LENGTH
                )));
            }
            if let Some(icons) = &payload.icons {
                if icons.len() > MAX_ICONS {
                    return Err(Error::Parse(format!(
                        "icons must be <= {} items",
                        MAX_ICONS
                    )));
                }
            }
            if let Some(label) = &payload.bar_label {
                if label.chars().count() > MAX_BAR_LABEL_LENGTH {
                    return Err(Error::Parse(format!(
                        "bar_label must be <= {} chars",
                        MAX_BAR_LABEL_LENGTH
                    )));
                }
            }
        }

        if let Some(bar_max) = payload.bar_max {
            if bar_max < 1 {
                return Err(Error::Parse("bar_max must be >= 1".into()));
            }
        }
        if let (Some(value), Some(max)) = (payload.bar_value, payload.bar_max) {
            if value > max {
                return Err(Error::Parse("bar_value must be <= bar_max".into()));
            }
        }
        if let Some(timeout) = payload.page_timeout_ms {
            if timeout == 0 {
                return Err(Error::Parse("page_timeout_ms must be > 0".into()));
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
        let scroll_speed_ms = payload.scroll_speed_ms.unwrap_or(defaults.scroll_speed_ms);
        let page_timeout_ms = payload.page_timeout_ms.unwrap_or(defaults.page_timeout_ms);

        let bar_percent = compute_bar_percent(&payload);
        let bar_row = if bar_percent.is_some() {
            if payload.bar_line1.unwrap_or(false) {
                Some(0)
            } else {
                Some(1)
            }
        } else {
            None
        };

        let mode = DisplayMode::parse(payload.mode.clone());
        let icons = parse_icons(payload.icons.clone());

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
    use crate::compression::{compress, CompressionCodec};
    use serde::Serialize;
    use serde_bytes::ByteBuf;

    fn parse(raw: &str) -> RenderFrame {
        RenderFrame::from_payload_json(raw).unwrap()
    }

    fn parse_with_defaults(raw: &str, defaults: Defaults) -> RenderFrame {
        RenderFrame::from_payload_json_with_defaults(raw, defaults).unwrap()
    }

    #[test]
    fn parses_basic_payload_with_defaults() {
        let raw = r#"{"schema_version":1,"line1":"Hello","line2":"World"}"#;
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
        let raw = r#"{"schema_version":1,"line1":"","line2":"","bar_value":500,"bar_max":1000}"#;
        let frame = RenderFrame::from_payload_json(raw).unwrap();
        assert_eq!(frame.bar_percent, Some(50));
    }

    #[test]
    fn bar_field_takes_priority() {
        let raw =
            r#"{"schema_version":1,"line1":"","line2":"","bar":42,"bar_value":10,"bar_max":20}"#;
        let frame = RenderFrame::from_payload_json(raw).unwrap();
        assert_eq!(frame.bar_percent, Some(42));
    }

    #[test]
    fn scroll_can_be_disabled() {
        let raw =
            r#"{"schema_version":1,"line1":"LongLineThatWillNotScroll","line2":"","scroll":false}"#;
        let frame = RenderFrame::from_payload_json(raw).unwrap();
        assert!(!frame.scroll_enabled);
    }

    #[test]
    fn checksum_validates() {
        let payload = Payload {
            line1: "Hi".into(),
            line2: "There".into(),
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
            schema_version: Some(1),
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
    fn checksum_validates_with_schema_v1() {
        let payload = Payload {
            line1: "Hi".into(),
            line2: "There".into(),
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
            schema_version: Some(1),
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
        let raw = r#"{"schema_version":1,"line1":"A","line2":"B","checksum":"deadbeef"}"#;
        let err = RenderFrame::from_payload_json(raw).unwrap_err();
        assert!(matches!(err, Error::ChecksumMismatch));
    }

    #[test]
    fn duration_ms_supports_new_name_only() {
        let raw_new = r#"{"schema_version":1,"line1":"","line2":"","duration_ms":1234}"#;
        let frame_new = RenderFrame::from_payload_json(raw_new).unwrap();
        assert_eq!(frame_new.duration_ms, Some(1234));

        // Older `ttl_ms` alias removed; ensure old name now fails
        let raw_legacy = r#"{"schema_version":1,"line1":"","line2":"","ttl_ms":2345}"#;
        let err = RenderFrame::from_payload_json(raw_legacy).unwrap_err();
        assert!(format!("{err}").contains("json"));
    }

    #[test]
    fn backlight_can_be_disabled() {
        let raw = r#"{"schema_version":1,"line1":"","line2":"","backlight":false}"#;
        let frame = parse(raw);
        assert!(!frame.backlight_on);
    }

    #[test]
    fn blink_defaults_false_and_can_enable() {
        let raw_default = r#"{"schema_version":1,"line1":"","line2":""}"#;
        let default_frame = parse(raw_default);
        assert!(!default_frame.blink);

        let raw_blink = r#"{"schema_version":1,"line1":"","line2":"","blink":true}"#;
        let blinking_frame = parse(raw_blink);
        assert!(blinking_frame.blink);
    }

    #[test]
    fn scroll_speed_override_respected() {
        let raw = r#"{"schema_version":1,"line1":"","line2":"","scroll_speed_ms":123}"#;
        let frame = parse(raw);
        assert_eq!(frame.scroll_speed_ms, 123);
    }

    #[test]
    fn page_timeout_override_respected() {
        let raw = r#"{"schema_version":1,"line1":"","line2":"","page_timeout_ms":3210}"#;
        let frame = parse(raw);
        assert_eq!(frame.page_timeout_ms, 3210);
    }

    #[test]
    fn bar_value_exceeding_max_rejected() {
        let raw = r#"{"schema_version":1,"line1":"","line2":"","bar_value":150,"bar_max":100}"#;
        let err = RenderFrame::from_payload_json(raw).unwrap_err();
        assert!(format!("{err}").contains("bar_value"));
    }

    #[test]
    fn bar_value_handles_zero_max() {
        let raw = r#"{"schema_version":1,"line1":"","line2":"","bar_value":0,"bar_max":0}"#;
        let err = RenderFrame::from_payload_json(raw).unwrap_err();
        assert!(format!("{err}").contains("bar_max"));
    }

    #[test]
    fn bar_row_defaults_to_bottom() {
        let raw = r#"{"schema_version":1,"line1":"","line2":"","bar":10}"#;
        let frame = parse(raw);
        assert_eq!(frame.bar_row, Some(1));
    }

    #[test]
    fn bar_row_can_be_top_when_requested() {
        let raw = r#"{"schema_version":1,"line1":"","line2":"","bar":55,"bar_line1":true}"#;
        let frame = parse(raw);
        assert_eq!(frame.bar_row, Some(0));
    }

    #[test]
    fn dashboard_mode_forces_bar_bottom() {
        let raw = r#"{"schema_version":1,"line1":"","line2":"","bar":88,"bar_line1":true,"mode":"dashboard"}"#;
        let frame = parse(raw);
        assert_eq!(frame.bar_row, Some(1));
    }

    #[test]
    fn banner_mode_clears_second_line() {
        let raw = r#"{"schema_version":1,"line1":"Banner text","line2":"ignored","mode":"banner"}"#;
        let frame = parse(raw);
        assert_eq!(frame.line2, "");
    }

    #[test]
    fn icons_parse_and_ignore_unknown() {
        let raw = r#"{"schema_version":1,"line1":"","line2":"","icons":["battery","unknown","heart","ARROW"]}"#;
        let frame = parse(raw);
        assert_eq!(frame.icons, vec![Icon::Battery, Icon::Heart, Icon::Arrow]);
    }

    #[test]
    fn config_reload_flag_can_enable() {
        let raw_true = r#"{"schema_version":1,"line1":"","line2":"","config_reload":true}"#;
        let frame_true = parse(raw_true);
        assert!(frame_true.config_reload);

        let raw_default = r#"{"schema_version":1,"line1":"","line2":""}"#;
        let frame_default = parse(raw_default);
        assert!(!frame_default.config_reload);
    }

    #[test]
    fn clear_and_test_flags_default_false_and_true() {
        let raw_default = r#"{"schema_version":1,"line1":"","line2":""}"#;
        let frame_default = parse(raw_default);
        assert!(!frame_default.clear);
        assert!(!frame_default.test);

        let raw_true = r#"{"schema_version":1,"line1":"","line2":"","clear":true,"test":true}"#;
        let frame_true = parse(raw_true);
        assert!(frame_true.clear);
        assert!(frame_true.test);
    }

    #[test]
    fn defaults_can_override_scroll_and_page_timeout() {
        let raw = r#"{"schema_version":1,"line1":"","line2":""}"#;
        let frame = parse_with_defaults(
            raw,
            Defaults {
                scroll_speed_ms: 999,
                page_timeout_ms: 7777,
            },
        );
        assert_eq!(frame.scroll_speed_ms, 999);
        assert_eq!(frame.page_timeout_ms, 7777);
    }

    #[test]
    fn rejects_bar_max_below_one() {
        let raw = r#"{"schema_version":1,"line1":"","line2":"","bar_value":10,"bar_max":0}"#;
        let err = RenderFrame::from_payload_json(raw).unwrap_err();
        assert!(format!("{err}").contains("bar_max"));
    }

    #[test]
    fn rejects_bar_value_above_max() {
        let raw = r#"{"schema_version":1,"line1":"","line2":"","bar_value":101,"bar_max":100}"#;
        let err = RenderFrame::from_payload_json(raw).unwrap_err();
        assert!(format!("{err}").contains("bar_value"));
    }

    #[test]
    fn rejects_zero_page_timeout() {
        let raw = r#"{"schema_version":1,"line1":"","line2":"","page_timeout_ms":0}"#;
        let err = RenderFrame::from_payload_json(raw).unwrap_err();
        assert!(format!("{err}").contains("page_timeout_ms"));
    }

    #[test]
    fn schema_v1_rejects_long_lines() {
        let long = "A".repeat(41);
        let raw = format!(r#"{{"schema_version":1,"line1":"{}","line2":""}}"#, long);
        let err = RenderFrame::from_payload_json(&raw).unwrap_err();
        assert!(format!("{err}").contains("line1"));
    }

    #[test]
    fn legacy_payload_allows_long_lines() {
        // No schema_version - legacy payloads are no longer supported and should be rejected
        let long = "A".repeat(80);
        let raw = format!(r#"{{"line1":"{}","line2":""}}"#, long);
        let err = RenderFrame::from_payload_json(&raw).unwrap_err();
        assert!(format!("{err}").contains("schema_version"));
    }

    #[test]
    fn schema_v1_rejects_too_many_icons() {
        let raw = r#"{"schema_version":1,"line1":"","line2":"","icons":["one","two","three","four","five"]}"#;
        let err = RenderFrame::from_payload_json(raw).unwrap_err();
        assert!(format!("{err}").contains("icons"));
    }

    #[test]
    fn schema_v1_rejects_long_bar_label() {
        let long_label = "L".repeat(41);
        let raw = format!(
            r#"{{"schema_version":1,"line1":"","line2":"","bar_label":"{}"}}"#,
            long_label
        );
        let err = RenderFrame::from_payload_json(&raw).unwrap_err();
        assert!(format!("{err}").contains("bar_label"));
    }

    #[test]
    fn schema_v1_allows_valid_frame() {
        let raw = r#"{"schema_version":1,"line1":"Hello","line2":"World","icons":["battery","heart","arrow","wifi"]}"#;
        let frame = RenderFrame::from_payload_json(raw).unwrap();
        assert_eq!(frame.icons.len(), 4);
    }

    #[test]
    fn command_frame_round_trip() {
        let msg = CommandMessage::Request {
            request_id: 7,
            cmd: "uptime".into(),
            scratch_path: Some(format!("{}/tunnel/req7", crate::CACHE_DIR)),
        };
        let encoded = encode_command_frame(&msg).unwrap();
        let decoded = decode_command_frame(&encoded).unwrap();
        assert!(matches!(
            decoded,
            CommandMessage::Request { request_id: 7, .. }
        ));
    }

    #[test]
    fn command_frame_rejects_external_cache_path() {
        let msg = CommandMessage::Request {
            request_id: 1,
            cmd: "whoami".into(),
            scratch_path: Some("/tmp/out".into()),
        };
        let err = encode_command_frame(&msg).unwrap_err();
        assert!(format!("{err}").contains("scratch_path"));
    }

    #[test]
    fn command_frame_rejects_long_command() {
        let mut cmd = "a".repeat(COMMAND_MAX_COMMAND_CHARS);
        cmd.push('x');
        let msg = CommandMessage::Request {
            request_id: 2,
            cmd,
            scratch_path: None,
        };
        let err = encode_command_frame(&msg).unwrap_err();
        assert!(format!("{err}").contains("command length"));
    }

    #[test]
    fn command_frame_rejects_large_chunk() {
        let msg = CommandMessage::Chunk {
            request_id: 3,
            stream: CommandStream::Stdout,
            seq: 0,
            data: ByteBuf::from(vec![0u8; COMMAND_MAX_CHUNK_BYTES + 1]),
        };
        let err = encode_command_frame(&msg).unwrap_err();
        assert!(format!("{err}").contains("chunk exceeds"));
    }

    #[derive(Serialize)]
    struct TestEnvelope {
        #[serde(rename = "type")]
        kind: &'static str,
        schema_version: u8,
        codec: &'static str,
        original_len: u32,
        #[serde(with = "serde_bytes")]
        data: ByteBuf,
    }

    #[test]
    fn compressed_envelope_round_trips() {
        let payload = r#"{"schema_version":1,"line1":"HELLO","line2":"WORLD"}"#;
        let compressed = compress(payload.as_bytes(), CompressionCodec::Lz4).unwrap();
        let envelope = TestEnvelope {
            kind: "compressed",
            schema_version: 1,
            codec: "lz4",
            original_len: payload.len() as u32,
            data: ByteBuf::from(compressed),
        };
        let raw = serde_json::to_string(&envelope).unwrap();
        let frame = RenderFrame::from_payload_json(&raw).unwrap();
        assert_eq!(frame.line1, "HELLO");
        assert_eq!(frame.line2, "WORLD");
    }

    #[test]
    fn compressed_envelope_rejects_unknown_codec() {
        let payload = r#"{"schema_version":1,"line1":"HELLO","line2":"WORLD"}"#;
        let compressed = compress(payload.as_bytes(), CompressionCodec::Lz4).unwrap();
        let envelope = TestEnvelope {
            kind: "compressed",
            schema_version: 1,
            codec: "lz4",
            original_len: payload.len() as u32,
            data: ByteBuf::from(compressed),
        };
        let mut value: serde_json::Value = serde_json::to_value(&envelope).unwrap();
        if let serde_json::Value::Object(ref mut map) = value {
            map.insert("codec".into(), serde_json::Value::String("snappy".into()));
        }
        let raw = serde_json::to_string(&value).unwrap();
        let err = RenderFrame::from_payload_json(&raw).unwrap_err();
        assert!(format!("{err}").contains("unsupported compression codec"));
    }
}
