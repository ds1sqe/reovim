use reovim_driver_command::Command;

use super::*;

#[test]
fn trouble_open_metadata() {
    let cmd = TroubleOpen;
    assert_eq!(cmd.id(), ids::TROUBLE_OPEN);
    assert_eq!(cmd.description(), "Open diagnostics panel");
    assert_eq!(cmd.names(), &["Trouble"]);
    assert_eq!(cmd.args().len(), 1);
}

#[test]
fn trouble_close_metadata() {
    let cmd = TroubleClose;
    assert_eq!(cmd.id(), ids::TROUBLE_CLOSE);
    assert_eq!(cmd.description(), "Close diagnostics panel");
    assert_eq!(cmd.names(), &["TroubleClose"]);
}

#[test]
fn trouble_toggle_metadata() {
    let cmd = TroubleToggle;
    assert_eq!(cmd.id(), ids::TROUBLE_TOGGLE);
    assert_eq!(cmd.description(), "Toggle diagnostics panel");
}

#[test]
fn trouble_next_metadata() {
    let cmd = TroubleNext;
    assert_eq!(cmd.id(), ids::TROUBLE_NEXT);
    assert_eq!(cmd.description(), "Next diagnostic item");
}

#[test]
fn trouble_prev_metadata() {
    let cmd = TroublePrev;
    assert_eq!(cmd.id(), ids::TROUBLE_PREV);
    assert_eq!(cmd.description(), "Previous diagnostic item");
}

#[test]
fn trouble_select_metadata() {
    let cmd = TroubleSelect;
    assert_eq!(cmd.id(), ids::TROUBLE_SELECT);
    assert_eq!(cmd.description(), "Jump to selected diagnostic");
}

#[test]
fn trouble_filter_error_metadata() {
    let cmd = TroubleFilterError;
    assert_eq!(cmd.id(), ids::TROUBLE_FILTER_ERROR);
    assert_eq!(cmd.description(), "Show only errors");
}

#[test]
fn trouble_filter_warning_metadata() {
    let cmd = TroubleFilterWarning;
    assert_eq!(cmd.id(), ids::TROUBLE_FILTER_WARNING);
    assert_eq!(cmd.description(), "Show only warnings");
}

#[test]
fn trouble_filter_all_metadata() {
    let cmd = TroubleFilterAll;
    assert_eq!(cmd.id(), ids::TROUBLE_FILTER_ALL);
    assert_eq!(cmd.description(), "Show all severities");
}

#[test]
fn trouble_sort_metadata() {
    let cmd = TroubleSort;
    assert_eq!(cmd.id(), ids::TROUBLE_SORT);
    assert_eq!(cmd.description(), "Cycle sort order");
}

#[test]
fn trouble_refresh_metadata() {
    let cmd = TroubleRefresh;
    assert_eq!(cmd.id(), ids::TROUBLE_REFRESH);
    assert_eq!(cmd.description(), "Refresh diagnostics panel");
}

#[test]
fn command_handlers_count() {
    let handlers = command_handlers();
    assert_eq!(handlers.len(), 11);
}

#[test]
fn all_handlers_have_unique_ids() {
    let handlers = command_handlers();
    let ids: Vec<CommandId> = handlers.iter().map(|h| h.id()).collect();
    let count = ids.len();
    let mut names: Vec<&str> = ids.iter().map(CommandId::name).collect();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), count, "Duplicate handler IDs");
}
