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
mod tests {
    use {super::*, crate::Style};

    fn make_section(id: SectionId, text: &str) -> Section {
        Section::new(id, text, Style::default())
    }

    #[test]
    fn test_content_metrics_from_sections() {
        let sections = vec![
            make_section(SectionId::A, " NORMAL "),      // 8
            make_section(SectionId::B, ""),              // 0
            make_section(SectionId::C, " main.rs [+] "), // 13
            make_section(SectionId::X, ""),              // 0
            make_section(SectionId::Y, " rust "),        // 6
            make_section(SectionId::Z, " 42:15 Top "),   // 11
        ];

        let metrics = ContentMetrics::from_sections(&sections);

        assert_eq!(metrics.left_width, 8 + 13); // A(8) + B(0) + C(13) = 21
        assert_eq!(metrics.right_width, 6 + 11); // X(0) + Y(6) + Z(11) = 17
        assert_eq!(metrics.total_width, 38);
        assert_eq!(metrics.section_widths[0], 8); // A
        assert_eq!(metrics.section_widths[2], 13); // C
    }

    #[test]
    fn test_content_metrics_overflows() {
        let metrics = ContentMetrics {
            total_width: 100,
            left_width: 50,
            right_width: 50,
            section_widths: [10, 20, 20, 20, 20, 10],
        };

        assert!(metrics.overflows(80));
        assert!(metrics.overflows(99));
        assert!(!metrics.overflows(100));
        assert!(!metrics.overflows(120));
    }

    #[test]
    fn test_content_metrics_gap_width() {
        let metrics = ContentMetrics {
            total_width: 60,
            left_width: 30,
            right_width: 30,
            section_widths: [10, 10, 10, 10, 10, 10],
        };

        assert_eq!(metrics.gap_width(100), 40); // 100 - 60 = 40
        assert_eq!(metrics.gap_width(60), 0); // exactly fits
        assert_eq!(metrics.gap_width(50), 0); // overflows
    }

    #[test]
    fn test_content_metrics_rows_for_wrap() {
        let metrics = ContentMetrics {
            total_width: 160,
            ..Default::default()
        };

        assert_eq!(metrics.rows_for_wrap(80), 2); // 160 / 80 = 2
        assert_eq!(metrics.rows_for_wrap(100), 2); // 160 / 100 = 1.6 → 2
        assert_eq!(metrics.rows_for_wrap(160), 1); // exact fit
        assert_eq!(metrics.rows_for_wrap(200), 1); // fits with room
    }

    #[test]
    fn test_content_metrics_rows_for_redistribute() {
        // All fits in one row
        let metrics = ContentMetrics {
            total_width: 60,
            left_width: 30,
            right_width: 30,
            section_widths: [10, 10, 10, 10, 10, 10],
        };
        assert_eq!(metrics.rows_for_redistribute(80), 1);

        // Needs 2 rows (left and right can fit separately)
        let metrics = ContentMetrics {
            total_width: 120,
            left_width: 60,
            right_width: 60,
            section_widths: [20, 20, 20, 20, 20, 20],
        };
        assert_eq!(metrics.rows_for_redistribute(80), 2);

        // Needs 3 rows (A+B on row1, C on row2, X+Y+Z on row3)
        let metrics = ContentMetrics {
            total_width: 180,
            left_width: 100, // A+B=40, C=60
            right_width: 80,
            section_widths: [20, 20, 60, 30, 30, 20],
        };
        assert_eq!(metrics.rows_for_redistribute(80), 3);
    }

    #[test]
    fn test_calculate_height_from_sections_no_overflow() {
        let sections = vec![
            make_section(SectionId::A, " NORMAL "),
            make_section(SectionId::Z, " 1:1 "),
        ];
        let config = HeightConfig::default();

        let result = calculate_height_from_sections(30, &sections, 80, &config);

        assert_eq!(result.height, 1);
        assert!(!result.has_overflow);
    }

    #[test]
    fn test_calculate_height_from_sections_with_overflow() {
        // Create sections that total more than 80 width
        let sections = vec![
            make_section(SectionId::A, " NORMAL "),
            make_section(SectionId::C, " very-long-filename-that-causes-overflow.rs [+] "),
            make_section(SectionId::Z, " 42:15 25% "),
        ];
        let config = HeightConfig::default().with_overflow_strategy(OverflowStrategy::Redistribute);

        let result = calculate_height_from_sections(50, &sections, 60, &config);

        assert!(result.has_overflow);
        assert!(result.height >= 1);
    }

