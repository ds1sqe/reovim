use super::*;

#[test]
fn module_id() {
    assert_eq!(MODULE.as_str(), "diagnostics-panel");
}

#[test]
fn command_ids_belong_to_module() {
    let commands = [
        TROUBLE_OPEN,
        TROUBLE_CLOSE,
        TROUBLE_TOGGLE,
        TROUBLE_NEXT,
        TROUBLE_PREV,
        TROUBLE_SELECT,
        TROUBLE_FILTER_ERROR,
        TROUBLE_FILTER_WARNING,
        TROUBLE_FILTER_ALL,
        TROUBLE_SORT,
        TROUBLE_REFRESH,
    ];
    for cmd in &commands {
        assert_eq!(*cmd.module(), MODULE);
    }
}

#[test]
fn command_ids_unique() {
    let ids: Vec<std::borrow::Cow<'static, str>> = vec![
        TROUBLE_OPEN.name_owned(),
        TROUBLE_CLOSE.name_owned(),
        TROUBLE_TOGGLE.name_owned(),
        TROUBLE_NEXT.name_owned(),
        TROUBLE_PREV.name_owned(),
        TROUBLE_SELECT.name_owned(),
        TROUBLE_FILTER_ERROR.name_owned(),
        TROUBLE_FILTER_WARNING.name_owned(),
        TROUBLE_FILTER_ALL.name_owned(),
        TROUBLE_SORT.name_owned(),
        TROUBLE_REFRESH.name_owned(),
    ];
    let count = ids.len();
    let mut unique = ids.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), count, "Duplicate command IDs found");
}
