use reovim_driver_text_lsp::{
    BufferDiagnosticEntry, DiagnosticItem, DiagnosticSeverity, DiagnosticSnapshot,
};

use super::*;

fn make_item(severity: DiagnosticSeverity, file: &str, line: u32) -> PanelItem {
    PanelItem {
        file_path: file.to_owned(),
        line,
        col: 0,
        severity,
        message: "test".to_owned(),
        source: None,
        buffer_id: None,
    }
}

// ============================================================================
// PanelMode
// ============================================================================

#[test]
fn panel_mode_titles() {
    assert_eq!(PanelMode::Diagnostics.title(), "Diagnostics");
    assert_eq!(PanelMode::Quickfix.title(), "Quickfix");
    assert_eq!(PanelMode::References.title(), "References");
    assert_eq!(PanelMode::Todo.title(), "TODO");
}

#[test]
fn panel_mode_from_arg() {
    assert_eq!(PanelMode::from_arg("diagnostics"), Some(PanelMode::Diagnostics));
    assert_eq!(PanelMode::from_arg("diag"), Some(PanelMode::Diagnostics));
    assert_eq!(PanelMode::from_arg("quickfix"), Some(PanelMode::Quickfix));
    assert_eq!(PanelMode::from_arg("qf"), Some(PanelMode::Quickfix));
    assert_eq!(PanelMode::from_arg("references"), Some(PanelMode::References));
    assert_eq!(PanelMode::from_arg("refs"), Some(PanelMode::References));
    assert_eq!(PanelMode::from_arg("todo"), Some(PanelMode::Todo));
    assert_eq!(PanelMode::from_arg("invalid"), None);
}

#[test]
fn panel_mode_debug_clone_copy_eq() {
    let a = PanelMode::Diagnostics;
    let b = a;
    assert_eq!(a, b);
    assert_ne!(PanelMode::Diagnostics, PanelMode::Quickfix);
    let debug = format!("{a:?}");
    assert!(debug.contains("Diagnostics"));
}

// ============================================================================
// SortOrder
// ============================================================================

#[test]
fn sort_order_cycle() {
    assert_eq!(SortOrder::BySeverity.next(), SortOrder::ByFile);
    assert_eq!(SortOrder::ByFile.next(), SortOrder::ByLine);
    assert_eq!(SortOrder::ByLine.next(), SortOrder::BySeverity);
}

#[test]
fn sort_order_debug_clone_copy_eq() {
    let a = SortOrder::BySeverity;
    let b = a;
    assert_eq!(a, b);
    assert_ne!(SortOrder::BySeverity, SortOrder::ByFile);
    let debug = format!("{a:?}");
    assert!(debug.contains("BySeverity"));
}

// ============================================================================
// SeverityFilter
// ============================================================================

#[test]
fn filter_all() {
    let f = SeverityFilter::all();
    assert!(f.allows(DiagnosticSeverity::Error));
    assert!(f.allows(DiagnosticSeverity::Warning));
    assert!(f.allows(DiagnosticSeverity::Information));
    assert!(f.allows(DiagnosticSeverity::Hint));
}

#[test]
fn filter_errors_only() {
    let f = SeverityFilter::errors_only();
    assert!(f.allows(DiagnosticSeverity::Error));
    assert!(!f.allows(DiagnosticSeverity::Warning));
    assert!(!f.allows(DiagnosticSeverity::Information));
    assert!(!f.allows(DiagnosticSeverity::Hint));
}

#[test]
fn filter_warnings_only() {
    let f = SeverityFilter::warnings_only();
    assert!(!f.allows(DiagnosticSeverity::Error));
    assert!(f.allows(DiagnosticSeverity::Warning));
    assert!(!f.allows(DiagnosticSeverity::Information));
    assert!(!f.allows(DiagnosticSeverity::Hint));
}

#[test]
fn filter_default_is_all() {
    let f = SeverityFilter::default();
    assert_eq!(f, SeverityFilter::all());
}

#[test]
fn filter_debug_clone_eq() {
    let f = SeverityFilter::all();
    let debug = format!("{f:?}");
    assert!(debug.contains("SeverityFilter"));

    #[allow(clippy::redundant_clone)]
    let cloned = f.clone();
    assert_eq!(f, cloned);
}

// ============================================================================
// sort_items
// ============================================================================

