//! Dynamic height calculation for statusline.
//!
//! Provides types and algorithms for calculating statusline height based on
//! screen size and content overflow.
//!
//! # Height Calculation
//!
//! The statusline height adapts to:
//! 1. **Screen size** - Larger screens can accommodate more rows
//! 2. **Content overflow** - Long content may need additional rows
//!
//! # Overflow Strategies
//!
//! When content doesn't fit in the available width:
//! - [`OverflowStrategy::Truncate`] - Truncate with "..." (default for narrow screens)
//! - [`OverflowStrategy::Wrap`] - Wrap content to next line
//! - [`OverflowStrategy::Redistribute`] - Move sections to next row (lualine-style)

/// Configuration for dynamic statusline height.
#[derive(Debug, Clone)]
pub struct HeightConfig {
    /// Minimum statusline height (default: 1).
    pub min_height: u16,
    /// Maximum statusline height (default: Some(3), None = unlimited).
    pub max_height: Option<u16>,
    /// Screen height thresholds for additional rows.
    pub screen_thresholds: Vec<ScreenThreshold>,
    /// How to handle content overflow.
    pub overflow_strategy: OverflowStrategy,
}

impl Default for HeightConfig {
    fn default() -> Self {
        Self {
            min_height: 1,
            max_height: Some(3),
            screen_thresholds: vec![
                ScreenThreshold {
                    min_screen_height: 40,
                    additional_rows: 1,
                },
                ScreenThreshold {
                    min_screen_height: 60,
                    additional_rows: 2,
                },
            ],
            overflow_strategy: OverflowStrategy::default(),
        }
    }
}

impl HeightConfig {
    /// Create a fixed-height configuration.
    #[must_use]
    pub const fn fixed(height: u16) -> Self {
        Self {
            min_height: height,
            max_height: Some(height),
            screen_thresholds: Vec::new(),
            overflow_strategy: OverflowStrategy::Truncate,
        }
    }

    /// Create a single-row configuration (default for P1).
    #[must_use]
    pub const fn single_row() -> Self {
        Self::fixed(1)
    }

    /// Create configuration with custom max height.
    #[must_use]
    pub const fn with_max_height(mut self, max: u16) -> Self {
        self.max_height = Some(max);
        self
    }

    /// Create configuration with unlimited height.
    #[must_use]
    pub const fn unlimited(mut self) -> Self {
        self.max_height = None;
        self
    }

    /// Set the overflow strategy.
    #[must_use]
    pub const fn with_overflow_strategy(mut self, strategy: OverflowStrategy) -> Self {
        self.overflow_strategy = strategy;
        self
    }
}

/// Screen height threshold for additional statusline rows.
///
/// When the terminal height meets or exceeds `min_screen_height`,
/// the statusline is allowed `additional_rows` more rows (on top of the base).
#[derive(Debug, Clone, Copy)]
pub struct ScreenThreshold {
    /// Minimum screen height to activate this threshold.
    pub min_screen_height: u16,
    /// Additional rows allowed at this threshold.
    pub additional_rows: u16,
}

/// Strategy for handling content overflow.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum OverflowStrategy {
    /// Truncate long content with "..." (default).
    ///
    /// ```text
    /// ┌───────────────────────────────────┐
    /// │ NORMAL │ very-long-filenam... │ 1:1 │
    /// └───────────────────────────────────┘
    /// ```
    #[default]
    Truncate,

    /// Wrap content to next line.
    ///
    /// ```text
    /// ┌───────────────────────────────────┐
    /// │ NORMAL │ very-long-filename-that  │
    /// │        │ -goes-on.rs [+]     │ 1:1 │
    /// └───────────────────────────────────┘
    /// ```
    Wrap,

    /// Move sections to next row (lualine-style, preferred).
    ///
    /// ```text
    /// ┌───────────────────────────────────┐
    /// │ NORMAL │ main │ very-long-file.rs │
    /// │ utf-8 unix │ rust │ 1:1  25% │
    /// └───────────────────────────────────┘
    /// ```
    ///
    /// Sections C/X/Y/Z move to row 2 when row 1 overflows.
    Redistribute,
}

impl OverflowStrategy {
    /// Check if this strategy may use multiple rows.
    #[must_use]
    pub const fn may_use_multiple_rows(&self) -> bool {
        matches!(self, Self::Wrap | Self::Redistribute)
    }
}

