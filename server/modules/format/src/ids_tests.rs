use super::*;

#[test]
fn module_id() {
    assert_eq!(MODULE.as_str(), "format");
}

#[test]
fn format_document_id() {
    assert_eq!(*FORMAT_DOCUMENT.module(), MODULE);
    assert_eq!(FORMAT_DOCUMENT.name(), "format-document");
}

#[test]
fn format_selection_id() {
    assert_eq!(*FORMAT_SELECTION.module(), MODULE);
    assert_eq!(FORMAT_SELECTION.name(), "format-selection");
}
