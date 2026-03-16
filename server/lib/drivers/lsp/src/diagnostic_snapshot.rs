//! Pre-resolved diagnostic snapshot for bridge serialization.
//!
//! Contains diagnostic data types shared across module boundaries.
//! These types are populated by the LSP module's `DiagnosticBridge`
//! and consumed by modules like `diagnostics-panel`.

use reovim_driver_session::SessionExtension;

/// Pre-resolved diagnostic snapshot stored in shared `ExtensionMap`.
///
/// Contains diagnostics grouped by buffer, with URIs already resolved
/// to buffer IDs. Populated by `DiagnosticBridge::tick()`.
#[derive(Debug, Default)]
pub struct DiagnosticSnapshot {
    /// Diagnostics grouped by buffer.
    pub entries: Vec<BufferDiagnosticEntry>,
}

impl SessionExtension for DiagnosticSnapshot {
    fn create() -> Self {
        Self::default()
    }
}

/// Diagnostics for a single buffer.
#[derive(Debug, Clone)]
pub struct BufferDiagnosticEntry {
    /// Buffer ID (resolved from URI).
    pub buffer_id: u64,
    /// Individual diagnostics in this buffer.
    pub diagnostics: Vec<DiagnosticItem>,
}

/// A single diagnostic item with resolved positions.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DiagnosticItem {
    /// Start line (0-indexed).
    pub start_line: u32,
    /// Start column (0-indexed).
    pub start_col: u32,
    /// End line (0-indexed).
    pub end_line: u32,
    /// End column (0-indexed).
    pub end_col: u32,
    /// Severity level.
    pub severity: DiagnosticSeverity,
    /// Diagnostic message.
    pub message: String,
    /// Source of the diagnostic (e.g., "rust-analyzer").
    pub source: Option<String>,
}

/// Diagnostic severity levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DiagnosticSeverity {
    /// Compile error or critical issue.
    Error,
    /// Warning that may indicate a problem.
    Warning,
    /// Informational message.
    Information,
    /// Hint or suggestion.
    Hint,
}

#[cfg(test)]
#[path = "diagnostic_snapshot_tests.rs"]
mod tests;
