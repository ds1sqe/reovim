//! Jump navigation commands.
//!
//! - `JumpSearchCommand` (`s` key) - start jump search, enter jump-input mode
//! - `JumpExecuteCommand` - cursor move after label resolution

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_text_session::{
        BufferApi, ExtensionApi, ModeApi, SessionRuntime, TransitionContext,
    },
    reovim_kernel::api::v1::CommandId,
};

use super::{ids, search::Direction, state::JumpSessionState};

/// Gather visible buffer lines and cursor context for a viewport-bounded jump.
///
/// Returns `(lines, relative_cursor_line, cursor_col, scroll_top)`.
/// Returns `None` if there is no active window or buffer.
#[allow(clippy::cast_possible_truncation)]
fn gather_viewport_context(
    runtime: &SessionRuntime<'_>,
    buffer_id: reovim_kernel::api::v1::BufferId,
) -> Option<(Vec<String>, u32, u32, u32)> {
    let window = runtime.windows().active()?;
    let cursor_line = window.cursor.line;
    let cursor_col = window.cursor.column as u32;
    let scroll_top = window.viewport.scroll_top;
    let viewport_height = usize::from(window.viewport.height);

    let line_count = runtime.buffer_line_count(buffer_id).unwrap_or(0);
    let start = scroll_top.min(line_count);
    let end = (scroll_top + viewport_height).min(line_count);
    let mut lines = Vec::with_capacity(end - start);
    for i in start..end {
        lines.push(runtime.buffer_line(buffer_id, i).unwrap_or_default());
    }

    let relative_cursor = cursor_line.saturating_sub(scroll_top) as u32;
    Some((lines, relative_cursor, cursor_col, start as u32))
}

/// Command to start a forward jump search (`s` in normal mode).
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
        "Start forward jump search (two-char pattern)"
    }
}

impl CommandHandler for JumpSearchCommand {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::Success;
        };

        let Some((lines, cursor, col, offset)) = gather_viewport_context(runtime, buffer_id) else {
            return CommandResult::Success;
        };

        let jump = runtime.ext_mut::<JumpSessionState>();
        jump.start(lines, cursor, col, Direction::Forward, offset);
        runtime.push_mode(ids::JUMP_INPUT_MODE, TransitionContext::new());
        CommandResult::Success
    }
}

/// Command to start a backward jump search (`S` in normal mode).
///
/// Identical to `JumpSearchCommand` except uses `Direction::Backward`.
#[derive(Debug, Clone, Copy, Default)]
pub struct JumpSearchBackwardCommand;

impl Command for JumpSearchBackwardCommand {
    fn id(&self) -> CommandId {
        ids::JUMP_SEARCH_BACKWARD
    }

    fn description(&self) -> &'static str {
        "Start backward jump search (two-char pattern)"
    }
}

impl CommandHandler for JumpSearchBackwardCommand {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::Success;
        };

        let Some((lines, cursor, col, offset)) = gather_viewport_context(runtime, buffer_id) else {
            return CommandResult::Success;
        };

        let jump = runtime.ext_mut::<JumpSessionState>();
        jump.start(lines, cursor, col, Direction::Backward, offset);
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
        Box::new(JumpSearchBackwardCommand),
        Box::new(JumpExecuteCommand),
        Box::new(StartFindCharJumpCommand),
    ]
}

#[cfg(test)]
#[path = "command_tests.rs"]
mod tests;
