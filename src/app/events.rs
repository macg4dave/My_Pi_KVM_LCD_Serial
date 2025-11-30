/// Stores scroll offsets for the two LCD lines to avoid ad-hoc tuples.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ScrollOffsets {
    pub top: usize,
    pub bottom: usize,
}

impl ScrollOffsets {
    pub fn zero() -> Self {
        Self { top: 0, bottom: 0 }
    }

    pub fn update(self, top: usize, bottom: usize) -> Self {
        Self { top, bottom }
    }
}
