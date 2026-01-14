//! Domain types for coordinates and positions
//!
//! These newtype wrappers provide type safety for coordinate operations,
//! preventing confusion between buffer coordinates and screen coordinates.

/// Line number in a buffer (0-indexed internally)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct LineNumber(pub usize);

/// Column position in a line (0-indexed)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct Column(pub usize);

/// Row on the screen (0-indexed from top)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct ScreenRow(pub u16);

/// Column on the screen (0-indexed from left)
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default)]
pub struct ScreenCol(pub u16);

impl LineNumber {
    #[must_use]
    pub const fn new(n: usize) -> Self {
        Self(n)
    }

    #[must_use]
    pub const fn as_usize(self) -> usize {
        self.0
    }

    #[must_use]
    pub const fn saturating_sub(self, n: usize) -> Self {
        Self(self.0.saturating_sub(n))
    }
}

impl Column {
    #[must_use]
    pub const fn new(n: usize) -> Self {
        Self(n)
    }

    #[must_use]
    pub const fn as_usize(self) -> usize {
        self.0
    }

    #[must_use]
    pub const fn saturating_sub(self, n: usize) -> Self {
        Self(self.0.saturating_sub(n))
    }
}

impl ScreenRow {
    #[must_use]
    pub const fn new(n: u16) -> Self {
        Self(n)
    }

    #[must_use]
    pub const fn as_u16(self) -> u16 {
        self.0
    }

    #[must_use]
    pub const fn saturating_sub(self, n: u16) -> Self {
        Self(self.0.saturating_sub(n))
    }
}

impl ScreenCol {
    #[must_use]
    pub const fn new(n: u16) -> Self {
        Self(n)
    }

    #[must_use]
    pub const fn as_u16(self) -> u16 {
        self.0
    }

    #[must_use]
    pub const fn saturating_sub(self, n: u16) -> Self {
        Self(self.0.saturating_sub(n))
    }
}

// Conversion traits for gradual migration
impl From<usize> for LineNumber {
    fn from(n: usize) -> Self {
        Self(n)
    }
}

impl From<usize> for Column {
    fn from(n: usize) -> Self {
        Self(n)
    }
}

impl From<u16> for ScreenRow {
    fn from(n: u16) -> Self {
        Self(n)
    }
}

impl From<u16> for ScreenCol {
    fn from(n: u16) -> Self {
        Self(n)
    }
}

impl From<LineNumber> for usize {
    fn from(n: LineNumber) -> Self {
        n.0
    }
}

impl From<Column> for usize {
    fn from(n: Column) -> Self {
        n.0
    }
}

impl From<ScreenRow> for u16 {
    fn from(n: ScreenRow) -> Self {
        n.0
    }
}

impl From<ScreenCol> for u16 {
    fn from(n: ScreenCol) -> Self {
        n.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_line_number_operations() {
        let ln = LineNumber::new(5);
        assert_eq!(ln.as_usize(), 5);
        assert_eq!(ln.saturating_sub(2).as_usize(), 3);
        assert_eq!(ln.saturating_sub(10).as_usize(), 0);
    }

    #[test]
    fn test_column_operations() {
        let col = Column::new(10);
        assert_eq!(col.as_usize(), 10);
        assert_eq!(col.saturating_sub(5).as_usize(), 5);
    }

    #[test]
    fn test_screen_row_operations() {
        let row = ScreenRow::new(20);
        assert_eq!(row.as_u16(), 20);
        assert_eq!(row.saturating_sub(5).as_u16(), 15);
    }

    #[test]
    fn test_conversions() {
        let ln: LineNumber = 5usize.into();
        let n: usize = ln.into();
        assert_eq!(n, 5);

        let row: ScreenRow = 10u16.into();
        let r: u16 = row.into();
        assert_eq!(r, 10);
    }
}
