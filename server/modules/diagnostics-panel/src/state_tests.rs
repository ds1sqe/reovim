use reovim_driver_lsp::DiagnosticSeverity;

use {super::*, crate::items::PanelItem};

fn make_item(severity: DiagnosticSeverity, line: u32) -> PanelItem {
    PanelItem {
        file_path: "test.rs".to_owned(),
        line,
        col: 0,
        severity,
        message: "test".to_owned(),
        source: None,
        buffer_id: None,
    }
}

#[test]
fn create_defaults() {
    let state = DiagnosticsState::create();
    assert!(!state.active);
    assert_eq!(state.mode, PanelMode::Diagnostics);
    assert!(state.items.is_empty());
    assert_eq!(state.selected, 0);
    assert_eq!(state.scroll_offset, 0);
    assert_eq!(state.sort_order, SortOrder::BySeverity);
    assert_eq!(state.severity_filter, SeverityFilter::all());
}

#[test]
fn set_items_sorts_and_resets() {
    let mut state = DiagnosticsState::create();
    state.selected = 5;
    state.scroll_offset = 10;

    let items = vec![
        make_item(DiagnosticSeverity::Warning, 3),
        make_item(DiagnosticSeverity::Error, 1),
    ];
    state.set_items(items);

    assert_eq!(state.items.len(), 2);
    assert_eq!(state.items[0].severity, DiagnosticSeverity::Error);
    assert_eq!(state.selected, 0);
    assert_eq!(state.scroll_offset, 0);
}

#[test]
fn set_items_filters() {
    let mut state = DiagnosticsState::create();
    state.severity_filter = SeverityFilter::errors_only();

    let items = vec![
        make_item(DiagnosticSeverity::Warning, 1),
        make_item(DiagnosticSeverity::Error, 2),
        make_item(DiagnosticSeverity::Hint, 3),
    ];
    state.set_items(items);

    assert_eq!(state.items.len(), 1);
    assert_eq!(state.items[0].severity, DiagnosticSeverity::Error);
}

#[test]
fn select_next_wraps() {
    let mut state = DiagnosticsState::create();
    state.items = vec![
        make_item(DiagnosticSeverity::Error, 1),
        make_item(DiagnosticSeverity::Error, 2),
        make_item(DiagnosticSeverity::Error, 3),
    ];
    assert_eq!(state.selected, 0);

    state.select_next();
    assert_eq!(state.selected, 1);

    state.select_next();
    assert_eq!(state.selected, 2);

    state.select_next();
    assert_eq!(state.selected, 0); // wraps
}

#[test]
fn select_prev_wraps() {
    let mut state = DiagnosticsState::create();
    state.items = vec![
        make_item(DiagnosticSeverity::Error, 1),
        make_item(DiagnosticSeverity::Error, 2),
    ];
    assert_eq!(state.selected, 0);

    state.select_prev();
    assert_eq!(state.selected, 1); // wraps to end

    state.select_prev();
    assert_eq!(state.selected, 0);
}

#[test]
fn select_on_empty() {
    let mut state = DiagnosticsState::create();
    state.select_next();
    assert_eq!(state.selected, 0);
    state.select_prev();
    assert_eq!(state.selected, 0);
}

#[test]
fn selected_item() {
    let mut state = DiagnosticsState::create();
    assert!(state.selected_item().is_none());

    state.items = vec![make_item(DiagnosticSeverity::Error, 5)];
    let item = state.selected_item().unwrap();
    assert_eq!(item.line, 5);
}

#[test]
fn severity_counts() {
    let mut state = DiagnosticsState::create();
    state.items = vec![
        make_item(DiagnosticSeverity::Error, 1),
        make_item(DiagnosticSeverity::Error, 2),
        make_item(DiagnosticSeverity::Warning, 3),
        make_item(DiagnosticSeverity::Information, 4),
        make_item(DiagnosticSeverity::Hint, 5),
        make_item(DiagnosticSeverity::Hint, 6),
    ];

    let counts = state.severity_counts();
    assert_eq!(
        counts,
        SeverityCounts {
            errors: 2,
            warnings: 1,
            info: 1,
            hints: 2,
        }
    );
}

#[test]
fn severity_counts_empty() {
    let state = DiagnosticsState::create();
    assert_eq!(state.severity_counts(), SeverityCounts::default());
}

#[test]
fn refilter_and_sort() {
    let mut state = DiagnosticsState::create();
    state.items = vec![
        make_item(DiagnosticSeverity::Warning, 1),
        make_item(DiagnosticSeverity::Error, 2),
        make_item(DiagnosticSeverity::Hint, 3),
    ];
    state.selected = 2;

    // Change filter to errors only
    state.severity_filter = SeverityFilter::errors_only();
    state.refilter_and_sort();

    assert_eq!(state.items.len(), 1);
    assert_eq!(state.items[0].severity, DiagnosticSeverity::Error);
    assert_eq!(state.selected, 0); // clamped
}

#[test]
fn refilter_clamps_selection() {
    let mut state = DiagnosticsState::create();
    state.items = vec![
        make_item(DiagnosticSeverity::Error, 1),
        make_item(DiagnosticSeverity::Error, 2),
    ];
    state.selected = 1;

    // Filter to something that leaves one item
    state.severity_filter = SeverityFilter {
        errors: true,
        warnings: false,
        information: false,
        hints: false,
    };
    // Remove second item manually to test clamping
    state.items.remove(1);
    state.refilter_and_sort();

    assert_eq!(state.selected, 0);
}

#[test]
fn severity_counts_debug() {
    let counts = SeverityCounts::default();
    let debug = format!("{counts:?}");
    assert!(debug.contains("SeverityCounts"));
}
