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
    let ids: Vec<&str> = vec![
        TROUBLE_OPEN.name(),
        TROUBLE_CLOSE.name(),
        TROUBLE_TOGGLE.name(),
        TROUBLE_NEXT.name(),
        TROUBLE_PREV.name(),
        TROUBLE_SELECT.name(),
        TROUBLE_FILTER_ERROR.name(),
        TROUBLE_FILTER_WARNING.name(),
        TROUBLE_FILTER_ALL.name(),
        TROUBLE_SORT.name(),
        TROUBLE_REFRESH.name(),
    ];
    let count = ids.len();
    let mut unique = ids.clone();
    unique.sort_unstable();
    unique.dedup();
    assert_eq!(unique.len(), count, "Duplicate command IDs found");
}