    #[test]
    fn test_height_config_default() {
        let config = HeightConfig::default();
        assert_eq!(config.min_height, 1);
        assert_eq!(config.max_height, Some(3));
        assert_eq!(config.overflow_strategy, OverflowStrategy::Truncate);
        assert_eq!(config.screen_thresholds.len(), 2);
    }

    #[test]
    fn test_height_config_fixed() {
        let config = HeightConfig::fixed(2);
        assert_eq!(config.min_height, 2);
        assert_eq!(config.max_height, Some(2));
    }

    #[test]
    fn test_height_config_single_row() {
        let config = HeightConfig::single_row();
        assert_eq!(config.min_height, 1);
        assert_eq!(config.max_height, Some(1));
    }

    #[test]
    fn test_overflow_strategy_may_use_multiple_rows() {
        assert!(!OverflowStrategy::Truncate.may_use_multiple_rows());
        assert!(OverflowStrategy::Wrap.may_use_multiple_rows());
        assert!(OverflowStrategy::Redistribute.may_use_multiple_rows());
    }

    #[test]
    fn test_calculate_height_no_overflow() {
        let config = HeightConfig::default();
        let result = calculate_height(30, 50, 80, &config);

        assert_eq!(result.height, 1);
        assert!(!result.has_overflow);
    }

    #[test]
    fn test_calculate_height_with_overflow_truncate() {
        let config = HeightConfig::default(); // Truncate by default
        let result = calculate_height(30, 100, 80, &config);

        // Truncate doesn't add rows
        assert_eq!(result.height, 1);
        assert!(result.has_overflow);
        assert_eq!(result.strategy, OverflowStrategy::Truncate);
    }

    #[test]
    fn test_calculate_height_with_overflow_wrap() {
        let config = HeightConfig::default().with_overflow_strategy(OverflowStrategy::Wrap);
        let result = calculate_height(50, 160, 80, &config);

        // 160 width needs 2 rows of 80
        assert_eq!(result.height, 2);
        assert!(result.has_overflow);
        assert_eq!(result.strategy, OverflowStrategy::Wrap);
    }

    #[test]
    fn test_calculate_height_respects_max() {
        let config = HeightConfig::default()
            .with_max_height(2)
            .with_overflow_strategy(OverflowStrategy::Wrap);
        let result = calculate_height(50, 300, 80, &config);

        // Would need 4 rows but capped at 2
        assert_eq!(result.height, 2);
        assert!(result.has_overflow);
    }

    #[test]
    fn test_calculate_height_screen_thresholds() {
        let config = HeightConfig::default().with_overflow_strategy(OverflowStrategy::Redistribute);

        // Small screen (30 rows) - no extra rows from thresholds
        let result = calculate_height(30, 200, 80, &config);
        assert_eq!(result.height, 1); // min_height, below first threshold

        // Medium screen (45 rows) - first threshold applies (+1 row allowed)
        let result = calculate_height(45, 200, 80, &config);
        assert_eq!(result.height, 2); // allowed up to 2, needs at least 3, capped

        // Large screen (70 rows) - both thresholds apply (+3 rows allowed)
        let result = calculate_height(70, 200, 80, &config);
        assert_eq!(result.height, 3); // allowed up to 4, needs 3, takes 3
    }

    #[test]
    fn test_height_result_single_row() {
        let result = HeightResult::single_row();
        assert_eq!(result.height, 1);
        assert!(!result.has_overflow);
        assert_eq!(result.strategy, OverflowStrategy::Truncate);
    }

    #[test]
    fn test_height_config_unlimited() {
        let config = HeightConfig::default().unlimited();
        assert!(config.max_height.is_none());
        assert_eq!(config.min_height, 1);
    }

    #[test]
    fn test_rows_for_wrap_zero_width() {
        let metrics = ContentMetrics {
            total_width: 100,
            ..Default::default()
        };
        assert_eq!(metrics.rows_for_wrap(0), 1);
    }

    #[test]
    fn test_rows_for_redistribute_zero_width() {
        let metrics = ContentMetrics {
            total_width: 100,
            left_width: 50,
            right_width: 50,
            section_widths: [10, 20, 20, 20, 20, 10],
        };
        assert_eq!(metrics.rows_for_redistribute(0), 1);
    }

