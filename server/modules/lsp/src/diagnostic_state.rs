//! Pre-resolved diagnostic snapshot for bridge serialization.
//!
//! The lsp module populates this state from `DiagnosticCache` data,
//! resolving URI strings to buffer IDs. The `DiagnosticBridge` reads
//! this snapshot from the shared `ExtensionMap` for serialization.

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
mod tests {
    use super::*;

    #[test]
    fn snapshot_default() {
        let snap = DiagnosticSnapshot::default();
        assert!(snap.entries.is_empty());
    }

    #[test]
    fn snapshot_create() {
        let snap = DiagnosticSnapshot::create();
        assert!(snap.entries.is_empty());
    }

    #[test]
    fn snapshot_debug() {
        let snap = DiagnosticSnapshot::default();
        let debug = format!("{snap:?}");
        assert!(debug.contains("DiagnosticSnapshot"));
    }

    #[test]
    fn entry_clone_debug() {
        let entry = BufferDiagnosticEntry {
            buffer_id: 1,
            diagnostics: vec![DiagnosticItem {
                start_line: 0,
                start_col: 0,
                end_line: 0,
                end_col: 5,
                severity: DiagnosticSeverity::Error,
                message: "test error".to_owned(),
                source: Some("test".to_owned()),
            }],
        };
        let debug = format!("{entry:?}");
        assert!(debug.contains("BufferDiagnosticEntry"));

        #[allow(clippy::redundant_clone)]
        let cloned = entry.clone();
        assert_eq!(cloned.buffer_id, 1);
        assert_eq!(cloned.diagnostics.len(), 1);
    }

    #[test]
    fn item_clone_debug_eq() {
        let item = DiagnosticItem {
            start_line: 5,
            start_col: 0,
            end_line: 5,
            end_col: 10,
            severity: DiagnosticSeverity::Warning,
            message: "unused variable".to_owned(),
            source: None,
        };
        let debug = format!("{item:?}");
        assert!(debug.contains("DiagnosticItem"));

        #[allow(clippy::redundant_clone)]
        let cloned = item.clone();
        assert_eq!(item, cloned);
    }

    #[test]
    fn item_not_equal() {
        let a = DiagnosticItem {
            start_line: 0,
            start_col: 0,
            end_line: 0,
            end_col: 5,
            severity: DiagnosticSeverity::Error,
            message: "a".to_owned(),
            source: None,
        };
        let b = DiagnosticItem {
            start_line: 0,
            start_col: 0,
            end_line: 0,
            end_col: 5,
            severity: DiagnosticSeverity::Error,
            message: "b".to_owned(),
            source: None,
        };
        assert_ne!(a, b);
    }

    #[test]
    fn severity_all_variants() {
        let variants = [
            DiagnosticSeverity::Error,
            DiagnosticSeverity::Warning,
            DiagnosticSeverity::Information,
            DiagnosticSeverity::Hint,
        ];
        for v in &variants {
            let debug = format!("{v:?}");
            assert!(!debug.is_empty());
        }
    }

    #[test]
    fn severity_clone_copy_eq() {
        let a = DiagnosticSeverity::Error;
        let b = a;
        #[allow(clippy::clone_on_copy)]
        let c = a.clone();
        assert_eq!(a, b);
        assert_eq!(a, c);
        assert_ne!(DiagnosticSeverity::Error, DiagnosticSeverity::Warning);
        assert_ne!(DiagnosticSeverity::Information, DiagnosticSeverity::Hint);
    }

    #[test]
    fn item_with_source() {
        let item = DiagnosticItem {
            start_line: 0,
            start_col: 0,
            end_line: 0,
            end_col: 1,
            severity: DiagnosticSeverity::Hint,
            message: "hint".to_owned(),
            source: Some("clippy".to_owned()),
        };
        assert_eq!(item.source.as_deref(), Some("clippy"));
    }

    #[test]
    fn item_without_source() {
        let item = DiagnosticItem {
            start_line: 0,
            start_col: 0,
            end_line: 0,
            end_col: 1,
            severity: DiagnosticSeverity::Information,
            message: "info".to_owned(),
            source: None,
        };
        assert!(item.source.is_none());
    }

    #[test]
    fn snapshot_with_entries() {
        let snap = DiagnosticSnapshot {
            entries: vec![BufferDiagnosticEntry {
                buffer_id: 42,
                diagnostics: vec![],
            }],
        };
        assert_eq!(snap.entries.len(), 1);
        assert_eq!(snap.entries[0].buffer_id, 42);
    }
}
