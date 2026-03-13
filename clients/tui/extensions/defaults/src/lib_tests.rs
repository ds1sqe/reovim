use std::collections::HashSet;

use super::*;

#[test]
fn test_create_extensions_count() {
    let exts = create_extensions();
    // Legacy extensions: cmdline, microscope, explorer, tetromino,
    // range-finder-jump, range-finder-fold, diagnostics, markdown, pair = 9
    assert_eq!(exts.len(), 9);
}

#[test]
fn test_extension_kinds() {
    let exts = create_extensions();
    let kinds: Vec<&str> = exts.iter().map(|e| e.kind()).collect();
    assert!(kinds.contains(&"cmdline"));
    assert!(kinds.contains(&"microscope"));
    assert!(kinds.contains(&"explorer"));
    assert!(kinds.contains(&"polyblocks")); // tetromino kind
    assert!(kinds.contains(&"range-finder-jump"));
    assert!(kinds.contains(&"range-finder-fold"));
    assert!(kinds.contains(&"diagnostics"));
    assert!(kinds.contains(&"markdown"));
    assert!(kinds.contains(&"pair"));
}

#[test]
fn test_all_extensions_initially_inactive() {
    let exts = create_extensions();
    for ext in &exts {
        // Markdown is always active (lightweight, no-op when no tables)
        if ext.kind() == "markdown" {
            assert!(ext.is_active());
            continue;
        }
        assert!(!ext.is_active(), "Extension '{}' should start inactive", ext.kind());
    }
}

#[test]
fn test_extension_dispatch() {
    let mut exts = create_extensions();

    // Find cmdline extension and activate it
    for ext in &mut exts {
        if ext.kind() == "cmdline" {
            ext.apply_notification(
                r#"{"active":true,"prompt":":","content":"","cursor_pos":0}"#,
            );
        }
    }

    // Markdown is always active = 1, cmdline just activated = 2
    let active_count = exts.iter().filter(|e| e.is_active()).count();
    assert_eq!(active_count, 2);
}

#[test]
fn test_create_extensions_sorted_by_depgraph() {
    // All current extensions have empty deps, so order is valid (no panic)
    let exts = create_extensions();
    assert_eq!(exts.len(), 9);
}

#[test]
fn test_shutdown_extensions_calls_exit() {
    let mut exts = create_extensions();
    // Should not panic — all extensions have no-op exit()
    shutdown_extensions(&mut exts);
}

#[test]
fn test_toposort_reorders_with_declared_deps() {
    // Verify the mechanism actually reorders when deps are declared.
    // Use mock extensions: B depends on A, so A must come first.
    use reovim_driver_display::render_backend::RenderBackend;

    struct MockA;
    impl TuiExtension for MockA {
        fn kind(&self) -> &'static str {
            "mock-a"
        }
        fn is_active(&self) -> bool {
            false
        }
        fn apply_notification(&mut self, _data: &str) {}
        fn render(&self, _backend: &mut dyn RenderBackend) {}
    }

    struct MockB;
    impl TuiExtension for MockB {
        fn kind(&self) -> &'static str {
            "mock-b"
        }
        fn is_active(&self) -> bool {
            false
        }
        fn apply_notification(&mut self, _data: &str) {}
        fn render(&self, _backend: &mut dyn RenderBackend) {}
        fn dependencies(&self) -> &[&'static str] {
            &["mock-a"]
        }
    }

    // Create in wrong order: B first, then A
    let extensions: Vec<Box<dyn TuiExtension>> = vec![Box::new(MockB), Box::new(MockA)];

    let entries: Vec<DepEntry<&'static str>> = extensions
        .iter()
        .map(|ext| DepEntry {
            key: ext.kind(),
            required: ext.dependencies().to_vec(),
            optional: Vec::new(),
            provides_caps: vec![],
            requires_caps: vec![],
        })
        .collect();

    let dep_order = resolve_dependencies(&entries).unwrap();

    let mut by_kind: HashMap<&'static str, Box<dyn TuiExtension>> = extensions
        .into_iter()
        .map(|ext| (ext.kind(), ext))
        .collect();
    let sorted: Vec<Box<dyn TuiExtension>> = dep_order
        .order
        .iter()
        .filter_map(|kind| by_kind.remove(kind))
        .collect();

    // A must come before B after toposort
    assert_eq!(sorted[0].kind(), "mock-a");
    assert_eq!(sorted[1].kind(), "mock-b");
}