#[test]
fn sort_by_severity() {
    let mut items = vec![
        make_item(DiagnosticSeverity::Hint, "a.rs", 1),
        make_item(DiagnosticSeverity::Error, "a.rs", 2),
        make_item(DiagnosticSeverity::Warning, "a.rs", 3),
    ];
    sort_items(&mut items, SortOrder::BySeverity);
    assert_eq!(items[0].severity, DiagnosticSeverity::Error);
    assert_eq!(items[1].severity, DiagnosticSeverity::Warning);
    assert_eq!(items[2].severity, DiagnosticSeverity::Hint);
}

#[test]
fn sort_by_file() {
    let mut items = vec![
        make_item(DiagnosticSeverity::Error, "c.rs", 1),
        make_item(DiagnosticSeverity::Error, "a.rs", 1),
        make_item(DiagnosticSeverity::Error, "b.rs", 1),
    ];
    sort_items(&mut items, SortOrder::ByFile);
    assert_eq!(items[0].file_path, "a.rs");
    assert_eq!(items[1].file_path, "b.rs");
    assert_eq!(items[2].file_path, "c.rs");
}

#[test]
fn sort_by_line() {
    let mut items = vec![
        make_item(DiagnosticSeverity::Error, "a.rs", 10),
        make_item(DiagnosticSeverity::Error, "a.rs", 1),
        make_item(DiagnosticSeverity::Error, "a.rs", 5),
    ];
    sort_items(&mut items, SortOrder::ByLine);
    assert_eq!(items[0].line, 1);
    assert_eq!(items[1].line, 5);
    assert_eq!(items[2].line, 10);
}

#[test]
fn sort_empty() {
    let mut items: Vec<PanelItem> = vec![];
    sort_items(&mut items, SortOrder::BySeverity);
    assert!(items.is_empty());
}

// ============================================================================
// items_from_snapshot
// ============================================================================

#[test]
fn items_from_snapshot_converts_correctly() {
    let snapshot = DiagnosticSnapshot {
        entries: vec![BufferDiagnosticEntry {
            buffer_id: 1,
            diagnostics: vec![
                DiagnosticItem {
                    start_line: 5,
                    start_col: 10,
                    end_line: 5,
                    end_col: 15,
                    severity: DiagnosticSeverity::Error,
                    message: "type error".to_owned(),
                    source: Some("rustc".to_owned()),
                },
                DiagnosticItem {
                    start_line: 10,
                    start_col: 0,
                    end_line: 10,
                    end_col: 5,
                    severity: DiagnosticSeverity::Warning,
                    message: "unused var".to_owned(),
                    source: None,
                },
            ],
        }],
    };

    let items = items_from_snapshot(&snapshot, |id| {
        if id == 1 {
            Some("/src/main.rs".to_owned())
        } else {
            None
        }
    });

    assert_eq!(items.len(), 2);
    assert_eq!(items[0].file_path, "/src/main.rs");
    assert_eq!(items[0].line, 5);
    assert_eq!(items[0].col, 10);
    assert_eq!(items[0].severity, DiagnosticSeverity::Error);
    assert_eq!(items[0].message, "type error");
    assert_eq!(items[0].source.as_deref(), Some("rustc"));
    assert_eq!(items[0].buffer_id, Some(1));

    assert_eq!(items[1].severity, DiagnosticSeverity::Warning);
    assert!(items[1].source.is_none());
}

#[test]
fn items_from_snapshot_skips_unresolved() {
    let snapshot = DiagnosticSnapshot {
        entries: vec![BufferDiagnosticEntry {
            buffer_id: 99,
            diagnostics: vec![DiagnosticItem {
                start_line: 0,
                start_col: 0,
                end_line: 0,
                end_col: 1,
                severity: DiagnosticSeverity::Error,
                message: "err".to_owned(),
                source: None,
            }],
        }],
    };

    let items = items_from_snapshot(&snapshot, |_| None);
    assert!(items.is_empty());
}

#[test]
fn items_from_empty_snapshot() {
    let snapshot = DiagnosticSnapshot::default();
    let items = items_from_snapshot(&snapshot, |_| None);
    assert!(items.is_empty());
}

// ============================================================================
// PanelItem
// ============================================================================

#[test]
fn panel_item_debug_clone_eq() {
    let item = make_item(DiagnosticSeverity::Error, "a.rs", 1);
    let debug = format!("{item:?}");
    assert!(debug.contains("PanelItem"));

    #[allow(clippy::redundant_clone)]
    let cloned = item.clone();
    assert_eq!(item, cloned);
}
