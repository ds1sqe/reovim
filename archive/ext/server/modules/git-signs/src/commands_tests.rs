use {reovim_driver_command::Command, reovim_kernel::api::v1::CommandId};

use super::*;

#[test]
fn next_hunk_metadata() {
    let cmd = NextHunk;
    assert_eq!(cmd.id(), ids::NEXT_HUNK);
    assert_eq!(cmd.description(), "Next git hunk");
}

#[test]
fn prev_hunk_metadata() {
    let cmd = PrevHunk;
    assert_eq!(cmd.id(), ids::PREV_HUNK);
    assert_eq!(cmd.description(), "Previous git hunk");
}

#[test]
fn stage_hunk_metadata() {
    let cmd = StageHunk;
    assert_eq!(cmd.id(), ids::STAGE_HUNK);
    assert_eq!(cmd.description(), "Stage git hunk");
}

#[test]
fn reset_hunk_metadata() {
    let cmd = ResetHunk;
    assert_eq!(cmd.id(), ids::RESET_HUNK);
    assert_eq!(cmd.description(), "Reset git hunk");
}

#[test]
fn stage_buffer_metadata() {
    let cmd = StageBuffer;
    assert_eq!(cmd.id(), ids::STAGE_BUFFER);
    assert_eq!(cmd.description(), "Stage entire buffer");
}

#[test]
fn reset_buffer_metadata() {
    let cmd = ResetBuffer;
    assert_eq!(cmd.id(), ids::RESET_BUFFER);
    assert_eq!(cmd.description(), "Reset entire buffer");
}

#[test]
fn unstage_file_metadata() {
    let cmd = UnstageFile;
    assert_eq!(cmd.id(), ids::UNSTAGE_FILE);
    assert_eq!(cmd.description(), "Unstage file");
}

#[test]
fn preview_hunk_metadata() {
    let cmd = PreviewHunk;
    assert_eq!(cmd.id(), ids::PREVIEW_HUNK);
    assert_eq!(cmd.description(), "Preview hunk diff");
}

#[test]
fn diff_this_metadata() {
    let cmd = DiffThis;
    assert_eq!(cmd.id(), ids::DIFF_THIS);
    assert_eq!(cmd.description(), "Show file diff");
}

#[test]
fn command_handlers_count() {
    let handlers = command_handlers();
    assert_eq!(handlers.len(), 9);
}

#[test]
fn all_handlers_unique_ids() {
    let handlers = command_handlers();
    let ids: Vec<CommandId> = handlers.iter().map(|h| h.id()).collect();
    let mut names: Vec<&str> = ids.iter().map(CommandId::name).collect();
    let count = names.len();
    names.sort_unstable();
    names.dedup();
    assert_eq!(names.len(), count, "Duplicate handler IDs");
}
