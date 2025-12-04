use std::collections::{HashMap, HashSet};

use crate::{display::lcd::Lcd, payload::Icon, Result};

pub trait GlyphWriter {
    fn write_glyph(&mut self, slot: u8, bitmap: &[u8; 8]) -> Result<()>;
}

impl GlyphWriter for Lcd {
    fn write_glyph(&mut self, slot: u8, bitmap: &[u8; 8]) -> Result<()> {
        self.write_custom_char(slot, bitmap)
    }
}

const MAX_SLOTS: usize = 8;
const BAR_LEVEL_COUNT: usize = 6;
const BAR_FALLBACK_CHARS: [char; BAR_LEVEL_COUNT] = [' ', '.', ':', '-', '=', '#'];
const HEARTBEAT_FALLBACK_CHAR: char = 'h';

const BAR_BITMAPS: [[u8; 8]; BAR_LEVEL_COUNT] = [
    [0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00],
    [0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x10, 0x10],
    [0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18, 0x18],
    [0x1c, 0x1c, 0x1c, 0x1c, 0x1c, 0x1c, 0x1c, 0x1c],
    [0x1e, 0x1e, 0x1e, 0x1e, 0x1e, 0x1e, 0x1e, 0x1e],
    [0x1f, 0x1f, 0x1f, 0x1f, 0x1f, 0x1f, 0x1f, 0x1f],
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum GlyphKind {
    Bar(u8),
    Heartbeat,
    Icon(Icon),
}

#[derive(Clone, Copy, Debug)]
struct SlotEntry {
    kind: GlyphKind,
    stamp: u64,
}

pub struct IconPalette {
    bar_chars: [Option<char>; BAR_LEVEL_COUNT],
    heartbeat_char: Option<char>,
    icon_chars: HashMap<Icon, char>,
    pub missing_icons: Vec<Icon>,
}

impl IconPalette {
    fn new() -> Self {
        Self {
            bar_chars: [None; BAR_LEVEL_COUNT],
            heartbeat_char: None,
            icon_chars: HashMap::new(),
            missing_icons: Vec::new(),
        }
    }

    fn register(&mut self, kind: GlyphKind, ch: char) {
        match kind {
            GlyphKind::Bar(level) => {
                if let Some(dest) = self.bar_chars.get_mut(level as usize) {
                    *dest = Some(ch);
                }
            }
            GlyphKind::Heartbeat => self.heartbeat_char = Some(ch),
            GlyphKind::Icon(icon) => {
                self.icon_chars.insert(icon, ch);
            }
        }
    }

    fn record_missing(&mut self, kind: GlyphKind) {
        if let GlyphKind::Icon(icon) = kind {
            self.missing_icons.push(icon);
        }
    }

    fn record_missing_icon(&mut self, icon: Icon) {
        self.missing_icons.push(icon);
    }

    pub fn bar_char(&self, level: usize) -> char {
        self.bar_chars
            .get(level)
            .and_then(|ch| *ch)
            .unwrap_or(BAR_FALLBACK_CHARS[level.min(BAR_FALLBACK_CHARS.len() - 1)])
    }

    pub fn heartbeat_char(&self) -> char {
        self.heartbeat_char.unwrap_or(HEARTBEAT_FALLBACK_CHAR)
    }

    pub fn icon_char(&self, icon: Icon) -> Option<char> {
        self.icon_chars.get(&icon).copied()
    }
}

pub struct IconBank {
    slots: [Option<SlotEntry>; MAX_SLOTS],
    next_stamp: u64,
}

impl IconBank {
    pub fn new() -> Self {
        Self {
            slots: [None; MAX_SLOTS],
            next_stamp: 0,
        }
    }

    pub fn build_palette<W: GlyphWriter>(
        &mut self,
        writer: &mut W,
        request: PaletteRequest<'_>,
    ) -> Result<IconPalette> {
        let mut palette = IconPalette::new();
        let mut required: Vec<GlyphKind> = Vec::new();

        if request.bar_required {
            for level in 0..BAR_LEVEL_COUNT {
                required.push(GlyphKind::Bar(level as u8));
            }
        }

        if request.heartbeat {
            required.push(GlyphKind::Heartbeat);
        }

        for icon in request.icons {
            if icon.bitmap().is_some() {
                required.push(GlyphKind::Icon(*icon));
            } else {
                palette.record_missing_icon(*icon);
            }
        }

        let required_set: HashSet<GlyphKind> = required.iter().copied().collect();
        for kind in required {
            match self.ensure_glyph(kind, &required_set, writer)? {
                Some(ch) => palette.register(kind, ch),
                None => palette.record_missing(kind),
            }
        }

        Ok(palette)
    }

    fn ensure_glyph<W: GlyphWriter>(
        &mut self,
        kind: GlyphKind,
        required: &HashSet<GlyphKind>,
        writer: &mut W,
    ) -> Result<Option<char>> {
        if let Some(idx) = self.slot_for_kind(kind) {
            let stamp = self.bump_stamp();
            if let Some(entry) = self.slots[idx].as_mut() {
                entry.stamp = stamp;
            }
            return Ok(Some(slot_to_char(idx)));
        }

        if let Some(idx) = self.find_free_slot() {
            if self.load_slot(idx, kind, writer)? {
                return Ok(Some(slot_to_char(idx)));
            }
            return Ok(None);
        }

        if let Some(idx) = self.find_evict_slot(required) {
            if self.load_slot(idx, kind, writer)? {
                return Ok(Some(slot_to_char(idx)));
            }
            return Ok(None);
        }

        Ok(None)
    }

    fn load_slot<W: GlyphWriter>(
        &mut self,
        idx: usize,
        kind: GlyphKind,
        writer: &mut W,
    ) -> Result<bool> {
        let Some(bitmap) = bitmap_for(kind) else {
            return Ok(false);
        };
        writer.write_glyph(idx as u8, &bitmap)?;
        self.slots[idx] = Some(SlotEntry {
            kind,
            stamp: self.bump_stamp(),
        });
        Ok(true)
    }

    fn slot_for_kind(&self, kind: GlyphKind) -> Option<usize> {
        self.slots
            .iter()
            .enumerate()
            .find(|(_, entry)| entry.as_ref().map(|info| info.kind) == Some(kind))
            .map(|(idx, _)| idx)
    }

    fn find_free_slot(&self) -> Option<usize> {
        self.slots
            .iter()
            .enumerate()
            .find(|(_, entry)| entry.is_none())
            .map(|(idx, _)| idx)
    }

    fn find_evict_slot(&self, required: &HashSet<GlyphKind>) -> Option<usize> {
        self.slots
            .iter()
            .enumerate()
            .filter(|(_, entry)| {
                entry
                    .as_ref()
                    .map(|info| !required.contains(&info.kind))
                    .unwrap_or(false)
            })
            .min_by_key(|(_, entry)| entry.as_ref().map(|info| info.stamp).unwrap_or(u64::MAX))
            .map(|(idx, _)| idx)
    }

    fn bump_stamp(&mut self) -> u64 {
        let current = self.next_stamp;
        self.next_stamp = self.next_stamp.wrapping_add(1);
        current
    }
}

fn bitmap_for(kind: GlyphKind) -> Option<[u8; 8]> {
    match kind {
        GlyphKind::Bar(level) => BAR_BITMAPS.get(level as usize).copied(),
        GlyphKind::Heartbeat => Icon::Heart.bitmap(),
        GlyphKind::Icon(icon) => icon.bitmap(),
    }
}

fn slot_to_char(idx: usize) -> char {
    char::from_u32((idx & 0xFF) as u32).unwrap_or(' ')
}

#[derive(Clone, Copy)]
pub struct PaletteRequest<'a> {
    pub bar_required: bool,
    pub heartbeat: bool,
    pub icons: &'a [Icon],
}

