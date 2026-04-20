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
    let names: Vec<std::borrow::Cow<'static, str>> = vec![
        OPEN_FILES.name_owned(),
        OPEN_BUFFERS.name_owned(),
        OPEN_GREP.name_owned(),
        OPEN_COMMANDS.name_owned(),
        SELECT_ITEM.name_owned(),
        CLOSE.name_owned(),
        NEXT_ITEM.name_owned(),
        PREV_ITEM.name_owned(),
        BACKSPACE.name_owned(),
    ];
    let mut deduped = names.clone();
    deduped.sort_unstable();
    deduped.dedup();
    assert_eq!(names.len(), deduped.len());
}
