//! Multi-row layout for statusline.
//!
//! Provides section distribution across multiple rows when content overflows.
//!
//! # Layout Strategies
//!
//! When the statusline uses multiple rows, sections are distributed:
//!
//! ```text
//! Height = 1 (default):
//! ┌─────────────────────────────────────────────────────────┐
//! │  A │ B │ C                           │ X │ Y │ Z        │
//! └─────────────────────────────────────────────────────────┘
//!
//! Height = 2:
//! ┌─────────────────────────────────────────────────────────┐
//! │  Row 1: A │ B │ C (left sections)                       │
//! │  Row 2: X │ Y │ Z (right sections)                      │
//! └─────────────────────────────────────────────────────────┘
//!
//! Height = 3 (with overflow):
//! ┌─────────────────────────────────────────────────────────┐
//! │  Row 1: A │ B (mode, branch)                            │
//! │  Row 2: C (filename - can be very long now!)            │
//! │  Row 3: X │ Y │ Z (encoding, filetype, position)        │
//! └─────────────────────────────────────────────────────────┘
//! ```

use super::section::SectionId;

/// Multi-row layout for statusline sections.
///
/// Defines which sections appear on which row when the statusline
/// uses multiple rows.
#[derive(Debug, Clone)]
pub struct MultiRowLayout {
    /// Section assignments per row. Each inner Vec contains section IDs for that row.
    pub rows: Vec<Vec<SectionId>>,
}

impl Default for MultiRowLayout {
    fn default() -> Self {
        Self::single_row()
    }
}

impl MultiRowLayout {
    /// Create a single-row layout (all sections on one row).
    #[must_use]
    pub fn single_row() -> Self {
        Self {
            rows: vec![SectionId::ALL.to_vec()],
        }
    }

    /// Create a 2-row layout: left sections on row 1, right sections on row 2.
    ///
    /// ```text
    /// Row 1: A │ B │ C
    /// Row 2: X │ Y │ Z
    /// ```
    #[must_use]
    pub fn two_rows() -> Self {
        Self {
            rows: vec![SectionId::LEFT.to_vec(), SectionId::RIGHT.to_vec()],
        }
    }

    /// Create a 3-row layout: mode on row 1, filename on row 2, info on row 3.
    ///
    /// ```text
    /// Row 1: A │ B (mode, branch)
    /// Row 2: C (filename - full width available)
    /// Row 3: X │ Y │ Z (encoding, filetype, position)
    /// ```
    #[must_use]
    pub fn three_rows() -> Self {
        Self {
            rows: vec![
                vec![SectionId::A, SectionId::B],
                vec![SectionId::C],
                vec![SectionId::X, SectionId::Y, SectionId::Z],
            ],
        }
    }

    /// Create a custom layout with explicit row assignments.
    #[must_use]
    pub const fn custom(rows: Vec<Vec<SectionId>>) -> Self {
        Self { rows }
    }

    /// Get the number of rows in this layout.
    #[must_use]
    pub fn row_count(&self) -> u16 {
        // Safety: row count is at most 6 (one section per row), fits in u16
        #[allow(clippy::cast_possible_truncation)]
        let count = self.rows.len().min(usize::from(u16::MAX)) as u16;
        count.max(1)
    }

    /// Get sections for a specific row (0-indexed).
    #[must_use]
    pub fn sections_for_row(&self, row: usize) -> &[SectionId] {
        self.rows.get(row).map_or(&[], Vec::as_slice)
    }

    /// Find which row a section is assigned to.
    ///
    /// Returns `None` if the section is not in this layout.
    #[must_use]
    pub fn row_for_section(&self, section_id: SectionId) -> Option<usize> {
        self.rows.iter().position(|row| row.contains(&section_id))
    }

    /// Check if a section is in the layout.
    #[must_use]
    pub fn contains(&self, section_id: SectionId) -> bool {
        self.rows.iter().any(|row| row.contains(&section_id))
    }

    /// Get all sections in this layout in order.
    #[must_use]
    pub fn all_sections(&self) -> Vec<SectionId> {
        self.rows.iter().flatten().copied().collect()
    }
}

/// Layout calculator for determining the optimal multi-row layout.
#[derive(Debug, Default)]
pub struct LayoutCalculator {
    /// Available width for the statusline.
    available_width: u16,
}

impl LayoutCalculator {
    /// Create a new layout calculator.
    #[must_use]
    pub const fn new(available_width: u16) -> Self {
        Self { available_width }
    }

    /// Calculate the optimal layout based on section widths.
    ///
    /// # Arguments
    ///
    /// * `section_widths` - Width of each section (A through Z)
    /// * `max_rows` - Maximum allowed rows
    ///
    /// # Returns
    ///
    /// A [`MultiRowLayout`] optimized for the given widths and constraints.
    #[must_use]
    pub fn calculate(&self, section_widths: &[usize; 6], max_rows: u16) -> MultiRowLayout {
        let width = usize::from(self.available_width);

        // Total content width
        let total: usize = section_widths.iter().sum();

        // If everything fits in one row, use single row layout
        if total <= width || max_rows == 1 {
            return MultiRowLayout::single_row();
        }

        // Try 2-row layout
        let left_width: usize = section_widths[0..3].iter().sum();
        let right_width: usize = section_widths[3..6].iter().sum();

        if (left_width <= width && right_width <= width) || max_rows == 2 {
            return MultiRowLayout::two_rows();
        }

        // Try 3-row layout
        let ab_width = section_widths[0] + section_widths[1];
        let c_width = section_widths[2];
        let xyz_width: usize = section_widths[3..6].iter().sum();

        if (ab_width <= width && c_width <= width && xyz_width <= width) || max_rows == 3 {
            return MultiRowLayout::three_rows();
        }

        // Fallback to 3-row layout even if it doesn't fit perfectly
        MultiRowLayout::three_rows()
    }

    /// Calculate layout from content metrics.
    #[must_use]
    pub fn calculate_from_metrics(
        &self,
        metrics: &super::height::ContentMetrics,
        max_rows: u16,
    ) -> MultiRowLayout {
        self.calculate(&metrics.section_widths, max_rows)
    }
}

/// Row content for rendering.
///
/// Contains the sections assigned to a specific row along with
/// layout hints for rendering.
#[derive(Debug, Clone)]
pub struct RowContent {
    /// Row index (0-indexed).
    pub row_index: usize,
    /// Sections assigned to this row.
    pub sections: Vec<SectionId>,
    /// Whether this row contains only left sections.
    pub is_left_only: bool,
    /// Whether this row contains only right sections.
    pub is_right_only: bool,
}

impl RowContent {
    /// Create row content from a layout.
    #[must_use]
    pub fn from_layout(layout: &MultiRowLayout) -> Vec<Self> {
        layout
            .rows
            .iter()
            .enumerate()
            .map(|(row_index, sections)| {
                let is_left_only = sections
                    .iter()
                    .all(|id| matches!(id, SectionId::A | SectionId::B | SectionId::C));
                let is_right_only = sections
                    .iter()
                    .all(|id| matches!(id, SectionId::X | SectionId::Y | SectionId::Z));

                Self {
                    row_index,
                    sections: sections.clone(),
                    is_left_only,
                    is_right_only,
                }
            })
            .collect()
    }
}

#[cfg(test)]
#[path = "layout_tests.rs"]
mod tests;
