use {
    super::*,
    crate::state::{HighlightKind, HighlightRange, IlluminateState},
    reovim_driver_command::{Command, CommandHandler},
    reovim_driver_command_types::{CommandContext, CommandResult},
    reovim_driver_session::testing::TestSessionRuntime,
};

// ========================================================================
// Command trait: id, description, all_commands
// ========================================================================

#[test]
fn test_next_reference_id() {
    let cmd = NextReferenceCommand;
    assert_eq!(cmd.id(), ids::NEXT_REFERENCE);
}

#[test]
fn test_next_reference_description() {
    let cmd = NextReferenceCommand;
    assert!(!cmd.description().is_empty());
}

#[test]
fn test_prev_reference_id() {
    let cmd = PrevReferenceCommand;
    assert_eq!(cmd.id(), ids::PREV_REFERENCE);
}

#[test]
fn test_prev_reference_description() {
    let cmd = PrevReferenceCommand;
    assert!(!cmd.description().is_empty());
}

#[test]
fn test_all_commands_count() {
    let cmds = all_commands();
    assert_eq!(cmds.len(), 2);
}

#[test]
fn test_all_commands_unique_ids() {
    let cmds = all_commands();
    assert_ne!(cmds[0].id(), cmds[1].id());
}

// ========================================================================
// NextReferenceCommand::execute
// ========================================================================

fn make_test_ranges() -> Vec<HighlightRange> {
    vec![
        HighlightRange {
            start_line: 0,
            start_col: 0,
            end_line: 0,
            end_col: 5,
            kind: HighlightKind::Text,
        },
        HighlightRange {
            start_line: 2,
            start_col: 10,
            end_line: 2,
            end_col: 15,
            kind: HighlightKind::Read,
        },
        HighlightRange {
            start_line: 4,
            start_col: 3,
            end_line: 4,
            end_col: 8,
            kind: HighlightKind::Write,
        },
    ]
}

#[test]
fn test_next_execute_no_state() {
    let mut test = TestSessionRuntime::with_buffer("hello world");
    let cmd = NextReferenceCommand;
    let ctx = CommandContext::new();

    let result = test.with_runtime(|rt| cmd.execute(rt, &ctx));
    assert_eq!(result, CommandResult::Success);
    test.assert_cursor(0, 0);
}

#[test]
fn test_next_execute_inactive_state() {
    let mut test = TestSessionRuntime::with_buffer("hello world");
    let _ = test.extensions.get_or_insert::<IlluminateState>();
    let cmd = NextReferenceCommand;
    let ctx = CommandContext::new();

    let result = test.with_runtime(|rt| cmd.execute(rt, &ctx));
    assert_eq!(result, CommandResult::Success);
    test.assert_cursor(0, 0);
}

#[test]
fn test_next_execute_active_empty_ranges() {
    let mut test = TestSessionRuntime::with_buffer("hello world");
    let state = test.extensions.get_or_insert::<IlluminateState>();
    state.active = true;
    let cmd = NextReferenceCommand;
    let ctx = CommandContext::new();

    let result = test.with_runtime(|rt| cmd.execute(rt, &ctx));
    assert_eq!(result, CommandResult::Success);
    test.assert_cursor(0, 0);
}

#[test]
fn test_next_execute_navigates_to_next_range() {
    let mut test = TestSessionRuntime::with_buffer("hello\nworld\nhello world\nfoo\nhello bar");
    let buffer_id = test.active_buffer().unwrap();
    let state = test.extensions.get_or_insert::<IlluminateState>();
    state.set_highlights(buffer_id, "hello".to_string(), make_test_ranges(), 0, 0);
    let cmd = NextReferenceCommand;
    let ctx = CommandContext::new();

    // Cursor at (0, 0), which is AT range[0]. next_range_index returns range[1] at (2, 10).
    let result = test.with_runtime(|rt| cmd.execute(rt, &ctx));
    assert_eq!(result, CommandResult::Success);
    test.assert_cursor(2, 10);
}

#[test]
fn test_next_execute_wraps_around() {
    let mut test = TestSessionRuntime::with_buffer("hello\nworld\nhello world\nfoo\nhello bar");
    let buffer_id = test.active_buffer().unwrap();
    let state = test.extensions.get_or_insert::<IlluminateState>();
    state.set_highlights(buffer_id, "hello".to_string(), make_test_ranges(), 0, 0);

    // Set cursor past last range
    if let Some(w) = test.windows.active_mut() {
        w.cursor.line = 4;
        w.cursor.column = 5;
    }

    let cmd = NextReferenceCommand;
    let ctx = CommandContext::new();

    // Past range[2] at (4, 3), should wrap to range[0] at (0, 0)
    let result = test.with_runtime(|rt| cmd.execute(rt, &ctx));
    assert_eq!(result, CommandResult::Success);
    test.assert_cursor(0, 0);
}

