//! Yank commands.
//!
//! Provides yank (copy) commands:
//! - `YankLine` (yy, Y)

use {
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_driver_session::SessionRuntime,
    reovim_kernel::api::v1::{CommandId, RegisterContent},
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
        let Some(buffer_arc) = runtime.kernel().buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        let count = args.count().unwrap_or(1);
        let buffer = buffer_arc.read();
        let start_line = buffer.position().line;
        let line_count = buffer.line_count();

        // Can't yank from empty buffer
        if line_count == 0 {
            return CommandResult::Success;
        }

        // Collect lines to yank
        let end_line = (start_line + count).min(line_count);
        let mut yanked = String::new();
        for line_idx in start_line..end_line {
            if let Some(line) = buffer.line(line_idx) {
                yanked.push_str(line);
                yanked.push('\n');
            }
        }
        drop(buffer);

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
