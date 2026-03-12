//! Line number annotation source and presenter.
//!
//! Implements line numbers using the generic annotation system,
//! supporting absolute, relative, and hybrid modes.

use std::sync::Arc;

use {
    reovim_driver_display::{
        Annotation, AnnotationContext, AnnotationKind, AnnotationPayload, AnnotationPresenter,
        AnnotationSource, AnnotationTarget, ColumnWidth, KindPattern, LineNumberMode,
        PresentedOutput, PresenterContext, Style,
    },
    reovim_kernel::api::v1::BufferId,
};

/// Static annotation kind for line numbers.
static LINE_NUMBER_KIND: std::sync::LazyLock<AnnotationKind> =
    std::sync::LazyLock::new(|| AnnotationKind::new("line_number"));

/// Line number annotation source.
///
/// Generates line number annotations for visible lines based on the
/// configured display mode (absolute, relative, hybrid).
#[derive(Debug, Clone)]
pub struct LineNumberSource {
    /// Display mode for line numbers.
    mode: LineNumberMode,
}

impl LineNumberSource {
    /// Create a new line number source with the given mode.
    #[must_use]
    pub const fn new(mode: LineNumberMode) -> Self {
        Self { mode }
    }

    /// Create with absolute line numbers.
    #[must_use]
    pub const fn absolute() -> Self {
        Self::new(LineNumberMode::Absolute)
    }

    /// Create with relative line numbers.
    #[must_use]
    pub const fn relative() -> Self {
        Self::new(LineNumberMode::Relative)
    }

    /// Create with hybrid line numbers.
    #[must_use]
    pub const fn hybrid() -> Self {
        Self::new(LineNumberMode::Hybrid)
    }

    /// Get the current mode.
    #[must_use]
    pub const fn mode(&self) -> LineNumberMode {
        self.mode
    }

    /// Set the mode.
    pub const fn set_mode(&mut self, mode: LineNumberMode) {
        self.mode = mode;
    }
}

impl AnnotationSource for LineNumberSource {
    fn id(&self) -> &'static str {
        "builtin.line_number"
    }

    fn provides(&self) -> Vec<AnnotationKind> {
        vec![LINE_NUMBER_KIND.clone()]
    }

    #[cfg_attr(coverage_nightly, coverage(off))]
    fn annotations(
        &self,
        _buffer_id: BufferId,
        range: std::ops::Range<usize>,
        context: &AnnotationContext,
    ) -> Vec<Annotation> {
        // Use context mode if provided, otherwise fall back to stored mode
        let mode = context.line_number_mode().unwrap_or(self.mode);

        if mode == LineNumberMode::None {
            return vec![];
        }

        range
            .map(|line| {
                let number = match mode {
                    LineNumberMode::None => unreachable!(),
                    LineNumberMode::Absolute => line + 1,
                    LineNumberMode::Hybrid if line == context.cursor_line => line + 1,
                    LineNumberMode::Relative | LineNumberMode::Hybrid => {
                        line.abs_diff(context.cursor_line)
                    }
                };

                Annotation {
                    kind: LINE_NUMBER_KIND.clone(),
                    target: AnnotationTarget::Line(line),
                    priority: 0,
                    payload: AnnotationPayload::Number(number),
                }
            })
            .collect()
    }

    fn has_annotations(&self, _buffer_id: BufferId) -> bool {
        self.mode != LineNumberMode::None
    }
}

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
    pub fn from_theme(manager: &reovim_driver_display::ThemeManager) -> Self {
        use reovim_driver_display::style::groups;
        Self {
            line_number_style: manager.get_style(groups::LINE_NUMBER),
            cursor_line_style: manager.get_style(groups::LINE_NUMBER_ACTIVE),
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

impl LineNumberPresenter {
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

/// Create a shared line number source.
#[must_use]
pub fn create_line_number_source(mode: LineNumberMode) -> Arc<LineNumberSource> {
    Arc::new(LineNumberSource::new(mode))
}

/// Create a shared line number presenter.
#[must_use]
pub fn create_line_number_presenter() -> Arc<LineNumberPresenter> {
    Arc::new(LineNumberPresenter::new())
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
