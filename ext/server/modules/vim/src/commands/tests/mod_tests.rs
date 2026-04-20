use super::super::*;

#[test]
fn test_mode_commands_count() {
    let cmds = mode_commands();
    assert_eq!(cmds.len(), 32);
}

#[test]
fn test_mode_commands_not_empty() {
    let cmds = mode_commands();
    assert!(!cmds.is_empty());
}

#[test]
fn test_mode_commands_all_have_unique_ids() {
    use std::collections::HashSet;
    let cmds = mode_commands();
    let mut ids = HashSet::new();
    for cmd in &cmds {
        let id = cmd.id();
        assert!(ids.insert(id.clone()), "Duplicate command ID: {id:?}");
    }
}

#[test]
fn test_mode_commands_all_have_descriptions() {
    let cmds = mode_commands();
    for cmd in &cmds {
        let desc = cmd.description();
        assert!(!desc.is_empty(), "Empty description for {:?}", cmd.id());
    }
}

#[test]
fn test_mode_commands_contains_enter_insert() {
    use crate::ids;
    let cmds = mode_commands();
    let has_enter_insert = cmds.iter().any(|c| c.id() == ids::ENTER_INSERT);
    assert!(has_enter_insert);
}

#[test]
fn test_mode_commands_contains_exit_insert() {
    use crate::ids;
    let cmds = mode_commands();
    let has_exit_insert = cmds.iter().any(|c| c.id() == ids::EXIT_INSERT);
    assert!(has_exit_insert);
}

#[test]
fn test_mode_commands_contains_dot_repeat() {
    use crate::ids;
    let cmds = mode_commands();
    let has_dot_repeat = cmds.iter().any(|c| c.id() == ids::DOT_REPEAT);
    assert!(has_dot_repeat);
}

#[test]
fn test_mode_commands_contains_enter_commandline() {
    use crate::ids;
    let cmds = mode_commands();
    let has_enter_cmdline = cmds.iter().any(|c| c.id() == ids::ENTER_COMMANDLINE);
    assert!(has_enter_cmdline);
}

#[test]
fn test_mode_commands_contains_execute_find_char() {
    use crate::ids;
    let cmds = mode_commands();
    let has_find_char = cmds.iter().any(|c| c.id() == ids::EXECUTE_FIND_CHAR);
    assert!(has_find_char);
}

#[test]
fn test_mode_commands_contains_change_line() {
    use crate::ids;
    let cmds = mode_commands();
    let has_change_line = cmds.iter().any(|c| c.id() == ids::CHANGE_LINE);
    assert!(has_change_line);
}

#[test]
fn test_mode_commands_contains_change_to_eol() {
    use crate::ids;
    let cmds = mode_commands();
    let has_change_to_eol = cmds.iter().any(|c| c.id() == ids::CHANGE_TO_EOL);
    assert!(has_change_to_eol);
}

#[test]
fn test_mode_commands_contains_open_line_below() {
    use crate::ids;
    let cmds = mode_commands();
    let has_open_line_below = cmds.iter().any(|c| c.id() == ids::OPEN_LINE_BELOW);
    assert!(has_open_line_below);
}

#[test]
fn test_mode_commands_contains_open_line_above() {
    use crate::ids;
    let cmds = mode_commands();
    let has_open_line_above = cmds.iter().any(|c| c.id() == ids::OPEN_LINE_ABOVE);
    assert!(has_open_line_above);
}

#[test]
fn test_mode_commands_contains_search_forward() {
    use crate::ids;
    let cmds = mode_commands();
    let has_search_forward = cmds.iter().any(|c| c.id() == ids::ENTER_SEARCH_FORWARD);
    assert!(has_search_forward);
}

#[test]
fn test_mode_commands_contains_search_backward() {
    use crate::ids;
    let cmds = mode_commands();
    let has_search_backward = cmds.iter().any(|c| c.id() == ids::ENTER_SEARCH_BACKWARD);
    assert!(has_search_backward);
}

#[test]
fn test_mode_commands_contains_enter_window_mode() {
    use crate::ids;
    let cmds = mode_commands();
    let has_enter_window = cmds.iter().any(|c| c.id() == ids::ENTER_WINDOW_MODE);
    assert!(has_enter_window);
}

#[test]
fn test_mode_commands_contains_cancel_to_normal() {
    use crate::ids;
    let cmds = mode_commands();
    let has_cancel = cmds.iter().any(|c| c.id() == ids::CANCEL_TO_NORMAL);
    assert!(has_cancel);
}

#[test]
fn test_mode_commands_contains_enter_replace_mode() {
    use crate::ids;
    let cmds = mode_commands();
    let has_enter_replace = cmds.iter().any(|c| c.id() == ids::ENTER_REPLACE_MODE);
    assert!(has_enter_replace);
}

#[test]
fn test_mode_commands_contains_replace_backspace() {
    use crate::ids;
    let cmds = mode_commands();
    let has_replace_bs = cmds.iter().any(|c| c.id() == ids::REPLACE_BACKSPACE);
    assert!(has_replace_bs);
}
