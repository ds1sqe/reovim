use {
    reovim_kernel::api::v1::BufferId,
    reovim_subsys_session::{CommandResult, Directive},
};

use {super::*, crate::api::StateChanges};

#[test]
fn empty_changes_produce_empty_dispatch_result() {
    let changes = StateChanges::new();
    let result = state_changes_to_dispatch_result(&changes);
    assert!(!result.buffers.has_changes());
    assert_eq!(result.directive, Directive::Continue);
}

#[test]
fn modified_created_and_deleted_buffers_map_directly() {
    let mut changes = StateChanges::new();
    let modified = BufferId::from_raw(1);
    let created = BufferId::from_raw(2);
    let deleted = BufferId::from_raw(3);

    changes.record_buffer_modified(modified);
    changes.record_buffer_created(created);
    changes.record_buffer_deleted(deleted);

    let result = state_changes_to_dispatch_result(&changes);
    assert_eq!(result.buffers.modified, vec![modified]);
    assert_eq!(result.buffers.created, vec![created]);
    assert_eq!(result.buffers.closed, vec![deleted]);
}

#[test]
fn quit_signal_maps_to_quit_directive() {
    let mut changes = StateChanges::new();
    changes.record_quit_requested();
    let result = state_changes_to_dispatch_result(&changes);
    assert_eq!(result.directive, Directive::Quit);
}

#[test]
fn non_dispatch_flags_are_dropped() {
    let mut changes = StateChanges::new();
    let buffer = BufferId::from_raw(9);
    changes.record_cursor_move(buffer);
    changes.record_mode_change();
    changes.record_selection_change(buffer);
    changes.option_changed = true;
    changes.window_changed = true;
    changes.focus_changed = true;
    changes.scroll_changed = true;
    changes.presence_changed = true;
    changes.extension_changed = true;

    let result = state_changes_to_dispatch_result(&changes);
    assert!(!result.buffers.has_changes());
    assert_eq!(result.directive, Directive::Continue);
}

#[test]
fn text_domain_edit_details_are_dropped() {
    let mut changes = StateChanges::new();
    changes.buffer_modified = true;
    let result = state_changes_to_dispatch_result(&changes);
    assert!(result.buffers.modified.is_empty());
}

#[test]
fn command_result_wraps_dispatch_result() {
    let mut changes = StateChanges::new();
    let created = BufferId::from_raw(5);
    changes.record_buffer_created(created);

    let result = state_changes_to_command_result(&changes);
    match result {
        CommandResult::Handled(dispatch) => {
            assert_eq!(dispatch.buffers.created, vec![created]);
            assert_eq!(dispatch.directive, Directive::Continue);
        }
        CommandResult::NotHandled | CommandResult::Error(_) => {
            panic!("expected handled command result")
        }
    }
}
