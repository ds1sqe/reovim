use super::*;

#[test]
fn module_id() {
    assert_eq!(MODULE.as_str(), "completion");
}

#[test]
fn editor_command_ids() {
    // TRIGGER/NEXT/PREV re-use editor module IDs for keybinding compatibility.
    assert_eq!(TRIGGER.module(), &EDITOR);
    assert_eq!(NEXT.module(), &EDITOR);
    assert_eq!(PREV.module(), &EDITOR);
}

#[test]
fn completion_command_ids() {
    assert_eq!(CONFIRM.module(), &MODULE);
    assert_eq!(DISMISS.module(), &MODULE);
}

#[test]
fn command_ids_are_unique() {
    let names: Vec<std::borrow::Cow<'static, str>> = vec![
        TRIGGER.name_owned(),
        NEXT.name_owned(),
        PREV.name_owned(),
        CONFIRM.name_owned(),
        DISMISS.name_owned(),
    ];
    let mut deduped = names.clone();
    deduped.sort_unstable();
    deduped.dedup();
    assert_eq!(names.len(), deduped.len());
}
