//! Diagnostic counts for statusline display.

/// Diagnostic counts from LSP or other sources.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DiagnosticCounts {
    /// Error count.
    pub errors: usize,
    /// Warning count.
    pub warnings: usize,
    /// Info count.
    pub info: usize,
    /// Hint count.
    pub hints: usize,
}

impl DiagnosticCounts {
    /// Check if there are any diagnostics.
    #[cfg_attr(coverage_nightly, coverage(off))]
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.errors == 0 && self.warnings == 0 && self.info == 0 && self.hints == 0
    }

    /// Total count of all diagnostics.
    #[must_use]
    pub const fn total(&self) -> usize {
        self.errors + self.warnings + self.info + self.hints
    }
}