impl Default for IconPalette {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct TestWriter {
        writes: Vec<(u8, [u8; 8])>,
    }

    impl GlyphWriter for TestWriter {
        fn write_glyph(&mut self, slot: u8, bitmap: &[u8; 8]) -> Result<()> {
            self.writes.push((slot, *bitmap));
            Ok(())
        }
    }

    #[test]
    fn reuses_slots_for_repeated_icons() {
        let mut bank = IconBank::new();
        let mut writer = TestWriter::default();
        let icon_list = [Icon::Battery];
        let request = PaletteRequest {
            bar_required: false,
            heartbeat: false,
            icons: &icon_list,
        };

        let palette = bank.build_palette(&mut writer, request).unwrap();
        assert_eq!(palette.missing_icons.len(), 0);
        assert_eq!(palette.icon_char(Icon::Battery).map(|c| c as u8), Some(0));
        let first_writes = writer.writes.len();
        assert!(first_writes > 0, "initial glyph write should occur");

        let palette = bank.build_palette(&mut writer, request).unwrap();
        assert_eq!(palette.missing_icons.len(), 0);
        assert_eq!(
            writer.writes.len(),
            first_writes,
            "subsequent call should reuse slot"
        );
        assert_eq!(palette.icon_char(Icon::Battery).map(|c| c as u8), Some(0));
    }

    #[test]
    fn reports_missing_icons_when_capacity_exceeded() {
        let mut bank = IconBank::new();
        let mut writer = TestWriter::default();
        let icons = [
            Icon::Arrow,
            Icon::Bell,
            Icon::Note,
            Icon::Clockface,
            Icon::Duck,
        ];
        let palette = bank
            .build_palette(
                &mut writer,
                PaletteRequest {
                    bar_required: true,
                    heartbeat: true,
                    icons: &icons,
                },
            )
            .unwrap();

        // With bar + heartbeat active only one icon fits; the rest fall back to ASCII.
        assert!(palette.icon_char(icons[0]).is_some());
        assert_eq!(palette.missing_icons.len(), icons.len() - 1);
        assert!(palette
            .missing_icons
            .iter()
            .all(|icon| icons[1..].contains(icon)));
    }
}
