//! Yank commands.
//!
//! Provides yank (copy) commands:
//! - `YankLine` (yy, Y)

use {
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_kernel::api::v1::{CommandId, KernelContext, RegisterContent},
};

use reovim_kernel::api::v1::ModuleId;

// Command module ID for editor commands.
const EDITOR_MODULE: ModuleId = ModuleId::new("editor");

/// Yank current line(s) to register (yy, Y).
#[derive(Debug, Clone, Copy, Default)]
pub struct YankLine;

impl Command for YankLine {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "yank-line")
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
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };
        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
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
        ctx.registers.write().set_by_name(register, content);

        CommandResult::Success
    }
}
