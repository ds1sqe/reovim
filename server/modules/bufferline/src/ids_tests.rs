use super::*;

#[test]
fn module_id() {
    assert_eq!(MODULE.as_str(), "bufferline");
}

#[test]
fn command_ids_belong_to_module() {
    let commands = [
        PIN_BUFFER,
        UNPIN_BUFFER,
        CLOSE_BUFFER,
        NEXT_BUFFER,
        PREV_BUFFER,
    ];
    for cmd in &commands {
        assert_eq!(*cmd.module(), MODULE);
    }
}

#[test]
fn command_ids_unique() {
    let ids: Vec<std::borrow::Cow<'static, str>> = vec![
        PIN_BUFFER.name_owned(),
        UNPIN_BUFFER.name_owned(),
        CLOSE_BUFFER.name_owned(),
        NEXT_BUFFER.name_owned(),
        PREV_BUFFER.name_owned(),
    ];
    let count = ids.len();
    let mut unique = ids.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), count, "Duplicate command IDs found");
}
