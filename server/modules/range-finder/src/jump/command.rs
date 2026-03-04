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

/// Return all jump command handlers.
#[must_use]
pub fn all_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![Box::new(JumpSearchCommand), Box::new(JumpExecuteCommand)]
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
    fn test_all_commands_count() {
        let commands = all_commands();
        assert_eq!(commands.len(), 2);
    }

    #[test]
    fn test_all_commands_unique_ids() {
        let commands = all_commands();
        let ids: Vec<CommandId> = commands.iter().map(|c| c.id()).collect();
        assert_ne!(ids[0], ids[1]);
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
        let mut harness = TestSessionRuntime::with_buffer("hello world");
        let mut args = CommandContext::new();
        args.set_buffer_id(reovim_kernel::api::v1::BufferId::from_raw(0));

        let result = harness.with_runtime(|rt| cmd.execute(rt, &args));
        assert!(matches!(result, CommandResult::Success));

        // Verify jump state was started.
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
            if let Some(window) = rt.windows_mut().active_mut() {
                assert_eq!(window.cursor.line, 0);
                assert_eq!(window.cursor.column, 6);
            }
        });
    }

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