/// Result of height calculation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HeightResult {
    /// Calculated statusline height.
    pub height: u16,
    /// Whether content overflow triggered additional rows.
    pub has_overflow: bool,
    /// The overflow strategy being used.
    pub strategy: OverflowStrategy,
}

impl HeightResult {
    /// Create a new height result.
    #[must_use]
    pub const fn new(height: u16, has_overflow: bool, strategy: OverflowStrategy) -> Self {
        Self {
            height,
            has_overflow,
            strategy,
        }
    }

    /// Create a single-row result with no overflow.
    #[must_use]
    pub const fn single_row() -> Self {
        Self {
            height: 1,
            has_overflow: false,
            strategy: OverflowStrategy::Truncate,
        }
    }
}

use crate::statusline::section::{Section, SectionId, SectionPosition};

/// Metrics about rendered statusline content.
///
/// Used for overflow detection and multi-row layout decisions.
#[derive(Debug, Clone, Default)]
pub struct ContentMetrics {
    /// Total display width of all section content.
    pub total_width: usize,
    /// Width of left sections (A, B, C).
    pub left_width: usize,
    /// Width of right sections (X, Y, Z).
    pub right_width: usize,
    /// Individual section widths.
    pub section_widths: [usize; 6],
}

impl ContentMetrics {
    /// Calculate metrics from rendered sections.
    #[must_use]
    pub fn from_sections(sections: &[Section]) -> Self {
        let mut metrics = Self::default();

        for section in sections {
            let width = section.display_width();
            let idx = Self::section_index(section.id);
            metrics.section_widths[idx] = width;

            match section.position() {
                SectionPosition::Left => metrics.left_width += width,
                SectionPosition::Right => metrics.right_width += width,
            }
        }

        metrics.total_width = metrics.left_width + metrics.right_width;
        metrics
    }

    /// Get the index for a section ID.
    const fn section_index(id: SectionId) -> usize {
        match id {
            SectionId::A => 0,
            SectionId::B => 1,
            SectionId::C => 2,
            SectionId::X => 3,
            SectionId::Y => 4,
            SectionId::Z => 5,
        }
    }

    /// Check if content overflows the available width.
    #[must_use]
    pub fn overflows(&self, available_width: u16) -> bool {
        self.total_width > usize::from(available_width)
    }

    /// Calculate the minimum gap needed between left and right sections.
    ///
    /// Returns 0 if sections overlap, otherwise returns the gap width.
    #[must_use]
    pub fn gap_width(&self, available_width: u16) -> usize {
        let total = self.left_width + self.right_width;
        if total >= usize::from(available_width) {
            0
        } else {
            usize::from(available_width) - total
        }
    }

    /// Calculate how many rows are needed using the Wrap strategy.
    #[must_use]
    pub fn rows_for_wrap(&self, available_width: u16) -> u16 {
        if available_width == 0 {
            return 1;
        }
        let width = usize::from(available_width);
        let rows = self.total_width.div_ceil(width);
        // Safety: rows is bounded by total_width which fits in usize
        #[allow(clippy::cast_possible_truncation)]
        let rows_u16 = rows.min(usize::from(u16::MAX)) as u16;
        rows_u16.max(1)
    }

    /// Calculate how many rows are needed using the Redistribute strategy.
    ///
    /// Redistribute moves sections to new rows to avoid truncation:
    /// - Row 1: A, B (mode, branch)
    /// - Row 2: C (filename - can be long)
    /// - Row 3: X, Y, Z (encoding, filetype, position)
    #[must_use]
    pub fn rows_for_redistribute(&self, available_width: u16) -> u16 {
        let width = usize::from(available_width);
        if width == 0 {
            return 1;
        }

        // If everything fits in one row, use one row
        if self.total_width <= width {
            return 1;
        }

        // Try 2-row layout: left on row 1, right on row 2
        let left_fits = self.left_width <= width;
        let right_fits = self.right_width <= width;

        if left_fits && right_fits {
            return 2;
        }

        // Need 3 rows: A+B on row 1, C on row 2, X+Y+Z on row 3
        // Check if each group fits
        let ab_width = self.section_widths[0] + self.section_widths[1];
        let c_width = self.section_widths[2];
        let xyz_width = self.section_widths[3] + self.section_widths[4] + self.section_widths[5];

        if ab_width <= width && c_width <= width && xyz_width <= width {
            return 3;
        }

        // Fallback: just calculate based on total width
        self.rows_for_wrap(available_width)
    }
}

