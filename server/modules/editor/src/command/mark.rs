//! Mark commands.
//!
//! Provides mark operations:
//! - `SetMark` (m{char}) - set a mark at cursor position
//! - `GotoMarkLine` ('{char}) - jump to mark line (column 0)
//! - `GotoMarkExact` (`{char}) - jump to mark exact position

use {
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_driver_session::{SessionRuntime, api::BufferApi},
    reovim_kernel::api::v1::{CommandId, Mark, Position},
};

use crate::ids;

/// Set mark at current cursor position (m{char}).
///
/// - Lowercase marks (a-z) are buffer-local (stored in per-client `MarkBank`)
/// - Uppercase marks (A-Z) are global (stored in kernel `global_marks`)
#[derive(Debug, Clone, Copy, Default)]
pub struct SetMark;

impl Command for SetMark {
    fn id(&self) -> CommandId {
        ids::SET_MARK
    }

    fn description(&self) -> &'static str {
        "Set mark at cursor position"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::required(
            "mark_char",
            ArgKind::Char,
            "Mark character (a-z local, A-Z global)",
        )]
    }
}

impl CommandHandler for SetMark {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(mark_char) = args.char("mark_char") else {
            return CommandResult::error("No mark character");
        };

        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let Some(window) = runtime.windows().active() else {
            return CommandResult::error("No active window");
        };
        let pos = Position::new(window.cursor.line, window.cursor.column);

        if mark_char.is_ascii_lowercase() {
            runtime.local_marks_mut().set_local(mark_char, pos);
            CommandResult::Success
        } else if mark_char.is_ascii_uppercase() {
            let mark = Mark::new(pos, buffer_id);
            runtime
                .kernel()
                .global_marks
                .write()
                .set_global(mark_char, mark);
            CommandResult::Success
        } else {
            CommandResult::error("Invalid mark character")
        }
    }
}

/// Go to mark line ('{char}).
///
/// Jumps to the mark's line with cursor at column 0 (Vim `'` behavior).
#[derive(Debug, Clone, Copy, Default)]
pub struct GotoMarkLine;

impl Command for GotoMarkLine {
    fn id(&self) -> CommandId {
        ids::GOTO_MARK_LINE
    }

    fn description(&self) -> &'static str {
        "Go to mark line"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::required(
            "mark_char",
            ArgKind::Char,
            "Mark character",
        )]
    }
}

impl CommandHandler for GotoMarkLine {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(mark_char) = args.char("mark_char") else {
            return CommandResult::error("No mark character");
        };

        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        goto_mark(runtime, mark_char, buffer_id, true)
    }
}

/// Go to mark exact position (`{char}).
///
/// Jumps to the mark's exact line and column (Vim `` ` `` behavior).
#[derive(Debug, Clone, Copy, Default)]
pub struct GotoMarkExact;

impl Command for GotoMarkExact {
    fn id(&self) -> CommandId {
        ids::GOTO_MARK_EXACT
    }

    fn description(&self) -> &'static str {
        "Go to mark exact position"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::required(
            "mark_char",
            ArgKind::Char,
            "Mark character",
        )]
    }
}

impl CommandHandler for GotoMarkExact {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(mark_char) = args.char("mark_char") else {
            return CommandResult::error("No mark character");
        };

        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        goto_mark(runtime, mark_char, buffer_id, false)
    }
}

/// Shared logic for mark goto commands.
///
/// `line_only`: if true, jump to column 0 (like `'`); if false, exact position (like `` ` ``).
fn goto_mark(
    runtime: &mut SessionRuntime<'_>,
    mark_char: char,
    buffer_id: reovim_kernel::api::v1::BufferId,
    line_only: bool,
) -> CommandResult {
    // Look up the mark
    let mark_result = if mark_char.is_ascii_lowercase() {
        runtime
            .local_marks()
            .get_local(mark_char)
            .map(|pos| (pos, buffer_id))
    } else if mark_char.is_ascii_uppercase() {
        runtime
            .kernel()
            .global_marks
            .read()
            .get_global(mark_char)
            .map(|m| (m.position, m.buffer_id))
    } else {
        // Special marks (' ` . ^ < >)
        runtime
            .local_marks()
            .get_by_char(mark_char)
            .map(|mr| (mr.position(), mr.buffer_id().unwrap_or(buffer_id)))
    };

    let Some((mark_pos, mark_buffer)) = mark_result else {
        return CommandResult::error("Mark not set");
    };

    // If mark is in a different buffer, switch active buffer
    if mark_buffer != buffer_id {
        runtime.set_active_buffer(Some(mark_buffer));
        if let Some(window) = runtime.windows_mut().active_mut() {
            window.buffer_id = Some(mark_buffer);
        }
    }

    // Compute target position
    let target = if line_only {
        Position::new(mark_pos.line, 0)
    } else {
        mark_pos
    };

    // Move cursor
    if let Some(window) = runtime.windows_mut().active_mut() {
        window.cursor.line = target.line;
        window.cursor.column = target.column;
    }

    CommandResult::Success
}
