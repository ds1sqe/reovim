use super::*;

#[test]
fn test_module_id() {
    assert_eq!(MODULE.as_str(), "snippet");
}

#[test]
fn test_navigating_mode() {
    assert_eq!(NAVIGATING_MODE.module().as_str(), "snippet");
    assert_eq!(NAVIGATING_MODE.name(), "navigating");
    assert_eq!(NAVIGATING_MODE.discriminant(), 0);
}

#[test]
fn test_command_ids() {
    assert_eq!(EXPAND.module().as_str(), "snippet");
    assert_eq!(EXPAND.name(), "expand");

    assert_eq!(JUMP_NEXT.module().as_str(), "snippet");
    assert_eq!(JUMP_NEXT.name(), "jump-next");

    assert_eq!(JUMP_PREV.module().as_str(), "snippet");
    assert_eq!(JUMP_PREV.name(), "jump-prev");

    assert_eq!(CANCEL.module().as_str(), "snippet");
    assert_eq!(CANCEL.name(), "cancel");

    assert_eq!(CATALOG.module().as_str(), "snippet");
    assert_eq!(CATALOG.name(), "catalog");

    assert_eq!(RELOAD.module().as_str(), "snippet");
    assert_eq!(RELOAD.name(), "reload");
}

#[test]
fn test_command_ids_are_distinct() {
    let commands = [&EXPAND, &JUMP_NEXT, &JUMP_PREV, &CANCEL, &CATALOG, &RELOAD];
    for (i, a) in commands.iter().enumerate() {
        for (j, b) in commands.iter().enumerate() {
            if i != j {
                assert_ne!(a, b, "commands {i} and {j} should differ");
            }
        }
    }
}
