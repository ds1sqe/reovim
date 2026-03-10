//! Jump navigation commands.
//!
//! - `JumpSearchCommand` (`s` key) - start jump search, enter jump-input mode
//! - `JumpExecuteCommand` - cursor move after label resolution

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::{BufferApi, ExtensionApi, ModeApi, SessionRuntime, TransitionContext},
    reovim_kernel::api::v1::CommandId,
};

use super::{ids, search::Direction, state::JumpSessionState};

/// Command to start a jump search (`s` in normal mode).
///
/// Initialises the `JumpSessionState` state machine and transitions
/// to `range-finder:jump-input` mode.
#[derive(Debug, Clone, Copy, Default)]
pub struct JumpSearchCommand;

impl Command for JumpSearchCommand {
    fn id(&self) -> CommandId {
        ids::JUMP_SEARCH
    }

    fn description(&self) -> &'static str {
        "Start jump search (two-char pattern)"
    }
}

impl CommandHandler for JumpSearchCommand {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::Success;
        };

        // Read cursor position from active window (#524).
        // CommandContext.cursor_position() is not populated by the server framework,
        // so we read directly from the per-client window state (same pattern as motions).
        let Some(window) = runtime.windows().active() else {
            return CommandResult::Success;
        };
        #[allow(clippy::cast_possible_truncation)]
        let (line, col) = (window.cursor.line as u32, window.cursor.column as u32);

        // Gather buffer lines for jump search.
        let line_count = runtime.buffer_line_count(buffer_id).unwrap_or(0);
        let mut lines = Vec::with_capacity(line_count);
        for i in 0..line_count {
            lines.push(runtime.buffer_line(buffer_id, i).unwrap_or_default());
        }

        // Start the jump state machine.
        let jump = runtime.ext_mut::<JumpSessionState>();
        jump.start(lines, line, col, Direction::Both);

        // Enter jump-input mode for label selection.
        runtime.push_mode(ids::JUMP_INPUT_MODE, TransitionContext::new());

        CommandResult::Success
    }
}

/// Command to execute a resolved jump (cursor move).
///
/// Called after label selection completes. Takes the target from
/// `JumpSessionState::take_target()` and moves the cursor.
#[derive(Debug, Clone, Copy, Default)]
pub struct JumpExecuteCommand;

impl Command for JumpExecuteCommand {
    fn id(&self) -> CommandId {
        ids::JUMP_EXECUTE
    }

    fn description(&self) -> &'static str {
        "Execute jump to resolved target"
    }
}

impl CommandHandler for JumpExecuteCommand {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        let Some(target) = runtime.ext_mut::<JumpSessionState>().take_target() else {
            return CommandResult::Success;
        };

        // Move cursor to the resolved jump target.
        if let Some(window) = runtime.windows_mut().active_mut() {
            window.cursor.line = target.line as usize;
            window.cursor.column = target.col as usize;
        }

        CommandResult::Success
    }
}

/// Start a find-char jump with pre-computed match positions.
///
/// Called cross-module by `ExecuteFindChar` (vim module) when multiple
/// f/t matches exist on a line. Match positions are passed as JSON in
/// `CommandContext.string("find_char_matches")` to avoid type coupling.
///
/// JSON format: `[{"line":0,"col":5},{"line":0,"col":10},...]`
#[derive(Debug, Clone, Copy, Default)]
pub struct StartFindCharJumpCommand;

impl Command for StartFindCharJumpCommand {
    fn id(&self) -> CommandId {
        ids::START_FIND_CHAR_JUMP
    }

    fn description(&self) -> &'static str {
        "Start find-char jump with pre-computed match positions"
    }
}

impl CommandHandler for StartFindCharJumpCommand {
    #[allow(clippy::cast_possible_truncation)]
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(json_str) = args.string("find_char_matches") else {
            return CommandResult::error("find_char_matches argument required");
        };

        // Deserialize match positions from JSON.
        let Ok(positions) = serde_json::from_str::<Vec<MatchPosition>>(json_str) else {
            return CommandResult::error("invalid find_char_matches JSON");
        };

        if positions.len() < 2 {
            return CommandResult::error("need at least 2 matches for jump labels");
        }

        // Generate labels and build JumpMatch vector.
        let labels = super::search::generate_labels(positions.len());
        let matches: Vec<super::search::JumpMatch> = positions
            .into_iter()
            .zip(labels)
            .enumerate()
            .map(|(i, (pos, label))| {
                super::search::JumpMatch::new(pos.line, pos.col, label, i as u32)
            })
            .collect();

        // Activate the jump state machine with pre-computed matches.
        let jump = runtime.ext_mut::<JumpSessionState>();
        jump.start_with_matches(matches);

        // Enter jump-input mode for label selection.
        runtime.push_mode(ids::JUMP_INPUT_MODE, TransitionContext::new());

