use super::*;

#[test]
fn test_response_debug() {
    let resp = LspResponse::Empty;
    assert_eq!(format!("{resp:?}"), "Empty");
}

#[test]
fn test_is_empty() {
    assert!(LspResponse::Empty.is_empty());
    assert!(LspResponse::Definition(None).is_empty());
    assert!(LspResponse::References(None).is_empty());
    assert!(LspResponse::Hover(None).is_empty());
}

#[test]
fn test_is_not_empty() {
    let locations = vec![];
    assert!(!LspResponse::References(Some(locations)).is_empty());
}

#[test]
fn test_is_empty_all_none_variants() {
    assert!(LspResponse::DocumentSymbol(None).is_empty());
    assert!(LspResponse::Completion(None).is_empty());
    assert!(LspResponse::SignatureHelp(None).is_empty());
    assert!(LspResponse::CodeAction(None).is_empty());
    assert!(LspResponse::Rename(None).is_empty());
    assert!(LspResponse::WorkspaceSymbol(None).is_empty());
    assert!(LspResponse::Formatting(None).is_empty());
}

#[test]
fn test_is_not_empty_with_some_variants() {
    assert!(!LspResponse::Formatting(Some(vec![])).is_empty());
    assert!(!LspResponse::WorkspaceSymbol(Some(vec![])).is_empty());
    assert!(!LspResponse::CodeAction(Some(vec![])).is_empty());
}

#[test]
fn test_response_debug_variants() {
    let resp = LspResponse::Definition(None);
    assert!(format!("{resp:?}").contains("Definition"));

    let resp = LspResponse::Hover(None);
    assert!(format!("{resp:?}").contains("Hover"));

    let resp = LspResponse::Rename(None);
    assert!(format!("{resp:?}").contains("Rename"));
}
