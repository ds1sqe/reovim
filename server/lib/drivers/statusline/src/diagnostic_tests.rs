use super::DiagnosticCounts;

#[test]
fn test_default_is_empty() {
    let counts = DiagnosticCounts::default();
    assert!(counts.is_empty());
    assert_eq!(counts.total(), 0);
}

#[test]
fn test_with_errors() {
    let counts = DiagnosticCounts {
        errors: 3,
        ..DiagnosticCounts::default()
    };
    assert!(!counts.is_empty());
    assert_eq!(counts.total(), 3);
}

#[test]
fn test_with_all_types() {
    let counts = DiagnosticCounts {
        errors: 1,
        warnings: 2,
        info: 3,
        hints: 4,
    };
    assert!(!counts.is_empty());
    assert_eq!(counts.total(), 10);
}

#[test]
fn test_only_hints() {
    let counts = DiagnosticCounts {
        hints: 1,
        ..DiagnosticCounts::default()
    };
    assert!(!counts.is_empty());
    assert_eq!(counts.total(), 1);
}

#[test]
fn test_clone() {
    let counts = DiagnosticCounts {
        errors: 5,
        warnings: 3,
        info: 0,
        hints: 0,
    };
    let cloned = counts.clone();
    assert_eq!(counts, cloned);
}

#[test]
fn test_debug() {
    let counts = DiagnosticCounts::default();
    let debug = format!("{counts:?}");
    assert!(debug.contains("DiagnosticCounts"));
}
