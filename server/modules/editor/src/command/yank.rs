//! Yank commands.
//!
//! Provides yank (copy) commands:
//! - `YankLine` (yy, Y)

use {
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_driver_session::{SessionRuntime, api::BufferApi},
    reovim_kernel::api::v1::{CommandId, Position, RegisterContent},
};

use crate::ids;

/// Yank current line(s) to register (yy, Y).
#[derive(Debug, Clone, Copy, Default)]
pub struct YankLine;

impl Command for YankLine {
    fn id(&self) -> CommandId {
        ids::YANK_LINE
    }

    fn description(&self) -> &'static str {
        "Yank current line"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![
            ArgSpec::optional("count", ArgKind::Count, "Number of lines to yank"),
            ArgSpec::optional("register", ArgKind::Register, "Target register"),
        ]
    }
}

impl CommandHandler for YankLine {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        // Get cursor from per-client Window (#471)
        let Some(window) = runtime.windows().active() else {
            return CommandResult::error("No active window");
        };
        let pos = Position::new(window.cursor.line, window.cursor.column);
        let start_line = pos.line;

        // Get line count via BufferApi
        let Some(line_count) = runtime.buffer_line_count(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        // Can't yank from empty buffer
        if line_count == 0 {
            return CommandResult::Success;
        }

        let count = args.count().unwrap_or(1);

        // Collect lines to yank via BufferApi
        let end_line = (start_line + count).min(line_count);
        let mut yanked = String::new();
        for line_idx in start_line..end_line {
            if let Some(line) = runtime.buffer_line(buffer_id, line_idx) {
                yanked.push_str(&line);
                yanked.push('\n');
            }
        }

        // Store in register (use specified register or unnamed)
        let content = RegisterContent::linewise(yanked);
        let register = args.register();
        runtime
            .kernel()
            .registers
            .write()
            .set_by_name(register, content);

        CommandResult::Success
    }
}
