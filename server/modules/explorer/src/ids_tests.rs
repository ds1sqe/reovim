use super::*;

#[test]
fn module_id() {
    assert_eq!(MODULE.as_str(), "explorer");
}

#[test]
fn command_ids_have_correct_module() {
    let commands = [
        TOGGLE,
        CLOSE,
        CURSOR_UP,
        CURSOR_DOWN,
        OPEN,
        EXPAND,
        COLLAPSE,
        GOTO_PARENT,
        GOTO_FIRST,
        GOTO_LAST,
        TOGGLE_HIDDEN,
        REFRESH,
        CREATE_FILE,
        CREATE_DIR,
        RENAME,
        DELETE,
        CONFIRM_INPUT,
        CANCEL_INPUT,
        INPUT_BACKSPACE,
        YANK_PATH,
        TOGGLE_GITIGNORED,
        CUT_MARK,
        PASTE,
    ];
    for cmd in &commands {
        assert_eq!(cmd.module(), &MODULE);
    }
}

#[test]
fn command_ids_are_unique() {
    let names: Vec<std::borrow::Cow<'static, str>> = vec![
        TOGGLE.name_owned(),
        CLOSE.name_owned(),
        CURSOR_UP.name_owned(),
        CURSOR_DOWN.name_owned(),
        OPEN.name_owned(),
        EXPAND.name_owned(),
        COLLAPSE.name_owned(),
        GOTO_PARENT.name_owned(),
        GOTO_FIRST.name_owned(),
        GOTO_LAST.name_owned(),
        TOGGLE_HIDDEN.name_owned(),
        REFRESH.name_owned(),
        CREATE_FILE.name_owned(),
        CREATE_DIR.name_owned(),
        RENAME.name_owned(),
        DELETE.name_owned(),
        CONFIRM_INPUT.name_owned(),
        CANCEL_INPUT.name_owned(),
        INPUT_BACKSPACE.name_owned(),
        YANK_PATH.name_owned(),
        TOGGLE_GITIGNORED.name_owned(),
        CUT_MARK.name_owned(),
        PASTE.name_owned(),
    ];
    let mut deduped = names.clone();
    deduped.sort_unstable();
    deduped.dedup();
    assert_eq!(names.len(), deduped.len());
}
