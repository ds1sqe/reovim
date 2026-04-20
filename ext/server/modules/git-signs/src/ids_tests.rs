use super::*;

#[test]
fn module_id() {
    assert_eq!(MODULE.as_str(), "git-signs");
}

#[test]
fn command_ids_belong_to_module() {
    let commands = [
        NEXT_HUNK,
        PREV_HUNK,
        STAGE_HUNK,
        RESET_HUNK,
        STAGE_BUFFER,
        RESET_BUFFER,
        UNSTAGE_FILE,
        PREVIEW_HUNK,
        DIFF_THIS,
    ];
    for cmd in &commands {
        assert_eq!(*cmd.module(), MODULE);
    }
}

#[test]
fn command_ids_unique() {
    let ids: Vec<std::borrow::Cow<'static, str>> = vec![
        NEXT_HUNK.name_owned(),
        PREV_HUNK.name_owned(),
        STAGE_HUNK.name_owned(),
        RESET_HUNK.name_owned(),
        STAGE_BUFFER.name_owned(),
        RESET_BUFFER.name_owned(),
        UNSTAGE_FILE.name_owned(),
        PREVIEW_HUNK.name_owned(),
        DIFF_THIS.name_owned(),
    ];
    let count = ids.len();
    let mut unique = ids.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), count, "Duplicate command IDs found");
}
