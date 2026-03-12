use super::*;

#[test]
fn test_all_count() {
    assert_eq!(ALL.len(), 15);
}

#[test]
fn test_all_sorted() {
    for window in ALL.windows(2) {
        assert!(window[0] < window[1], "ALL is not sorted: {:?} >= {:?}", window[0], window[1]);
    }
}

#[test]
fn test_all_unique() {
    let mut seen = std::collections::HashSet::new();
    for kind in ALL {
        assert!(seen.insert(kind), "Duplicate kind in ALL: {kind:?}");
    }
}

#[test]
fn test_constants_nonempty() {
    for kind in ALL {
        assert!(!kind.is_empty(), "Kind constant must not be empty");
    }
}

#[test]
fn test_constants_match_expected_values() {
    assert_eq!(WHICHKEY, "whichkey");
    assert_eq!(CMDLINE, "cmdline");
    assert_eq!(NOTIFICATION, "notification");
    assert_eq!(MICROSCOPE, "microscope");
    assert_eq!(COMPLETION, "completion");
    assert_eq!(EXPLORER, "explorer");
    assert_eq!(POLYBLOCKS, "polyblocks");
    assert_eq!(RANGE_FINDER_JUMP, "range-finder-jump");
    assert_eq!(RANGE_FINDER_FOLD, "range-finder-fold");
    assert_eq!(HOVER, "hover");
    assert_eq!(SIGNATURE_HELP, "signature-help");
    assert_eq!(DIAGNOSTICS, "diagnostics");
    assert_eq!(MARKDOWN, "markdown");
    assert_eq!(MODULE_MANAGER, "module-manager");
    assert_eq!(PAIR, "pair");
}
