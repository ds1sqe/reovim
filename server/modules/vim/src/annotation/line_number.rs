//! Line number annotation source.
//!
//! Implements line numbers using the generic annotation system,
//! supporting absolute, relative, and hybrid modes.

use std::sync::Arc;

use {
    reovim_kernel::api::v1::BufferId,
    reovim_driver_annotation::{
        Annotation, AnnotationContext, AnnotationKind, AnnotationPayload, AnnotationSource,
        AnnotationTarget, LineNumberMode,
    },
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

/// Create a shared line number source.
#[must_use]
pub fn create_line_number_source(mode: LineNumberMode) -> Arc<LineNumberSource> {
    Arc::new(LineNumberSource::new(mode))
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