    #[test]
    fn test_rows_for_redistribute_3_rows_exact() {
        // A+B fits, C fits, X+Y+Z fits -> exactly 3 rows
        let metrics = ContentMetrics {
            total_width: 150,
            left_width: 90,
            right_width: 60,
            section_widths: [20, 20, 50, 20, 20, 20],
        };
        // width = 55: left(90) > 55, right(60) > 55 -> skip 2-row
        // A+B = 40 <= 55, C = 50 <= 55, X+Y+Z = 60 > 55 -> 3-row condition false
        // Falls through to rows_for_wrap(55) -> 150/55 = 3
        assert_eq!(metrics.rows_for_redistribute(55), 3);

        // Now test where the 3-row condition IS true (line 296-298)
        let metrics2 = ContentMetrics {
            total_width: 140,
            left_width: 80,
            right_width: 60,
            section_widths: [20, 20, 40, 15, 15, 30],
        };
        // width = 65: left(80) > 65, right(60) <= 65 -> skip 2-row (left doesn't fit)
        // A+B = 40 <= 65, C = 40 <= 65, X+Y+Z = 60 <= 65 -> 3-row condition TRUE
        assert_eq!(metrics2.rows_for_redistribute(65), 3);
    }

    #[test]
    fn test_rows_for_redistribute_fallback_to_wrap() {
        // None of the groups fit individually -> fallback to wrap calculation
        let metrics = ContentMetrics {
            total_width: 300,
            left_width: 200,
            right_width: 100,
            section_widths: [80, 80, 40, 40, 40, 20],
        };
        // width = 30: left(200) > 30, right(100) > 30 -> skip 2-row
        // A+B = 160 > 30, C = 40 > 30, X+Y+Z = 100 > 30 -> 3-row false
        // Fallback to rows_for_wrap(30) -> 300/30 = 10
        assert_eq!(metrics.rows_for_redistribute(30), 10);
    }

    #[test]
    fn test_calculate_height_with_metrics_wrap_strategy() {
        let config = HeightConfig {
            min_height: 1,
            max_height: Some(5),
            screen_thresholds: vec![ScreenThreshold {
                min_screen_height: 20,
                additional_rows: 4,
            }],
            overflow_strategy: OverflowStrategy::Wrap,
        };

        let metrics = ContentMetrics {
            total_width: 200,
            left_width: 100,
            right_width: 100,
            section_widths: [30, 30, 40, 30, 30, 40],
        };

        // Screen height 30 >= threshold 20, so allowed_height = 1 + 4 = 5
        // Overflow: 200 > 80 -> true
        // Wrap strategy: rows_for_wrap(80) = 200/80 = 3
        // Final: max(1, 3).min(5) = 3
        let result = calculate_height_with_metrics(30, &metrics, 80, &config);
        assert_eq!(result.height, 3);
        assert!(result.has_overflow);
        assert_eq!(result.strategy, OverflowStrategy::Wrap);
    }

    #[test]
    fn test_calculate_height_with_metrics_redistribute_strategy() {
        let config = HeightConfig {
            min_height: 1,
            max_height: Some(5),
            screen_thresholds: vec![ScreenThreshold {
                min_screen_height: 20,
                additional_rows: 4,
            }],
            overflow_strategy: OverflowStrategy::Redistribute,
        };

        let metrics = ContentMetrics {
            total_width: 140,
            left_width: 80,
            right_width: 60,
            section_widths: [20, 20, 40, 15, 15, 30],
        };

        // Overflow: 140 > 65 -> true
        // Redistribute strategy: rows_for_redistribute(65)
        // left(80) > 65 -> skip 2-row
        // A+B=40 <= 65, C=40 <= 65, X+Y+Z=60 <= 65 -> 3 rows
        let result = calculate_height_with_metrics(30, &metrics, 65, &config);
        assert_eq!(result.height, 3);
        assert!(result.has_overflow);
        assert_eq!(result.strategy, OverflowStrategy::Redistribute);
    }

    #[test]
    fn test_calculate_height_with_metrics_truncate_strategy() {
        // Covers line 423: OverflowStrategy::Truncate arm in calculate_height_with_metrics
        let config = HeightConfig {
            min_height: 1,
            max_height: Some(3),
            screen_thresholds: vec![ScreenThreshold {
                min_screen_height: 20,
                additional_rows: 2,
            }],
            overflow_strategy: OverflowStrategy::Truncate,
        };

        let metrics = ContentMetrics {
            total_width: 200,
            left_width: 100,
            right_width: 100,
            section_widths: [30, 30, 40, 30, 30, 40],
        };

        // Overflow: 200 > 80 -> true
        // Truncate strategy: returns config.min_height = 1
        let result = calculate_height_with_metrics(30, &metrics, 80, &config);
        assert_eq!(result.height, 1);
        assert!(result.has_overflow);
        assert_eq!(result.strategy, OverflowStrategy::Truncate);
    }

