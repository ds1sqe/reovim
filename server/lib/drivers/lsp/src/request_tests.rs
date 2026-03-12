use super::*;

fn make_uri(path: &str) -> Uri {
    path.parse().expect("test URI should parse")
}

#[test]
fn test_did_open_debug() {
    let req = LspRequest::DidOpen {
        uri: make_uri("file:///test.rs"),
        language_id: "rust".to_string(),
        version: 1,
        content: "fn main() {}".to_string(),
    };
    let debug_str = format!("{req:?}");
    assert!(debug_str.contains("DidOpen"));
    assert!(debug_str.contains("rust"));
    assert!(debug_str.contains("version: 1"));
    // Content should NOT be in debug output
    assert!(!debug_str.contains("fn main"));
}

#[test]
fn test_did_change_debug() {
    let req = LspRequest::DidChange {
        uri: make_uri("file:///test.rs"),
        version: 2,
        content: "fn main() { println!(\"hello\"); }".to_string(),
    };
    let debug_str = format!("{req:?}");
    assert!(debug_str.contains("DidChange"));
    assert!(debug_str.contains("version: 2"));
}

#[test]
fn test_did_close_debug() {
    let req = LspRequest::DidClose {
        uri: make_uri("file:///test.rs"),
    };
    let debug_str = format!("{req:?}");
    assert!(debug_str.contains("DidClose"));
    assert!(debug_str.contains("test.rs"));
}

#[test]
fn test_shutdown_debug() {
    let req = LspRequest::Shutdown;
    assert_eq!(format!("{req:?}"), "Shutdown");
}

#[test]
fn test_goto_definition_debug() {
    let (tx, _rx) = reovim_kernel::api::v1::oneshot();
    let req = LspRequest::GotoDefinition {
        uri: make_uri("file:///test.rs"),
        position: Position::new(10, 5),
        response_tx: tx,
    };
    let debug_str = format!("{req:?}");
    assert!(debug_str.contains("GotoDefinition"));
    assert!(debug_str.contains("position"));
    // response_tx should NOT be in debug output
    assert!(!debug_str.contains("response_tx"));
}

#[test]
fn test_references_debug() {
    let (tx, _rx) = reovim_kernel::api::v1::oneshot();
    let req = LspRequest::References {
        uri: make_uri("file:///test.rs"),
        position: Position::new(5, 10),
        include_declaration: true,
        response_tx: tx,
    };
    let debug_str = format!("{req:?}");
    assert!(debug_str.contains("References"));
    assert!(debug_str.contains("include_declaration: true"));
}

#[test]
fn test_completion_debug() {
    let (tx, _rx) = reovim_kernel::api::v1::oneshot();
    let req = LspRequest::Completion {
        uri: make_uri("file:///test.rs"),
        position: Position::new(5, 10),
        response_tx: tx,
    };
    let debug_str = format!("{req:?}");
    assert!(debug_str.contains("Completion"));
    assert!(debug_str.contains("position"));
    assert!(!debug_str.contains("response_tx"));
}

#[test]
fn test_hover_debug() {
    let (tx, _rx) = reovim_kernel::api::v1::oneshot();
    let req = LspRequest::Hover {
        uri: make_uri("file:///test.rs"),
        position: Position::new(1, 1),
        response_tx: tx,
    };
    let debug_str = format!("{req:?}");
    assert!(debug_str.contains("Hover"));
    assert!(debug_str.contains("position"));
}

#[test]
fn test_did_save_debug() {
    let req = LspRequest::DidSave {
        uri: make_uri("file:///test.rs"),
        text: Some("fn main() {}".to_string()),
    };
    let debug_str = format!("{req:?}");
    assert!(debug_str.contains("DidSave"));
    assert!(debug_str.contains("test.rs"));
    // text should NOT be in debug output (finish_non_exhaustive)
}

#[test]
fn test_did_save_no_text_debug() {
    let req = LspRequest::DidSave {
        uri: make_uri("file:///test.rs"),
        text: None,
    };
    let debug_str = format!("{req:?}");
    assert!(debug_str.contains("DidSave"));
}

#[test]
fn test_signature_help_debug() {
    let (tx, _rx) = reovim_kernel::api::v1::oneshot();
    let req = LspRequest::SignatureHelp {
        uri: make_uri("file:///test.rs"),
        position: Position::new(3, 7),
        response_tx: tx,
    };
    let debug_str = format!("{req:?}");
    assert!(debug_str.contains("SignatureHelp"));
    assert!(debug_str.contains("position"));
    assert!(!debug_str.contains("response_tx"));
}
