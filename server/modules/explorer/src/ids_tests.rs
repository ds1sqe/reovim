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
    let names: Vec<&str> = vec![
        TOGGLE.name(),
        CLOSE.name(),
        CURSOR_UP.name(),
        CURSOR_DOWN.name(),
        OPEN.name(),
        EXPAND.name(),
        COLLAPSE.name(),
        GOTO_PARENT.name(),
        GOTO_FIRST.name(),
        GOTO_LAST.name(),
        TOGGLE_HIDDEN.name(),
        REFRESH.name(),
        CREATE_FILE.name(),
        CREATE_DIR.name(),
        RENAME.name(),
        DELETE.name(),
        CONFIRM_INPUT.name(),
        CANCEL_INPUT.name(),
        INPUT_BACKSPACE.name(),
        YANK_PATH.name(),
        TOGGLE_GITIGNORED.name(),
        CUT_MARK.name(),
        PASTE.name(),
    ];
    let mut deduped = names.clone();
    deduped.sort_unstable();
    deduped.dedup();
    assert_eq!(names.len(), deduped.len());
}
