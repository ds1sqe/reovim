//! Tests for DispatchResult types.

use {super::dispatch_result::*, reovim_kernel::api::v1::BufferId};

#[test]
fn buffer_changes_none_has_no_changes() {
    let changes = BufferChanges::none();
    assert!(!changes.has_changes());
}

#[test]
fn buffer_changes_modified_has_changes() {
    let changes = BufferChanges {
        modified: vec![BufferId::from_raw(1)],
        ..BufferChanges::none()
    };
    assert!(changes.has_changes());
}

#[test]
fn buffer_changes_created_has_changes() {
    let changes = BufferChanges {
        created: vec![BufferId::from_raw(1)],
        ..BufferChanges::none()
    };
    assert!(changes.has_changes());
}

#[test]
fn buffer_changes_closed_has_changes() {
    let changes = BufferChanges {
        closed: vec![BufferId::from_raw(1)],
        ..BufferChanges::none()
    };
    assert!(changes.has_changes());
}

#[test]
fn dispatch_result_default_is_continue() {
    let result = DispatchResult::default();
    assert_eq!(result.directive, Directive::Continue);
    assert!(!result.buffers.has_changes());
}

#[test]
fn directive_variants() {
    assert_eq!(Directive::default(), Directive::Continue);
    assert_ne!(Directive::Quit, Directive::Continue);
    assert_ne!(Directive::Detach, Directive::Continue);
    assert_ne!(Directive::Suspend, Directive::Continue);
    assert_ne!(Directive::ForceRedraw, Directive::Continue);
}

#[test]
fn command_result_handled() {
    let result = CommandResult::Handled(DispatchResult::default());
    assert!(matches!(result, CommandResult::Handled(_)));
}

#[test]
fn command_result_not_handled() {
    let result = CommandResult::NotHandled;
    assert!(matches!(result, CommandResult::NotHandled));
}

#[test]
fn command_result_error() {
    let result = CommandResult::Error("test error".to_string());
    assert!(matches!(result, CommandResult::Error(_)));
}
