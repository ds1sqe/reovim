//! Line number annotation presenter.
//!
//! Renders line number annotations as right-aligned numbers with
//! appropriate styling for cursor line vs other lines.

use crate::highlight::Style;

use super::{
    Annotation, AnnotationPresenter, AnnotationTarget, ColumnWidth, KindPattern, PresentedOutput,
    PresenterContext,
};

/// Line number presenter.
///
/// Renders line number annotations as right-aligned numbers with
/// appropriate styling for cursor line vs other lines.
#[derive(Debug, Clone, Default)]
pub struct LineNumberPresenter {
    /// Style for regular line numbers.
    line_number_style: Style,
    /// Style for the cursor line number.
    cursor_line_style: Style,
}

impl LineNumberPresenter {
    /// Create a new presenter with default styles.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Create a presenter with custom styles.
    #[must_use]
    pub const fn with_styles(line_number_style: Style, cursor_line_style: Style) -> Self {
        Self {
            line_number_style,
            cursor_line_style,
        }
    }

    /// Set styles from a theme manager.
    #[must_use]
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn from_theme(manager: &crate::style::ThemeManager) -> Self {
        use crate::style::groups;
        Self {
            line_number_style: manager.get_style(groups::LINE_NUMBER),
            cursor_line_style: manager.get_style(groups::LINE_NUMBER_ACTIVE),
        }
    }

    /// Calculate digits needed to display a number.
    fn digits_needed(n: usize) -> usize {
        if n == 0 {
            1
        } else {
            #[allow(
                clippy::cast_precision_loss,
                clippy::cast_sign_loss,
                clippy::cast_possible_truncation
            )]
            let d = (n as f64).log10().floor() as usize + 1;
            d
        }
    }
}

impl AnnotationPresenter for LineNumberPresenter {
    fn id(&self) -> &'static str {
        "builtin.line_number"
    }

    fn handles(&self) -> KindPattern {
        KindPattern::exact("line_number")
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn present(&self, annotation: &Annotation, ctx: &PresenterContext) -> PresentedOutput {
        let Some(num) = annotation.payload.as_number() else {
            return PresentedOutput::Hidden;
        };

        let is_cursor_line = match annotation.target {
            AnnotationTarget::Line(l) => l == ctx.cursor_line,
            _ => false,
        };

        let style = if is_cursor_line {
            &self.cursor_line_style
        } else {
            &self.line_number_style
        };

        // Format with dynamic width based on total lines
        let digits = Self::digits_needed(ctx.total_lines);
        let text = format!("{num:>digits$} ");

        PresentedOutput::text(&text, style)
    }

    fn column_width(&self, ctx: &PresenterContext) -> ColumnWidth {
        // digits + 1 space padding
        let digits = Self::digits_needed(ctx.total_lines);
        #[allow(clippy::cast_possible_truncation)]
        let width = (digits + 1) as u16;
        ColumnWidth::fixed(width)
    }
}

#[cfg(test)]
#[path = "line_number_presenter_tests.rs"]
mod tests;
