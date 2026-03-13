//! Gutter configuration for the annotation system.
//!
//! Configuration determines which annotation kinds are displayed,
//! their visibility modes, and column ordering.
//!
//! # Architecture
//!
//! Configuration is part of the mechanism layer - it defines the
//! structure of gutter layout, not the actual values. Policy modules
//! and user configuration provide the actual settings.
//!
//! # Example
//!
//! ```
//! use reovim_driver_display::annotation::{
//!     ColumnConfig, GutterConfig, KindPattern, VisibilityMode,
//! };
//!
//! // Create a simple gutter with line numbers only
//! let config = GutterConfig::default_line_numbers();
//!
//! // Create a full gutter with multiple columns
//! let config = GutterConfig::new(vec![
//!     ColumnConfig::new(KindPattern::exact("sign"))
//!         .visibility(VisibilityMode::Auto)
//!         .width(2),
//!     ColumnConfig::new(KindPattern::prefix("diagnostic"))
//!         .visibility(VisibilityMode::Auto),
//!     ColumnConfig::new(KindPattern::exact("line_number"))
//!         .visibility(VisibilityMode::Always),
//! ]);
//! ```

use super::presenter::KindPattern;

/// Visibility mode for a gutter column.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum VisibilityMode {
    /// Show when annotations exist for this pattern.
    #[default]
    Auto,
    /// Always show this column.
    Always,
    /// Never show this column.
    Never,
}

impl VisibilityMode {
    /// Check if this mode means the column should be visible.
    ///
    /// For `Auto` mode, the caller must check if annotations exist.
    #[must_use]
    pub const fn should_show(&self, has_annotations: bool) -> bool {
        match *self {
            Self::Auto => has_annotations,
            Self::Always => true,
            Self::Never => false,
        }
    }
}

/// Configuration for a single gutter column.
///
/// Each column displays annotations matching its pattern.
#[derive(Debug, Clone)]
pub struct ColumnConfig {
    /// Pattern for matching annotation kinds.
    pub pattern: KindPattern,
    /// Visibility mode.
    pub visibility: VisibilityMode,
    /// Fixed width override (None = dynamic).
    pub width: Option<u16>,
}

impl ColumnConfig {
    /// Create a new column configuration.
    #[must_use]
    pub fn new(pattern: KindPattern) -> Self {
        Self {
            pattern,
            visibility: VisibilityMode::default(),
            width: None,
        }
    }

    /// Set the visibility mode.
    #[must_use]
    pub const fn visibility(mut self, mode: VisibilityMode) -> Self {
        self.visibility = mode;
        self
    }

    /// Set a fixed width.
    #[must_use]
    pub const fn width(mut self, width: u16) -> Self {
        self.width = Some(width);
        self
    }

    /// Check if this column matches the given kind.
    #[must_use]
    pub fn matches(&self, kind: &super::AnnotationKind) -> bool {
        self.pattern.matches(kind)
    }
}

/// Configuration for the entire gutter.
///
/// Defines column order and behavior.
#[derive(Debug, Clone, Default)]
pub struct GutterConfig {
    /// Columns in display order (left to right).
    pub columns: Vec<ColumnConfig>,
    /// Show separator after gutter.
    pub show_separator: bool,
}

impl GutterConfig {
    /// Create a new gutter configuration.
    ///
    /// Note: Cannot be const because `Vec` field initialization is not const.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn new(columns: Vec<ColumnConfig>) -> Self {
        Self {
            columns,
            show_separator: true,
        }
    }

    /// Create a minimal configuration with only line numbers.
    #[must_use]
    pub fn default_line_numbers() -> Self {
        Self {
            columns: vec![
                ColumnConfig::new(KindPattern::exact("line_number"))
                    .visibility(VisibilityMode::Always),
            ],
            show_separator: true,
        }
    }

    /// Create a full configuration with signs, diagnostics, and line numbers.
    #[must_use]
    pub fn full() -> Self {
        Self {
            columns: vec![
                ColumnConfig::new(KindPattern::prefix("sign"))
                    .visibility(VisibilityMode::Auto)
                    .width(2),
                ColumnConfig::new(KindPattern::prefix("git"))
                    .visibility(VisibilityMode::Auto)
                    .width(1),
                ColumnConfig::new(KindPattern::prefix("diagnostic"))
                    .visibility(VisibilityMode::Auto)
                    .width(1),
                ColumnConfig::new(KindPattern::exact("fold"))
                    .visibility(VisibilityMode::Auto)
                    .width(1),
                ColumnConfig::new(KindPattern::exact("line_number"))
                    .visibility(VisibilityMode::Always),
            ],
            show_separator: true,
        }
    }

    /// Create a configuration with no gutter.
    #[must_use]
    pub const fn none() -> Self {
        Self {
            columns: Vec::new(),
            show_separator: false,
        }
    }

    /// Set whether to show the separator.
    #[must_use]
    pub const fn with_separator(mut self, show: bool) -> Self {
        self.show_separator = show;
        self
    }

    /// Add a column to the configuration.
    #[must_use]
    pub fn add_column(mut self, column: ColumnConfig) -> Self {
        self.columns.push(column);
        self
    }

    /// Get the number of columns.
    ///
    /// Note: Cannot be const because `Vec::len()` is not const.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn column_count(&self) -> usize {
        self.columns.len()
    }

    /// Check if the gutter is empty (no columns).
    ///
    /// Note: Cannot be const because `Vec::is_empty()` is not const.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn is_empty(&self) -> bool {
        self.columns.is_empty()
    }

    /// Find the column that matches the given kind.
    #[must_use]
    pub fn column_for_kind(&self, kind: &super::AnnotationKind) -> Option<(usize, &ColumnConfig)> {
        self.columns
            .iter()
            .enumerate()
            .find(|(_, col)| col.matches(kind))
    }
}

#[cfg(test)]
#[path = "config_tests.rs"]
mod tests;