/// Calculate statusline height based on screen size and content.
///
/// # Arguments
///
/// * `screen_height` - Terminal height in rows
/// * `content_width` - Total width of all section content
/// * `available_width` - Available width for statusline
/// * `config` - Height configuration
///
/// # Returns
///
/// A [`HeightResult`] with the calculated height and overflow information.
#[must_use]
pub fn calculate_height(
    screen_height: u16,
    content_width: usize,
    available_width: u16,
    config: &HeightConfig,
) -> HeightResult {
    // 1. Calculate base height from screen thresholds
    let mut allowed_height = config.min_height;
    for threshold in &config.screen_thresholds {
        if screen_height >= threshold.min_screen_height {
            allowed_height = allowed_height.saturating_add(threshold.additional_rows);
        }
    }

    // 2. Cap at max_height if configured
    if let Some(max) = config.max_height {
        allowed_height = allowed_height.min(max);
    }

    // 3. Check for content overflow
    let has_overflow = content_width > usize::from(available_width);

    // 4. Calculate required rows based on overflow strategy
    let required_rows = if has_overflow && config.overflow_strategy.may_use_multiple_rows() {
        // Calculate how many rows the content needs
        let rows = content_width.div_ceil(usize::from(available_width));
        // Safety: rows is at most content_width which fits in usize, and we cap it below
        #[allow(clippy::cast_possible_truncation)]
        let rows_u16 = rows.min(usize::from(u16::MAX)) as u16;
        rows_u16.max(config.min_height)
    } else {
        config.min_height
    };

    // 5. Final height = max(min_height, required_rows), capped at allowed_height
    let height = required_rows.max(config.min_height).min(allowed_height);

    HeightResult::new(height, has_overflow, config.overflow_strategy)
}

/// Calculate statusline height from rendered sections.
///
/// This is the main entry point for height calculation with actual section content.
///
/// # Arguments
///
/// * `screen_height` - Terminal height in rows
/// * `sections` - Rendered sections with content
/// * `available_width` - Available width for statusline
/// * `config` - Height configuration
///
/// # Returns
///
/// A [`HeightResult`] with the calculated height and overflow information.
#[must_use]
pub fn calculate_height_from_sections(
    screen_height: u16,
    sections: &[Section],
    available_width: u16,
    config: &HeightConfig,
) -> HeightResult {
    let metrics = ContentMetrics::from_sections(sections);
    calculate_height_with_metrics(screen_height, &metrics, available_width, config)
}

/// Calculate statusline height using pre-computed content metrics.
///
/// # Arguments
///
/// * `screen_height` - Terminal height in rows
/// * `metrics` - Pre-computed content metrics
/// * `available_width` - Available width for statusline
/// * `config` - Height configuration
///
/// # Returns
///
/// A [`HeightResult`] with the calculated height and overflow information.
#[must_use]
pub fn calculate_height_with_metrics(
    screen_height: u16,
    metrics: &ContentMetrics,
    available_width: u16,
    config: &HeightConfig,
) -> HeightResult {
    // 1. Calculate allowed height from screen thresholds
    let mut allowed_height = config.min_height;
    for threshold in &config.screen_thresholds {
        if screen_height >= threshold.min_screen_height {
            allowed_height = allowed_height.saturating_add(threshold.additional_rows);
        }
    }

    // 2. Cap at max_height if configured
    if let Some(max) = config.max_height {
        allowed_height = allowed_height.min(max);
    }

    // 3. Check for content overflow
    let has_overflow = metrics.overflows(available_width);

    // 4. Calculate required rows based on overflow strategy
    let required_rows = if has_overflow {
        match config.overflow_strategy {
            OverflowStrategy::Wrap => metrics.rows_for_wrap(available_width),
            OverflowStrategy::Redistribute => metrics.rows_for_redistribute(available_width),
            OverflowStrategy::Truncate => config.min_height, // Truncate doesn't add rows
        }
    } else {
        config.min_height
    };

    // 5. Final height = max(min_height, required_rows), capped at allowed_height
    let height = required_rows.max(config.min_height).min(allowed_height);

    HeightResult::new(height, has_overflow, config.overflow_strategy)
}

#[cfg(test)]
#[path = "height_tests.rs"]
mod tests;
