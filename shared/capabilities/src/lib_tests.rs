use super::*;

#[test]
fn test_all_count() {
    assert_eq!(ALL.len(), 15, "Update ALL array when adding/removing capabilities");
}

#[test]
fn test_all_sorted() {
    for window in ALL.windows(2) {
        assert!(
            window[0] < window[1],
            "ALL array must be sorted: {:?} >= {:?}",
            window[0],
            window[1]
        );
    }
}

#[test]
fn test_no_duplicates() {
    let mut seen = std::collections::HashSet::new();
    for cap in ALL {
        assert!(seen.insert(cap), "Duplicate capability: {cap}");
    }
}

#[test]
fn test_all_contains_each_constant() {
    assert!(ALL.contains(&BUFFER_MANAGER));
    assert!(ALL.contains(&CLIPBOARD_PROVIDER));
    assert!(ALL.contains(&COMMAND_DISPATCH));
    assert!(ALL.contains(&COMPLETION_PROVIDER));
    assert!(ALL.contains(&FILE_EXPLORER));
    assert!(ALL.contains(&FUZZY_FINDER));
    assert!(ALL.contains(&GIT_PROVIDER));
    assert!(ALL.contains(&LSP_PROVIDER));
    assert!(ALL.contains(&MODE_MANAGEMENT));
    assert!(ALL.contains(&MOTION_COMMANDS));
    assert!(ALL.contains(&SEARCH_PROVIDER));
    assert!(ALL.contains(&SNIPPET_PROVIDER));
    assert!(ALL.contains(&SYNTAX_HIGHLIGHTING));
    assert!(ALL.contains(&UNDO_PROVIDER));
    assert!(ALL.contains(&VFS_PROVIDER));
}

#[test]
fn test_constants_are_kebab_case() {
    for cap in ALL {
        assert!(
            cap.chars().all(|c| c.is_ascii_lowercase() || c == '-'),
            "Capability {cap:?} must be kebab-case (lowercase ASCII + hyphens)"
        );
    }
}
