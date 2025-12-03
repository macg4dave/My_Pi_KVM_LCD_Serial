use std::{
    collections::VecDeque,
    time::{Duration, Instant},
};

use crc32fast::Hasher;

use crate::{
    payload::{Defaults, RenderFrame, DEFAULT_PAGE_TIMEOUT_MS, DEFAULT_SCROLL_MS},
    Error, Result,
};

#[derive(Clone)]
struct FrameEntry {
    frame: RenderFrame,
    expires_at: Option<Instant>,
}

pub const MAX_FRAME_BYTES: usize = 512;

/// Maintains a queue of render frames and deduplicates identical payloads.
pub struct RenderState {
    pages: VecDeque<FrameEntry>,
    last_crc: Option<u32>,
    defaults: Defaults,
}

impl RenderState {
    pub fn new(defaults: Option<Defaults>) -> Self {
        Self {
            pages: VecDeque::new(),
            last_crc: None,
            defaults: defaults.unwrap_or(Defaults {
                scroll_speed_ms: DEFAULT_SCROLL_MS,
                page_timeout_ms: DEFAULT_PAGE_TIMEOUT_MS,
            }),
        }
    }

    /// Ingest a JSON frame string. Returns Some(frame) if it is new, None if duplicate.
    pub fn ingest(&mut self, raw: &str) -> Result<Option<RenderFrame>> {
        self.prune_expired(Instant::now());
        if raw.len() > MAX_FRAME_BYTES {
            return Err(Error::Parse(format!(
                "frame exceeds {MAX_FRAME_BYTES} bytes"
            )));
        }

        let crc = checksum_raw(raw);
        if self.last_crc == Some(crc) {
            return Ok(None);
        }
        let frame = RenderFrame::from_payload_json_with_defaults(raw, self.defaults)?;
        let expires_at = frame
            .duration_ms
            .map(|ms| Instant::now() + Duration::from_millis(ms));
        self.last_crc = Some(crc);
        self.pages.push_back(FrameEntry {
            frame: frame.clone(),
            expires_at,
        });
        Ok(Some(frame))
    }

    /// Advance to the next page/frame if available.
    pub fn next_page(&mut self) -> Option<RenderFrame> {
        self.prune_expired(Instant::now());
        let front = self.pages.pop_front()?;
        let frame = front.frame.clone();
        self.pages.push_back(front);
        Some(frame)
    }

    /// Get the current frame without rotating.
    pub fn current(&mut self) -> Option<&RenderFrame> {
        self.prune_expired(Instant::now());
        self.pages.front().map(|f| &f.frame)
    }

    pub fn len(&mut self) -> usize {
        self.prune_expired(Instant::now());
        self.pages.len()
    }

    pub fn is_empty(&mut self) -> bool {
        self.prune_expired(Instant::now());
        self.pages.is_empty()
    }

    pub fn set_defaults(&mut self, defaults: Defaults) {
        self.defaults = defaults;
    }

    fn prune_expired(&mut self, now: Instant) {
        // Drop expired frames so the queue reflects currently valid pages and CRC dedupe can reset.
        while let Some(front) = self.pages.front() {
            if let Some(expiry) = front.expires_at {
                if expiry <= now {
                    self.pages.pop_front();
                    continue;
                }
            }
            break;
        }
        if self.pages.is_empty() {
            self.last_crc = None;
        }
    }
}

fn checksum_raw(raw: &str) -> u32 {
    let mut hasher = Hasher::new();
    hasher.update(raw.as_bytes());
    hasher.finalize()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dedupes_identical_frames() {
        let mut state = RenderState::new(None);
        let raw = r#"{"schema_version":1,"line1":"A","line2":"B"}"#;
        let first = state.ingest(raw).unwrap();
        assert!(first.is_some());
        let second = state.ingest(raw).unwrap();
        assert!(second.is_none());
    }

    #[test]
    fn rotates_pages() {
        let mut state = RenderState::new(None);
        state
            .ingest(r#"{"schema_version":1,"line1":"A","line2":"B"}"#)
            .unwrap();
        state
            .ingest(r#"{"schema_version":1,"line1":"C","line2":"D"}"#)
            .unwrap();
        let first = state.next_page().unwrap();
        assert_eq!(first.line1, "A");
        let second = state.next_page().unwrap();
        assert_eq!(second.line1, "C");
        let third = state.next_page().unwrap();
        assert_eq!(third.line1, "A");
    }

    #[test]
    fn rejects_oversize_frame() {
        let mut state = RenderState::new(None);
        let long = format!(
            r#"{{"schema_version":1,"line1":"{}","line2":""}}"#,
            "x".repeat(MAX_FRAME_BYTES)
        );
        let err = state.ingest(&long).unwrap_err();
        assert!(format!("{err}").contains("exceeds"));
    }

    #[test]
    fn expires_frame_after_ttl() {
        let mut state = RenderState::new(None);
        state
            .ingest(r#"{"schema_version":1,"line1":"A","line2":"B","duration_ms":1}"#)
            .unwrap();
        assert_eq!(state.len(), 1);
        std::thread::sleep(std::time::Duration::from_millis(5));
        assert!(
            state.next_page().is_none(),
            "expired frame should be dropped"
        );
    }
}