    #[test]
    fn test_rows_for_redistribute_left_exceeds_right_fits() {
        // Line 286: left_fits && right_fits where left_fits is false (left exceeds width)
        let metrics = ContentMetrics {
            total_width: 100,
            left_width: 60,  // Exceeds width=50
            right_width: 40, // Fits in 50
            section_widths: [20, 20, 20, 15, 15, 10],
        };
        // total > width -> not 1 row; left_fits=false -> skip 2-row
        // A+B=40<=50, C=20<=50, X+Y+Z=40<=50 -> 3-row condition true
        assert_eq!(metrics.rows_for_redistribute(50), 3);
    }

    #[test]
    fn test_rows_for_redistribute_both_exceed() {
        // Line 296: multi-term AND where xyz_width > width
        let metrics = ContentMetrics {
            total_width: 200,
            left_width: 80,
            right_width: 120,
            section_widths: [20, 20, 40, 40, 40, 40],
        };
        // left(80)>50, right(120)>50 -> skip 2-row
        // A+B=40<=50, C=40<=50, X+Y+Z=120>50 -> condition false
        // Falls through to rows_for_wrap
        let rows = metrics.rows_for_redistribute(50);
        assert!(rows >= 3);
    }

    #[test]
    fn test_calculate_height_no_max() {
        // Line 333: max_height is None (no cap applied)
        let config = HeightConfig {
            min_height: 1,
            max_height: None,
            screen_thresholds: vec![],
            overflow_strategy: OverflowStrategy::Truncate,
        };

        let result = calculate_height(30, 100, 80, &config);
        assert_eq!(result.height, 1);
    }

    #[test]
    fn test_height_config_with_overflow_strategy() {
        let config = HeightConfig::default().with_overflow_strategy(OverflowStrategy::Redistribute);
        assert_eq!(config.overflow_strategy, OverflowStrategy::Redistribute);
    }

    #[test]
    fn test_calculate_height_with_metrics_unlimited_config() {
        let config = HeightConfig::default()
            .unlimited()
            .with_overflow_strategy(OverflowStrategy::Wrap);

        let metrics = ContentMetrics {
            total_width: 400,
            left_width: 200,
            right_width: 200,
            section_widths: [60, 60, 80, 60, 60, 80],
        };

        // No max_height cap, large screen triggers both thresholds
        // allowed_height = 1 + 1 + 2 = 4 (from default thresholds)
        // Wrap: 400/80 = 5, but capped at allowed_height = 4
        let result = calculate_height_with_metrics(70, &metrics, 80, &config);
        assert_eq!(result.height, 4);
        assert!(result.has_overflow);
    }

    #[test]
    fn test_rows_for_redistribute_right_exceeds_left_fits() {
        // Line 286: left_fits (true) && right_fits (false)
        let metrics = ContentMetrics {
            total_width: 100,
            left_width: 40,  // Fits in 50
            right_width: 60, // Exceeds width=50
            section_widths: [10, 10, 20, 20, 20, 20],
        };
        // total > width -> not 1 row
        // left_fits=true, right_fits=false -> skip 2-row
        // A+B=20<=50, C=20<=50, X+Y+Z=60>50 -> 3-row condition false
        // Falls through to rows_for_wrap
        let rows = metrics.rows_for_redistribute(50);
        assert!(rows >= 2);
    }

    #[test]
    fn test_rows_for_redistribute_ab_exceeds_but_c_xyz_fit() {
        // Line 296: ab_width > width (first term false), c and xyz fit
        let metrics = ContentMetrics {
            total_width: 120,
            left_width: 80,
            right_width: 40,
            section_widths: [30, 30, 20, 15, 15, 10],
        };
        // total(120) > 50 -> not 1 row
        // left(80) > 50 -> skip 2-row
        // A+B=60 > 50 (false), so the 3-term AND is false at first term
        // Falls through to rows_for_wrap
        let rows = metrics.rows_for_redistribute(50);
        assert!(rows >= 2);
    }

    #[test]
    fn test_rows_for_redistribute_ab_fits_c_exceeds() {
        // Line 296 MC/DC: ab_width <= width (true), c_width > width (false)
        // Middle sub-condition independently flips the 3-term AND
        let metrics = ContentMetrics {
            total_width: 150,
            left_width: 90,
            right_width: 60,
            section_widths: [10, 10, 70, 20, 20, 20],
        };
        // total(150) > 50 -> not 1 row
        // left(90) > 50 -> skip 2-row
        // A+B=20<=50 (true), C=70>50 (false) -> 3-row AND fails at second term
        let rows = metrics.rows_for_redistribute(50);
        assert!(rows >= 2);
    }
}
