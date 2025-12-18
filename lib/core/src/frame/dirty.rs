//! Dirty tracking for optimized rendering

/// A rectangular region that has been marked dirty
#[derive(Debug, Clone, Copy)]
pub struct DirtyRect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl DirtyRect {
    /// Create a new dirty rectangle
    #[must_use]
    pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Check if a point is within this rectangle
    #[must_use]
    pub const fn contains(&self, x: u16, y: u16) -> bool {
        x >= self.x && x < self.x + self.width && y >= self.y && y < self.y + self.height
    }
}

/// Region-based dirty tracking
#[derive(Debug, Clone, Default)]
pub struct DirtyRegions {
    regions: Vec<DirtyRect>,
}

impl DirtyRegions {
    /// Create a new empty dirty region tracker
    #[must_use]
    pub const fn new() -> Self {
        Self {
            regions: Vec::new(),
        }
    }

    /// Mark a rectangular region as dirty
    pub fn mark_rect(&mut self, x: u16, y: u16, width: u16, height: u16) {
        self.regions.push(DirtyRect::new(x, y, width, height));
    }

    /// Mark a single row as dirty
    pub fn mark_row(&mut self, y: u16, screen_width: u16) {
        self.mark_rect(0, y, screen_width, 1);
    }

    /// Mark the entire screen as dirty
    pub fn mark_all(&mut self, width: u16, height: u16) {
        self.regions.clear();
        self.regions.push(DirtyRect::new(0, 0, width, height));
    }

    /// Clear all dirty regions
    pub fn clear(&mut self) {
        self.regions.clear();
    }

    /// Check if there are no dirty regions
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // Vec::is_empty not const stable
    pub fn is_empty(&self) -> bool {
        self.regions.is_empty()
    }

    /// Check if a cell is within any dirty region
    #[must_use]
    pub fn contains(&self, x: u16, y: u16) -> bool {
        self.regions.iter().any(|r| r.contains(x, y))
    }

    /// Iterate over all dirty regions
    pub fn iter(&self) -> impl Iterator<Item = &DirtyRect> {
        self.regions.iter()
    }

    /// Get the number of dirty regions
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // Vec::len not const stable
    pub fn len(&self) -> usize {
        self.regions.len()
    }
}

/// Cell-level dirty tracking using a compact bitset
pub struct DirtyCells {
    width: u16,
    height: u16,
    /// Packed bits, one per cell (64 cells per u64)
    bits: Vec<u64>,
}

impl DirtyCells {
    /// Create a new dirty cell tracker
    #[must_use]
    pub fn new(width: u16, height: u16) -> Self {
        let total_cells = usize::from(width) * usize::from(height);
        let num_u64s = total_cells.div_ceil(64);
        Self {
            width,
            height,
            bits: vec![0; num_u64s],
        }
    }

    /// Resize the tracker
    pub fn resize(&mut self, width: u16, height: u16) {
        self.width = width;
        self.height = height;
        let total_cells = usize::from(width) * usize::from(height);
        let num_u64s = total_cells.div_ceil(64);
        self.bits.resize(num_u64s, 0);
        self.clear();
    }

    /// Calculate bit index for a position
    #[inline]
    #[allow(clippy::missing_const_for_fn)] // uses casts
    fn bit_index(&self, x: u16, y: u16) -> usize {
        (y as usize) * (self.width as usize) + (x as usize)
    }

    /// Mark a cell as dirty
    pub fn mark(&mut self, x: u16, y: u16) {
        if x < self.width && y < self.height {
            let idx = self.bit_index(x, y);
            let word = idx / 64;
            let bit = idx % 64;
            if word < self.bits.len() {
                self.bits[word] |= 1u64 << bit;
            }
        }
    }

    /// Check if a cell is dirty
    #[must_use]
    pub fn is_dirty(&self, x: u16, y: u16) -> bool {
        if x >= self.width || y >= self.height {
            return false;
        }
        let idx = self.bit_index(x, y);
        let word = idx / 64;
        let bit = idx % 64;
        word < self.bits.len() && (self.bits[word] & (1u64 << bit)) != 0
    }

    /// Clear all dirty flags
    pub fn clear(&mut self) {
        self.bits.fill(0);
    }

    /// Mark all cells as dirty
    pub fn mark_all(&mut self) {
        self.bits.fill(u64::MAX);
    }

    /// Mark a rectangular region as dirty
    pub fn mark_rect(&mut self, x: u16, y: u16, width: u16, height: u16) {
        for row in y..y.saturating_add(height).min(self.height) {
            for col in x..x.saturating_add(width).min(self.width) {
                self.mark(col, row);
            }
        }
    }

    /// Count number of dirty cells
    #[must_use]
    pub fn count_dirty(&self) -> usize {
        self.bits.iter().map(|w| w.count_ones() as usize).sum()
    }

    /// Iterate over dirty cell positions
    pub fn dirty_cells(&self) -> impl Iterator<Item = (u16, u16)> + '_ {
        (0..self.height).flat_map(move |y| {
            (0..self.width)
                .filter(move |&x| self.is_dirty(x, y))
                .map(move |x| (x, y))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_dirty_regions() {
        let mut regions = DirtyRegions::new();
        assert!(regions.is_empty());

        regions.mark_rect(10, 5, 20, 10);
        assert!(!regions.is_empty());
        assert!(regions.contains(15, 10));
        assert!(!regions.contains(5, 5));

        regions.clear();
        assert!(regions.is_empty());
    }

    #[test]
    fn test_dirty_cells() {
        let mut cells = DirtyCells::new(80, 24);
        assert!(!cells.is_dirty(10, 10));

        cells.mark(10, 10);
        assert!(cells.is_dirty(10, 10));
        assert!(!cells.is_dirty(11, 10));

        cells.mark_rect(0, 0, 5, 5);
        assert!(cells.is_dirty(3, 3));

        let count = cells.count_dirty();
        assert!(count >= 26); // At least 25 from rect + 1 from mark

        cells.clear();
        assert!(!cells.is_dirty(10, 10));
    }

    #[test]
    fn test_dirty_rect_contains() {
        let rect = DirtyRect::new(10, 10, 5, 5);
        assert!(rect.contains(10, 10));
        assert!(rect.contains(14, 14));
        assert!(!rect.contains(15, 15));
        assert!(!rect.contains(9, 10));
    }
}
