use {super::*, reovim_driver_lsp::diagnostic_snapshot::DiagnosticSeverity};

fn pos(line: u32, col: u32, severity: DiagnosticSeverity) -> (u32, u32, DiagnosticSeverity) {
    (line, col, severity)
}

// ============================================================================
// Command identity tests
// ============================================================================

#[test]
fn next_diagnostic_id() {
    assert_eq!(NextDiagnostic.id(), ids::NEXT_DIAGNOSTIC);
    assert!(!NextDiagnostic.description().is_empty());
}

#[test]
fn prev_diagnostic_id() {
    assert_eq!(PrevDiagnostic.id(), ids::PREV_DIAGNOSTIC);
    assert!(!PrevDiagnostic.description().is_empty());
}

#[test]
fn next_error_id() {
    assert_eq!(NextError.id(), ids::NEXT_ERROR);
}

#[test]
fn prev_error_id() {
    assert_eq!(PrevError.id(), ids::PREV_ERROR);
}

#[test]
fn next_warning_id() {
    assert_eq!(NextWarning.id(), ids::NEXT_WARNING);
}

#[test]
fn prev_warning_id() {
    assert_eq!(PrevWarning.id(), ids::PREV_WARNING);
}

#[test]
fn command_handlers_count() {
    assert_eq!(command_handlers().len(), 6);
}

// ============================================================================
// find_next_pos / find_prev_pos tests
// ============================================================================

#[test]
fn find_next_returns_after_cursor() {
    let positions = vec![
        pos(5, 0, DiagnosticSeverity::Error),
        pos(10, 0, DiagnosticSeverity::Warning),
        pos(15, 0, DiagnosticSeverity::Error),
    ];

    let result = find_next_pos(&positions, 7, 0);
    assert_eq!(result, Some((10, 0)));
}

#[test]
fn find_next_wraps_around() {
    let positions = vec![
        pos(5, 0, DiagnosticSeverity::Error),
        pos(10, 0, DiagnosticSeverity::Warning),
    ];

    let result = find_next_pos(&positions, 20, 0);
    assert_eq!(result, Some((5, 0)));
}

#[test]
fn find_prev_returns_before_cursor() {
    let positions = vec![
        pos(5, 0, DiagnosticSeverity::Error),
        pos(10, 0, DiagnosticSeverity::Warning),
        pos(15, 0, DiagnosticSeverity::Error),
    ];

    let result = find_prev_pos(&positions, 12, 0);
    assert_eq!(result, Some((10, 0)));
}

#[test]
fn find_prev_wraps_around() {
    let positions = vec![
        pos(5, 0, DiagnosticSeverity::Error),
        pos(10, 0, DiagnosticSeverity::Warning),
    ];

    let result = find_prev_pos(&positions, 2, 0);
    assert_eq!(result, Some((10, 0)));
}

#[test]
fn find_next_empty_returns_none() {
    let positions: Vec<(u32, u32, DiagnosticSeverity)> = vec![];
    assert!(find_next_pos(&positions, 0, 0).is_none());
}

#[test]
fn find_prev_empty_returns_none() {
    let positions: Vec<(u32, u32, DiagnosticSeverity)> = vec![];
    assert!(find_prev_pos(&positions, 0, 0).is_none());
}

#[test]
fn find_next_same_position_wraps() {
    let positions = vec![pos(5, 3, DiagnosticSeverity::Error)];

    let result = find_next_pos(&positions, 5, 3);
    assert_eq!(result, Some((5, 3)));
}

// ============================================================================
// Severity filter tests
// ============================================================================

#[test]
fn matches_filter_any() {
    assert!(matches_filter(DiagnosticSeverity::Error, SeverityFilter::Any));
    assert!(matches_filter(DiagnosticSeverity::Warning, SeverityFilter::Any));
    assert!(matches_filter(DiagnosticSeverity::Information, SeverityFilter::Any));
    assert!(matches_filter(DiagnosticSeverity::Hint, SeverityFilter::Any));
}

#[test]
fn matches_filter_error_only() {
    assert!(matches_filter(DiagnosticSeverity::Error, SeverityFilter::Error));
    assert!(!matches_filter(DiagnosticSeverity::Warning, SeverityFilter::Error));
    assert!(!matches_filter(DiagnosticSeverity::Information, SeverityFilter::Error));
}

#[test]
fn matches_filter_warning_only() {
    assert!(matches_filter(DiagnosticSeverity::Warning, SeverityFilter::Warning));
    assert!(!matches_filter(DiagnosticSeverity::Error, SeverityFilter::Warning));
    assert!(!matches_filter(DiagnosticSeverity::Hint, SeverityFilter::Warning));
}
