use super::*;

#[test]
fn test_module_id() {
    assert_eq!(MODULE.as_str(), "module-manager");
}

#[test]
fn test_command_ids_unique() {
    let ids = [OPEN, CLOSE, NEXT, PREV, TOGGLE_FILTER, TOGGLE_DETAIL];
    for (i, a) in ids.iter().enumerate() {
        for (j, b) in ids.iter().enumerate() {
            if i != j {
                assert_ne!(a, b, "Command IDs must be unique: {a:?} == {b:?}");
            }
        }
    }
}

#[test]
fn test_command_ids_have_correct_module() {
    let ids = [OPEN, CLOSE, NEXT, PREV, TOGGLE_FILTER, TOGGLE_DETAIL];
    for id in &ids {
        assert_eq!(*id.module(), MODULE);
    }
}
