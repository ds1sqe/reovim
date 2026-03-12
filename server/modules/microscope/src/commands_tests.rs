use {super::*, reovim_driver_command::Command};

#[test]
fn open_files_metadata() {
    let cmd = OpenFiles;
    assert_eq!(cmd.id(), ids::OPEN_FILES);
    assert!(!cmd.description().is_empty());
}

#[test]
fn open_buffers_metadata() {
    let cmd = OpenBuffers;
    assert_eq!(cmd.id(), ids::OPEN_BUFFERS);
    assert!(!cmd.description().is_empty());
}

#[test]
fn open_grep_metadata() {
    let cmd = OpenGrep;
    assert_eq!(cmd.id(), ids::OPEN_GREP);
    assert!(!cmd.description().is_empty());
}

#[test]
fn open_commands_metadata() {
    let cmd = OpenCommands;
    assert_eq!(cmd.id(), ids::OPEN_COMMANDS);
    assert!(!cmd.description().is_empty());
}

#[test]
fn next_item_metadata() {
    let cmd = NextItem;
    assert_eq!(cmd.id(), ids::NEXT_ITEM);
    assert!(!cmd.description().is_empty());
}

#[test]
fn prev_item_metadata() {
    let cmd = PrevItem;
    assert_eq!(cmd.id(), ids::PREV_ITEM);
    assert!(!cmd.description().is_empty());
}

#[test]
fn select_item_metadata() {
    let cmd = SelectItem;
    assert_eq!(cmd.id(), ids::SELECT_ITEM);
    assert!(!cmd.description().is_empty());
}

#[test]
fn close_metadata() {
    let cmd = Close;
    assert_eq!(cmd.id(), ids::CLOSE);
    assert!(!cmd.description().is_empty());
}

#[test]
fn backspace_metadata() {
    let cmd = Backspace;
    assert_eq!(cmd.id(), ids::BACKSPACE);
    assert!(!cmd.description().is_empty());
}

#[test]
fn command_handlers_not_empty() {
    let handlers = command_handlers();
    assert_eq!(handlers.len(), 9);
}

#[test]
fn command_handlers_unique_ids() {
    let handlers = command_handlers();
    let ids: Vec<CommandId> = handlers.iter().map(|h| h.id()).collect();
    let mut deduped = ids.clone();
    deduped.sort_by_key(CommandId::name);
    deduped.dedup_by_key(|id| id.name());
    assert_eq!(ids.len(), deduped.len());
}
