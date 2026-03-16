use {reovim_driver_command::Command, reovim_kernel::api::v1::CommandId};

use super::*;

#[test]
fn test_format_document_id() {
    let cmd = FormatDocument;
    assert_eq!(cmd.id(), ids::FORMAT_DOCUMENT);
}

#[test]
fn test_format_document_description() {
    let cmd = FormatDocument;
    assert_eq!(cmd.description(), "Format current document");
}

#[test]
fn test_format_selection_id() {
    let cmd = FormatSelection;
    assert_eq!(cmd.id(), ids::FORMAT_SELECTION);
}

#[test]
fn test_format_selection_description() {
    let cmd = FormatSelection;
    assert_eq!(cmd.description(), "Format selected range");
}

#[test]
fn test_command_handlers_count() {
    let handlers = command_handlers();
    assert_eq!(handlers.len(), 2);
}

#[test]
fn test_command_handlers_ids() {
    let handlers = command_handlers();
    let cmd_ids: Vec<CommandId> = handlers.iter().map(|h| h.id()).collect();
    assert!(cmd_ids.contains(&ids::FORMAT_DOCUMENT));
    assert!(cmd_ids.contains(&ids::FORMAT_SELECTION));
}
