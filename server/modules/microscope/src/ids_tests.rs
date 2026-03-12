use super::*;

#[test]
fn module_id() {
    assert_eq!(MODULE.as_str(), "microscope");
}

#[test]
fn command_ids_have_correct_module() {
    let commands = [
        OPEN_FILES,
        OPEN_BUFFERS,
        OPEN_GREP,
        OPEN_COMMANDS,
        SELECT_ITEM,
        CLOSE,
        NEXT_ITEM,
        PREV_ITEM,
        BACKSPACE,
    ];
    for cmd in &commands {
        assert_eq!(cmd.module(), &MODULE);
    }
}

#[test]
fn command_ids_are_unique() {
    let names: Vec<&str> = vec![
        OPEN_FILES.name(),
        OPEN_BUFFERS.name(),
        OPEN_GREP.name(),
        OPEN_COMMANDS.name(),
        SELECT_ITEM.name(),
        CLOSE.name(),
        NEXT_ITEM.name(),
        PREV_ITEM.name(),
        BACKSPACE.name(),
    ];
    let mut deduped = names.clone();
    deduped.sort_unstable();
    deduped.dedup();
    assert_eq!(names.len(), deduped.len());
}
