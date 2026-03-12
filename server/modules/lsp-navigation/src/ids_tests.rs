use super::*;

#[test]
fn module_id() {
    assert_eq!(MODULE.as_str(), "lsp-navigation");
}

#[test]
fn goto_definition_id() {
    assert_eq!(GOTO_DEFINITION.name(), "goto-definition");
    assert_eq!(*GOTO_DEFINITION.module(), MODULE);
}

#[test]
fn references_id() {
    assert_eq!(REFERENCES.name(), "references");
    assert_eq!(*REFERENCES.module(), MODULE);
}

#[test]
fn hover_id() {
    assert_eq!(HOVER.name(), "hover");
    assert_eq!(*HOVER.module(), MODULE);
}

#[test]
fn signature_help_id() {
    assert_eq!(SIGNATURE_HELP.name(), "signature-help");
    assert_eq!(*SIGNATURE_HELP.module(), MODULE);
}
