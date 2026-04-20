use reovim_client_driver::ClientModule;

use super::*;

#[test]
fn test_module_metadata() {
    let module = IlluminateModule::new();
    assert_eq!(module.id(), "illuminate");
    assert_eq!(module.kind(), "illuminate");
    assert_eq!(module.name(), "Illuminate");
    assert_eq!(module.server_kinds(), vec!["illuminate"]);
}

#[test]
fn test_no_buffer_contrib_initially() {
    let module = IlluminateModule::new();
    assert!(!module.has_buffer_contrib());
}

#[test]
fn test_notification_activates_highlights() {
    let mut module = IlluminateModule::new();
    let json = r#"{
        "active": true,
        "bufferId": 1,
        "word": "count",
        "ranges": [
            {"startLine": 0, "startCol": 4, "endLine": 0, "endCol": 9, "kind": "text"},
            {"startLine": 2, "startCol": 4, "endLine": 2, "endCol": 9, "kind": "text"}
        ],
        "sequence": 1
    }"#;

    module.on_notification(json);

    assert!(module.has_buffer_contrib());
    assert_eq!(module.inline_decorations(0).len(), 1);
    assert_eq!(module.inline_decorations(2).len(), 1);
    assert!(module.inline_decorations(1).is_empty());

    // Check column range
    let dec = &module.inline_decorations(0)[0];
    assert_eq!(dec.col_start, 4);
    assert_eq!(dec.col_end, 9);
}

#[test]
fn test_notification_deactivates_highlights() {
    let mut module = IlluminateModule::new();

    // Activate first
    module.on_notification(
        r#"{
        "active": true,
        "bufferId": 1,
        "word": "foo",
        "ranges": [
            {"startLine": 0, "startCol": 0, "endLine": 0, "endCol": 3, "kind": "text"},
            {"startLine": 1, "startCol": 0, "endLine": 1, "endCol": 3, "kind": "text"}
        ],
        "sequence": 1
    }"#,
    );
    assert!(module.has_buffer_contrib());

    // Deactivate
    module.on_notification(r#"{"active": false, "sequence": 2}"#);
    assert!(!module.has_buffer_contrib());
    assert!(module.inline_decorations(0).is_empty());
}

#[test]
fn test_duplicate_sequence_ignored() {
    let mut module = IlluminateModule::new();

    module.on_notification(
        r#"{
        "active": true,
        "bufferId": 1,
        "word": "x",
        "ranges": [
            {"startLine": 0, "startCol": 0, "endLine": 0, "endCol": 1, "kind": "text"},
            {"startLine": 1, "startCol": 0, "endLine": 1, "endCol": 1, "kind": "text"}
        ],
        "sequence": 5
    }"#,
    );
    assert!(module.has_buffer_contrib());

    // Same sequence — should be ignored (highlights remain)
    module.on_notification(r#"{"active": false, "sequence": 5}"#);
    assert!(module.has_buffer_contrib()); // NOT deactivated
}

#[test]
fn test_inline_decorations_empty_line() {
    let module = IlluminateModule::new();
    assert!(module.inline_decorations(999).is_empty());
}

#[test]
fn test_invalid_json_ignored() {
    let mut module = IlluminateModule::new();
    module.on_notification("not json");
    assert!(!module.has_buffer_contrib());
}

#[test]
fn test_default() {
    let module = IlluminateModule::default();
    assert!(!module.active);
}
