use super::*;

#[test]
fn test_module_id() {
    assert_eq!(MODULE.as_str(), "range-finder");
}

#[test]
fn test_jump_search_id() {
    assert_eq!(JUMP_SEARCH.module(), &MODULE);
    assert_eq!(JUMP_SEARCH.name(), "jump-search");
}

#[test]
fn test_jump_execute_id() {
    assert_eq!(JUMP_EXECUTE.module(), &MODULE);
    assert_eq!(JUMP_EXECUTE.name(), "jump-execute");
}

#[test]
fn test_start_find_char_jump_id() {
    assert_eq!(START_FIND_CHAR_JUMP.module(), &MODULE);
    assert_eq!(START_FIND_CHAR_JUMP.name(), "start-find-char-jump");
}

#[test]
fn test_command_ids_unique() {
    assert_ne!(JUMP_SEARCH, JUMP_EXECUTE);
    assert_ne!(JUMP_SEARCH, START_FIND_CHAR_JUMP);
    assert_ne!(JUMP_EXECUTE, START_FIND_CHAR_JUMP);
}

#[test]
fn test_command_ids_belong_to_module() {
    assert_eq!(JUMP_SEARCH.module(), &MODULE);
    assert_eq!(JUMP_EXECUTE.module(), &MODULE);
    assert_eq!(START_FIND_CHAR_JUMP.module(), &MODULE);
}

#[test]
fn test_jump_input_mode_id() {
    assert_eq!(JUMP_INPUT_MODE.module().as_str(), "range-finder");
    assert_eq!(JUMP_INPUT_MODE.name(), "jump-input");
    assert_eq!(JUMP_INPUT_MODE.discriminant(), 0);
}

#[test]
fn test_jump_input_mode_distinct_from_commands() {
    // Mode and command IDs are different types, but verify module matches
    assert_eq!(JUMP_INPUT_MODE.module(), JUMP_SEARCH.module());
}