        CommandResult::Success
    }
}

/// Match position for cross-module serialization.
#[derive(serde::Deserialize)]
struct MatchPosition {
    line: u32,
    col: u32,
}

/// Return all jump command handlers.
#[must_use]
pub fn all_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(JumpSearchCommand),
        Box::new(JumpExecuteCommand),
        Box::new(StartFindCharJumpCommand),
    ]
}

#[cfg(test)]
mod tests {
    use reovim_driver_session::{TextInputSink, testing::TestSessionRuntime};

    use super::*;

    #[test]
    fn test_jump_search_command_id() {
        let cmd = JumpSearchCommand;
        assert_eq!(cmd.id(), ids::JUMP_SEARCH);
    }

    #[test]
    fn test_jump_search_command_description() {
        let cmd = JumpSearchCommand;
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn test_jump_execute_command_id() {
        let cmd = JumpExecuteCommand;
        assert_eq!(cmd.id(), ids::JUMP_EXECUTE);
    }

    #[test]
    fn test_jump_execute_command_description() {
        let cmd = JumpExecuteCommand;
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn test_start_find_char_jump_command_id() {
        let cmd = StartFindCharJumpCommand;
        assert_eq!(cmd.id(), ids::START_FIND_CHAR_JUMP);
    }

    #[test]
    fn test_start_find_char_jump_command_description() {
        let cmd = StartFindCharJumpCommand;
        assert!(!cmd.description().is_empty());
    }

    #[test]
    fn test_all_commands_count() {
        let commands = all_commands();
        assert_eq!(commands.len(), 3);
    }

    #[test]
    fn test_all_commands_unique_ids() {
        let commands = all_commands();
        let ids: Vec<CommandId> = commands.iter().map(|c| c.id()).collect();
        assert_ne!(ids[0], ids[1]);
        assert_ne!(ids[0], ids[2]);
        assert_ne!(ids[1], ids[2]);
    }

    #[test]
    fn test_jump_search_command_no_args() {
        let cmd = JumpSearchCommand;
        assert!(cmd.args().is_empty());
    }

    #[test]
    fn test_jump_search_command_no_names() {
        let cmd = JumpSearchCommand;
        assert!(cmd.names().is_empty());
    }

    #[test]
    fn test_jump_search_no_buffer() {
        let cmd = JumpSearchCommand;
        let mut harness = TestSessionRuntime::new();
        let args = CommandContext::new();
        let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
        assert!(matches!(result, CommandResult::Success));
    }

    #[test]
    fn test_jump_search_no_window() {
        let cmd = JumpSearchCommand;
        let mut harness = TestSessionRuntime::new();
        let mut args = CommandContext::new();
        args.set_buffer_id(reovim_kernel::api::v1::BufferId::from_raw(0));
        let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
        assert!(matches!(result, CommandResult::Success));
    }

    #[test]
    fn test_jump_search_starts_state_and_pushes_mode() {
        let cmd = JumpSearchCommand;
        let mut harness = TestSessionRuntime::with_buffer("hello world\nfoo bar");
        let buffer_id = harness.active_buffer().unwrap();
        let mut args = CommandContext::new();
        args.set_buffer_id(buffer_id);

        let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
        assert!(matches!(result, CommandResult::Success));

        // Verify jump state was started with buffer lines.
        harness.with_runtime(|rt| {
            let jump = rt.ext_mut::<JumpSessionState>();
            assert!(jump.is_active());
        });

        // Verify mode was pushed.
        assert!(harness.changes().mode_changed);
    }

    #[test]
    fn test_jump_execute_no_target() {
        let cmd = JumpExecuteCommand;
        let mut harness = TestSessionRuntime::with_buffer("hello world");
        let args = CommandContext::new();
        let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
        assert!(matches!(result, CommandResult::Success));
    }

    #[test]
    fn test_jump_execute_moves_cursor() {
        let cmd = JumpExecuteCommand;
        let mut harness = TestSessionRuntime::with_buffer("hello world\nfoo bar");
        let args = CommandContext::new();

        // Set up a jump state with a resolved target via "wo" search.
        harness.with_runtime(|rt| {
            let jump = rt.ext_mut::<JumpSessionState>();
            jump.start(vec!["hello world".into(), "foo bar".into()], 0, 0, Direction::Both);
            jump.insert_char('w');
            jump.insert_char('o');
            assert!(jump.has_target());
        });

        let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
        assert!(matches!(result, CommandResult::Success));

        // Verify cursor was moved.
        harness.with_runtime(|rt| {
            let window = rt.windows_mut().active_mut().expect("active window");
            assert_eq!(window.cursor.line, 0);
            assert_eq!(window.cursor.column, 6);
        });
    }

    #[test]
    fn test_jump_execute_no_window() {
        let cmd = JumpExecuteCommand;
        let mut harness = TestSessionRuntime::new();
        let args = CommandContext::new();

        // Set up a target without any window.
        harness.with_runtime(|rt| {
            let jump = rt.ext_mut::<JumpSessionState>();
            jump.start(vec!["hello world".into()], 0, 0, Direction::Both);
            jump.insert_char('w');
            jump.insert_char('o');
            assert!(jump.has_target());
        });

        let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
        assert!(matches!(result, CommandResult::Success));
    }

    // =========================================================================
    // StartFindCharJumpCommand execute tests
    // =========================================================================

    #[test]
    fn test_start_find_char_jump_no_args() {
        let cmd = StartFindCharJumpCommand;
        let mut harness = TestSessionRuntime::with_buffer("hello");
        let args = CommandContext::new();
        let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
        assert!(matches!(result, CommandResult::Error(_)));
    }

    #[test]
    fn test_start_find_char_jump_invalid_json() {
        let cmd = StartFindCharJumpCommand;
        let mut harness = TestSessionRuntime::with_buffer("hello");
        let mut args = CommandContext::new();
        args.set(
            "find_char_matches",
            reovim_driver_command::ArgValue::String("not json".to_string()),
        );
        let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
        assert!(matches!(result, CommandResult::Error(_)));
    }

    #[test]
    fn test_start_find_char_jump_too_few_matches() {
        let cmd = StartFindCharJumpCommand;
        let mut harness = TestSessionRuntime::with_buffer("hello");
        let mut args = CommandContext::new();
        args.set(
            "find_char_matches",
            reovim_driver_command::ArgValue::String(r#"[{"line":0,"col":5}]"#.to_string()),
        );
        let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
        assert!(matches!(result, CommandResult::Error(_)));
    }

    #[test]
    fn test_start_find_char_jump_activates_state() {
        let cmd = StartFindCharJumpCommand;
        let mut harness = TestSessionRuntime::with_buffer("aabaa");
        let mut args = CommandContext::new();
        args.set(
            "find_char_matches",
            reovim_driver_command::ArgValue::String(
                r#"[{"line":0,"col":0},{"line":0,"col":2},{"line":0,"col":4}]"#.to_string(),
            ),
        );

        let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
        assert!(matches!(result, CommandResult::Success));

        // Verify jump state was activated with 3 matches.
        harness.with_runtime(|rt| {
            let jump = rt.ext_mut::<JumpSessionState>();
            assert!(jump.is_active());
            let m = jump.get_matches().expect("should have matches");
            assert_eq!(m.len(), 3);
        });

        // Verify mode was pushed.
        assert!(harness.changes().mode_changed);
    }

    #[test]
    fn test_start_find_char_jump_label_selection_moves_cursor() {
        let cmd = StartFindCharJumpCommand;
        let mut harness = TestSessionRuntime::with_buffer("aabaa");
        let mut args = CommandContext::new();
        args.set(
            "find_char_matches",
            reovim_driver_command::ArgValue::String(
                r#"[{"line":0,"col":0},{"line":0,"col":2},{"line":0,"col":4}]"#.to_string(),
            ),
        );

        harness.with_runtime(|rt| cmd.execute(rt, &args));

        // Select second label ("f") to jump to col 2
        harness.with_runtime(|rt| {
            let jump = rt.ext_mut::<JumpSessionState>();
            jump.insert_char('f');
            assert!(!jump.is_active());
            assert!(jump.has_target());
        });

        // Execute the jump
        let exec_cmd = JumpExecuteCommand;
        let exec_args = CommandContext::new();
        harness.with_runtime(|rt| exec_cmd.execute(rt, &exec_args));

        // Verify cursor position
        harness.with_runtime(|rt| {
            let window = rt.windows_mut().active_mut().expect("active window");
            assert_eq!(window.cursor.column, 2);
        });
    }

    // =========================================================================
    // JumpExecuteCommand additional tests
    // =========================================================================

    #[test]
    fn test_jump_execute_consumes_target() {
        let cmd = JumpExecuteCommand;
        let mut harness = TestSessionRuntime::with_buffer("hello world");
        let args = CommandContext::new();

        // Set up a target.
        harness.with_runtime(|rt| {
            let jump = rt.ext_mut::<JumpSessionState>();
            jump.start(vec!["hello world".into()], 0, 0, Direction::Both);
            jump.insert_char('w');
            jump.insert_char('o');
            assert!(jump.has_target());
        });

        // Execute once.
        harness.with_runtime(|rt| cmd.execute(rt, &args));

        // Target should be consumed (take_target returns None).
        harness.with_runtime(|rt| {
            let jump = rt.ext_mut::<JumpSessionState>();
            assert!(!jump.has_target());
        });
    }
}
