//! Panel item model and conversion from diagnostic snapshot.

use reovim_driver_lsp::{DiagnosticSeverity, DiagnosticSnapshot};

/// A single item displayed in the diagnostics panel.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelItem {
    /// File path for the diagnostic.
    pub file_path: String,
    /// Line number (0-indexed).
    pub line: u32,
    /// Column number (0-indexed).
    pub col: u32,
    /// Diagnostic severity.
    pub severity: DiagnosticSeverity,
    /// Diagnostic message.
    pub message: String,
    /// Source of the diagnostic (e.g., "rust-analyzer").
    pub source: Option<String>,
    /// Buffer ID for jump navigation.
    pub buffer_id: Option<u64>,
}

/// Panel display mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PanelMode {
    /// LSP diagnostics.
    Diagnostics,
    /// Quickfix list entries (stub).
    Quickfix,
    /// LSP references (stub).
    References,
    /// TODO/FIXME comments (stub).
    Todo,
}

impl PanelMode {
    /// Display title for this mode.
    #[must_use]
    pub const fn title(self) -> &'static str {
        match self {
            Self::Diagnostics => "Diagnostics",
            Self::Quickfix => "Quickfix",
            Self::References => "References",
            Self::Todo => "TODO",
        }
    }

    /// Parse a mode from a string argument.
    #[must_use]
    pub fn from_arg(arg: &str) -> Option<Self> {
        match arg {
            "diagnostics" | "diag" => Some(Self::Diagnostics),
            "quickfix" | "qf" => Some(Self::Quickfix),
            "references" | "refs" => Some(Self::References),
            "todo" => Some(Self::Todo),
            _ => None,
        }
    }
}

/// Sort order for panel items.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SortOrder {
    /// Sort by severity (errors first).
    BySeverity,
    /// Sort by file path.
    ByFile,
    /// Sort by line number.
    ByLine,
}

impl SortOrder {
    /// Cycle to the next sort order.
    #[must_use]
    pub const fn next(self) -> Self {
        match self {
            Self::BySeverity => Self::ByFile,
            Self::ByFile => Self::ByLine,
            Self::ByLine => Self::BySeverity,
        }
    }
}

/// Severity filter configuration.
#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct SeverityFilter {
    /// Show errors.
    pub errors: bool,
    /// Show warnings.
    pub warnings: bool,
    /// Show informational diagnostics.
    pub information: bool,
    /// Show hints.
    pub hints: bool,
}

impl SeverityFilter {
    /// Show all severities.
    #[must_use]
    pub const fn all() -> Self {
        Self {
            errors: true,
            warnings: true,
            information: true,
            hints: true,
        }
    }

    /// Show only errors.
    #[must_use]
    pub const fn errors_only() -> Self {
        Self {
            errors: true,
            warnings: false,
            information: false,
            hints: false,
        }
    }

    /// Show only warnings.
    #[must_use]
    pub const fn warnings_only() -> Self {
        Self {
            errors: false,
            warnings: true,
            information: false,
            hints: false,
        }
    }

    /// Check whether a severity passes the filter.
    #[must_use]
    pub const fn allows(&self, severity: DiagnosticSeverity) -> bool {
        match severity {
            DiagnosticSeverity::Error => self.errors,
            DiagnosticSeverity::Warning => self.warnings,
            DiagnosticSeverity::Information => self.information,
            DiagnosticSeverity::Hint => self.hints,
        }
    }
}

impl Default for SeverityFilter {
    fn default() -> Self {
        Self::all()
    }
}

/// Severity ordering value (lower = more severe).
const fn severity_rank(s: DiagnosticSeverity) -> u8 {
    match s {
        DiagnosticSeverity::Error => 0,
        DiagnosticSeverity::Warning => 1,
        DiagnosticSeverity::Information => 2,
        DiagnosticSeverity::Hint => 3,
    }
}

/// Sort a list of panel items by the given order.
pub fn sort_items(items: &mut [PanelItem], order: SortOrder) {
    match order {
        SortOrder::BySeverity => {
            items.sort_by(|a, b| {
                severity_rank(a.severity)
                    .cmp(&severity_rank(b.severity))
                    .then(a.file_path.cmp(&b.file_path))
                    .then(a.line.cmp(&b.line))
            });
        }
        SortOrder::ByFile => {
            items.sort_by(|a, b| {
                a.file_path
                    .cmp(&b.file_path)
                    .then(a.line.cmp(&b.line))
                    .then(severity_rank(a.severity).cmp(&severity_rank(b.severity)))
            });
        }
        SortOrder::ByLine => {
            items.sort_by(|a, b| {
                a.line
                    .cmp(&b.line)
                    .then(a.file_path.cmp(&b.file_path))
                    .then(severity_rank(a.severity).cmp(&severity_rank(b.severity)))
            });
        }
    }
}

/// Convert a `DiagnosticSnapshot` into a flat list of `PanelItem`.
///
/// Uses `path_resolver` to map buffer IDs to file paths. Items for
/// buffers with no resolved path are skipped.
pub fn items_from_snapshot<F>(snapshot: &DiagnosticSnapshot, path_resolver: F) -> Vec<PanelItem>
where
    F: Fn(u64) -> Option<String>,
{
    let mut items = Vec::new();
    for entry in &snapshot.entries {
        let Some(path) = path_resolver(entry.buffer_id) else {
            continue;
        };
        for diag in &entry.diagnostics {
            items.push(PanelItem {
                file_path: path.clone(),
                line: diag.start_line,
                col: diag.start_col,
                severity: diag.severity,
                message: diag.message.clone(),
                source: diag.source.clone(),
                buffer_id: Some(entry.buffer_id),
            });
        }
    }
    items
}

#[cfg(test)]
#[path = "items_tests.rs"]
mod tests;