#[test]
fn test_validate_extensions_all_matched() {
    let exts = create_extensions();
    // Provide all kinds that extensions need
    let server_kinds: Vec<String> = exts
        .iter()
        .flat_map(|e| e.server_kinds())
        .map(String::from)
        .collect();
    // Should not warn — all matched
    validate_extensions(&exts, &server_kinds);
}

#[test]
fn test_validate_extensions_warns_unmatched() {
    use reovim_driver_display::render_backend::RenderBackend;

    struct UnmatchedExt;
    impl TuiExtension for UnmatchedExt {
        fn kind(&self) -> &'static str {
            "unmatched"
        }
        fn is_active(&self) -> bool {
            false
        }
        fn apply_notification(&mut self, _data: &str) {}
        fn render(&self, _backend: &mut dyn RenderBackend) {}
    }

    let exts: Vec<Box<dyn TuiExtension>> = vec![Box::new(UnmatchedExt)];
    let server_kinds: Vec<String> = vec!["other".into()];
    // Should warn but not panic
    validate_extensions(&exts, &server_kinds);
}

#[test]
fn test_validate_extensions_empty() {
    let exts: Vec<Box<dyn TuiExtension>> = vec![];
    let server_kinds: Vec<String> = vec![];
    validate_extensions(&exts, &server_kinds);
}

// ============================================================================
// Filtered extension creation (#586)
// ============================================================================

#[test]
fn test_create_extensions_filtered_empty_set_returns_all() {
    let exts = create_extensions_filtered(&HashSet::new());
    assert_eq!(exts.len(), 9);
}

#[test]
fn test_create_extensions_filtered_disable_one() {
    let disabled: HashSet<String> = ["cmdline".to_string()].into();
    let exts = create_extensions_filtered(&disabled);
    assert_eq!(exts.len(), 8);
    assert!(exts.iter().all(|e| e.kind() != "cmdline"));
}

#[test]
fn test_create_extensions_filtered_disable_multiple() {
    let disabled: HashSet<String> = [
        "cmdline".to_string(),
        "microscope".to_string(),
        "explorer".to_string(),
    ]
    .into();
    let exts = create_extensions_filtered(&disabled);
    assert_eq!(exts.len(), 6);
    for ext in &exts {
        assert!(!disabled.contains(ext.kind()));
    }
}

#[test]
fn test_create_extensions_filtered_unknown_kinds_ignored() {
    let disabled: HashSet<String> = ["nonexistent".to_string()].into();
    let exts = create_extensions_filtered(&disabled);
    assert_eq!(exts.len(), 9);
}

#[test]
fn test_create_extensions_filtered_all_disabled() {
    let all = create_extensions();
    let disabled: HashSet<String> = all.iter().map(|e| e.kind().to_string()).collect();
    let exts = create_extensions_filtered(&disabled);
    assert!(exts.is_empty());
}

#[test]
fn test_create_extensions_matches_filtered_empty() {
    let unfiltered = create_extensions();
    let filtered = create_extensions_filtered(&HashSet::new());
    assert_eq!(unfiltered.len(), filtered.len());
    let unfiltered_kinds: HashSet<&str> = unfiltered.iter().map(|e| e.kind()).collect();
    let filtered_kinds: HashSet<&str> = filtered.iter().map(|e| e.kind()).collect();
    assert_eq!(unfiltered_kinds, filtered_kinds);
}

// ============================================================================
// Native modules (#634)
// ============================================================================

#[test]
fn test_create_native_modules_count() {
    let modules = create_native_modules();
    // statusline, hover, signature-help, landing, completion, notification, whichkey = 7
    assert_eq!(modules.len(), 7);
}

#[test]
fn test_native_module_kinds() {
    let modules = create_native_modules();
    let kinds: Vec<&str> = modules.iter().map(|m| m.kind()).collect();
    assert!(kinds.contains(&"statusline"));
    assert!(kinds.contains(&"hover"));
    assert!(kinds.contains(&"signature-help"));
    assert!(kinds.contains(&"landing"));
    assert!(kinds.contains(&"completion"));
    assert!(kinds.contains(&"notification"));
    assert!(kinds.contains(&"whichkey"));
}
