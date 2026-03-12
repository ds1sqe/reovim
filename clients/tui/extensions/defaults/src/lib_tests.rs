use super::*;

#[test]
fn test_create_extensions_count() {
    let exts = create_extensions();
    assert_eq!(exts.len(), 13);
}

#[test]
fn test_extension_kinds() {
    let exts = create_extensions();
    let kinds: Vec<&str> = exts.iter().map(|e| e.kind()).collect();
    assert!(kinds.contains(&"whichkey"));
    assert!(kinds.contains(&"cmdline"));
    assert!(kinds.contains(&"notification"));
    assert!(kinds.contains(&"microscope"));
    assert!(kinds.contains(&"completion"));
    assert!(kinds.contains(&"explorer"));
    assert!(kinds.contains(&"polyblocks"));
    assert!(kinds.contains(&"range-finder-jump"));
    assert!(kinds.contains(&"range-finder-fold"));
    assert!(kinds.contains(&"hover"));
    assert!(kinds.contains(&"signature-help"));
    assert!(kinds.contains(&"diagnostics"));
    assert!(kinds.contains(&"markdown"));
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

    // Find whichkey extension and activate it
    for ext in &mut exts {
        if ext.kind() == "whichkey" {
            ext.apply_notification(
                r#"{"active":true,"prefix":"g","hints":[{"key":"g","command":"top"}]}"#,
            );
        }
    }

    // With the default 500ms show-delay, whichkey is not yet visible
    // (server_active=true but visible=false until tick() after delay)
    // Markdown is always active (lightweight), so count = 1
    let active_count = exts.iter().filter(|e| e.is_active()).count();
    assert_eq!(active_count, 1);
}