#[test]
fn test_next_execute_no_cursor() {
    // No window → no cursor → early return
    let mut test = TestSessionRuntime::new();
    let state = test.extensions.get_or_insert::<IlluminateState>();
    state.set_highlights(
        reovim_kernel::api::v1::BufferId::from_raw(1),
        "x".to_string(),
        vec![HighlightRange {
            start_line: 0,
            start_col: 0,
            end_line: 0,
            end_col: 1,
            kind: HighlightKind::Text,
        }],
        0,
        0,
    );
    let cmd = NextReferenceCommand;
    let ctx = CommandContext::new();

    let result = test.with_runtime(|rt| cmd.execute(rt, &ctx));
    assert_eq!(result, CommandResult::Success);
}

// ========================================================================
// PrevReferenceCommand::execute
// ========================================================================

#[test]
fn test_prev_execute_active_empty_ranges() {
    let mut test = TestSessionRuntime::with_buffer("hello world");
    let state = test.extensions.get_or_insert::<IlluminateState>();
    state.active = true;
    let cmd = PrevReferenceCommand;
    let ctx = CommandContext::new();

    let result = test.with_runtime(|rt| cmd.execute(rt, &ctx));
    assert_eq!(result, CommandResult::Success);
    test.assert_cursor(0, 0);
}

#[test]
fn test_prev_execute_no_state() {
    let mut test = TestSessionRuntime::with_buffer("hello world");
    let cmd = PrevReferenceCommand;
    let ctx = CommandContext::new();

    let result = test.with_runtime(|rt| cmd.execute(rt, &ctx));
    assert_eq!(result, CommandResult::Success);
    test.assert_cursor(0, 0);
}

#[test]
fn test_prev_execute_inactive_state() {
    let mut test = TestSessionRuntime::with_buffer("hello world");
    let _ = test.extensions.get_or_insert::<IlluminateState>();
    let cmd = PrevReferenceCommand;
    let ctx = CommandContext::new();

    let result = test.with_runtime(|rt| cmd.execute(rt, &ctx));
    assert_eq!(result, CommandResult::Success);
    test.assert_cursor(0, 0);
}

#[test]
fn test_prev_execute_navigates_to_prev_range() {
    let mut test = TestSessionRuntime::with_buffer("hello\nworld\nhello world\nfoo\nhello bar");
    let buffer_id = test.active_buffer().unwrap();
    let state = test.extensions.get_or_insert::<IlluminateState>();
    state.set_highlights(buffer_id, "hello".to_string(), make_test_ranges(), 0, 0);

    // Set cursor at (3, 0) — between range[1](2,10) and range[2](4,3)
    if let Some(w) = test.windows.active_mut() {
        w.cursor.line = 3;
        w.cursor.column = 0;
    }

    let cmd = PrevReferenceCommand;
    let ctx = CommandContext::new();

    let result = test.with_runtime(|rt| cmd.execute(rt, &ctx));
    assert_eq!(result, CommandResult::Success);
    test.assert_cursor(2, 10);
}

#[test]
fn test_prev_execute_wraps_around() {
    let mut test = TestSessionRuntime::with_buffer("hello\nworld\nhello world\nfoo\nhello bar");
    let buffer_id = test.active_buffer().unwrap();
    let state = test.extensions.get_or_insert::<IlluminateState>();
    state.set_highlights(buffer_id, "hello".to_string(), make_test_ranges(), 0, 0);
    // Cursor at (0, 0), which is AT range[0]. prev wraps to last range[2] at (4, 3).
    let cmd = PrevReferenceCommand;
    let ctx = CommandContext::new();

    let result = test.with_runtime(|rt| cmd.execute(rt, &ctx));
    assert_eq!(result, CommandResult::Success);
    test.assert_cursor(4, 3);
}

#[test]
fn test_prev_execute_no_cursor() {
    let mut test = TestSessionRuntime::new();
    let state = test.extensions.get_or_insert::<IlluminateState>();
    state.set_highlights(
        reovim_kernel::api::v1::BufferId::from_raw(1),
        "x".to_string(),
        vec![HighlightRange {
            start_line: 0,
            start_col: 0,
            end_line: 0,
            end_col: 1,
            kind: HighlightKind::Text,
        }],
        0,
        0,
    );
    let cmd = PrevReferenceCommand;
    let ctx = CommandContext::new();

    let result = test.with_runtime(|rt| cmd.execute(rt, &ctx));
    assert_eq!(result, CommandResult::Success);
}
