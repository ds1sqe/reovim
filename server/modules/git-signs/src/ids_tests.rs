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
    let ids: Vec<&str> = vec![
        NEXT_HUNK.name(),
        PREV_HUNK.name(),
        STAGE_HUNK.name(),
        RESET_HUNK.name(),
        STAGE_BUFFER.name(),
        RESET_BUFFER.name(),
        UNSTAGE_FILE.name(),
        PREVIEW_HUNK.name(),
        DIFF_THIS.name(),
    ];
    let count = ids.len();
    let mut unique = ids.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), count, "Duplicate command IDs found");
}
