//! Write commands.

use std::path::Path;

use {
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult, RuntimeSignal,
    },
    reovim_driver_session::{BufferApi, CommandApi, SessionRuntime},
    reovim_kernel::api::v1::{CommandId, ModuleId, events::kernel::BufferSaved},
};

const COMMANDS_MODULE: ModuleId = ModuleId::new("commands");

/// Write command - save the buffer to disk.
///
/// Behavior:
/// - `:w` - Write current buffer to its file
/// - `:w filename` - Write current buffer to specified file
#[derive(Debug, Clone, Copy)]
pub struct WriteCommand;

/// Command ID for the write command (used by `WriteQuitCommand` for re-entrant call).
pub const WRITE_CMD_ID: CommandId = CommandId::new(COMMANDS_MODULE, "write");

impl Command for WriteCommand {
    fn id(&self) -> CommandId {
        WRITE_CMD_ID
    }

    fn description(&self) -> &'static str {
        "Write the current buffer to disk. Use :w filename to save to a specific file."
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional("file", ArgKind::Rest, "File to write")]
    }

    fn names(&self) -> &[&'static str] {
        &["w", "write"]
    }
}

impl CommandHandler for WriteCommand {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, ctx: &CommandContext) -> CommandResult {
        let Some(buffer_id) = ctx.buffer_id() else {
            return CommandResult::Error("no buffer".to_string());
        };

        // Determine target path: explicit argument or buffer's existing path
        let explicit_file = ctx.string("file");
        let path = if let Some(file) = explicit_file {
            file.to_string()
        } else if let Some(existing) = runtime.buffer_file_path(buffer_id) {
            existing
        } else {
            return CommandResult::Error("No file name".to_string());
        };

        // Get buffer content
        let Some(content) = runtime.buffer_content(buffer_id) else {
            return CommandResult::Error("buffer not found".to_string());
        };

        // Write via VFS
        let Some(vfs) = ctx.vfs() else {
            return CommandResult::Error("VFS not available".to_string());
        };
        if let Err(e) = vfs.write_str(Path::new(&path), &content) {
            return CommandResult::Error(format!("Write failed: {e}"));
        }

        // If saving to a new filename, update the buffer's file path
        if explicit_file.is_some() {
            runtime.rename_buffer(buffer_id, &path);
        }

        // Clear modified flag
        runtime.set_buffer_modified(buffer_id, false);

        // Emit BufferSaved event for subscribers (LSP DidSave, etc.)
        #[allow(clippy::cast_possible_truncation)]
        let buffer_id_raw = buffer_id.as_usize() as u64;
        runtime.kernel().event_bus.emit(BufferSaved {
            buffer_id: buffer_id_raw,
            path,
        });

        CommandResult::Success
    }
}

/// Write and quit command - save and exit.
///
/// Uses re-entrant execution: calls `:write` first, then signals quit.
#[derive(Debug, Clone, Copy)]
pub struct WriteQuitCommand;

impl Command for WriteQuitCommand {
    fn id(&self) -> CommandId {
        CommandId::new(COMMANDS_MODULE, "write-quit")
    }

    fn description(&self) -> &'static str {
        "Write the current buffer and quit the editor."
    }

    fn names(&self) -> &[&'static str] {
        &["wq"]
    }
}

impl CommandHandler for WriteQuitCommand {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, ctx: &CommandContext) -> CommandResult {
        // Write first via re-entrant execution
        let result = runtime.execute_command(WRITE_CMD_ID, ctx.clone());
        if result.is_error() {
            return result;
        }

        // Then signal quit
        runtime.signal(RuntimeSignal::Quit);
        CommandResult::Success
    }
}

#[cfg(test)]
#[path = "write_tests.rs"]
mod tests;
