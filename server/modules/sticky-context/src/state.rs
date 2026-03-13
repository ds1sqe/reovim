//! Sticky context session state.
//!
//! Per-client state for determining which scope headers to display.

use {reovim_driver_session::SessionExtension, reovim_module_context::ContextSessionState};

/// Options for sticky context display.
#[derive(Debug, Clone)]
pub struct StickyContextOptions {
    /// Enable sticky scope headers.
    pub enabled: bool,
    /// Maximum number of header rows (1-5).
    pub max_count: usize,
    /// Show separator line below headers.
    pub separator: bool,
}

impl Default for StickyContextOptions {
    fn default() -> Self {
        Self {
            enabled: true,
            max_count: 3,
            separator: true,
        }
    }
}

/// A single header row for display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeaderRow {
    /// Source line number (0-based) in the buffer.
    pub line: u32,
    /// Display text (e.g., "fn main", "impl Foo").
    pub text: String,
    /// Scope kind label (e.g., "fn", "class", "mod").
    pub kind: String,
}

impl HeaderRow {
    /// Create a new header row.
    #[must_use]
    pub fn new(line: u32, text: impl Into<String>, kind: impl Into<String>) -> Self {
        Self {
            line,
            text: text.into(),
            kind: kind.into(),
        }
    }
}

/// Per-client sticky context state.
///
/// Reads from `ContextSessionState` and produces header rows.
#[derive(Debug, Default)]
pub struct StickyContextState {
    /// Display options.
    pub options: StickyContextOptions,
}

impl SessionExtension for StickyContextState {
    fn create() -> Self {
        Self::default()
    }
}

impl StickyContextState {
    /// Compute header rows from the context hierarchy.
    ///
    /// Returns up to `max_count` outermost scopes whose start line is
    /// above the viewport top (i.e., the scope started before what's
    /// currently visible).
    #[must_use]
    pub fn header_rows(
        &self,
        context_state: Option<&ContextSessionState>,
        viewport_top: u32,
    ) -> Vec<HeaderRow> {
        if !self.options.enabled {
            return Vec::new();
        }

        let Some(ctx) = context_state else {
            return Vec::new();
        };

        let Some(hierarchy) = ctx.hierarchy() else {
            return Vec::new();
        };

        if hierarchy.is_empty() {
            return Vec::new();
        }

        // Filter to scopes that start above the viewport (scrolled past).
        // These are the scopes the user can no longer see the header of.
        let mut rows: Vec<HeaderRow> = hierarchy
            .items
            .iter()
            .filter(|scope| scope.start_line < viewport_top)
            .map(|scope| HeaderRow::new(scope.start_line, &scope.display_text, scope.kind.as_str()))
            .collect();

        // Take only the last max_count (innermost scopes are most relevant)
        if rows.len() > self.options.max_count {
            let skip = rows.len() - self.options.max_count;
            rows = rows.split_off(skip);
        }

        rows
    }
}

#[cfg(test)]
#[path = "state_tests.rs"]
mod tests;
