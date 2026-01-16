//! File operations.
//!
//! Provides file operation commands:
//! - `WriteBufferCommand` (`:w`)

use std::path::Path;

use {
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_kernel::api::v1::{CommandId, KernelContext},
};

use super::super::mode::EDITOR_MODULE;

/// Write buffer to file.
///
/// Saves the current buffer's contents to a file. If no path is provided,
/// uses the buffer's current file path.
///
/// This command demonstrates VFS integration - file operations go through
/// the VFS abstraction, enabling test isolation and future remote FS support.
///
/// # Arguments
///
/// - `file`: Optional file path to save to (defaults to buffer's path)
///
/// # Examples
///
/// - `:w` - Save to current file
/// - `:w newfile.txt` - Save to specified file
#[derive(Debug, Clone, Copy, Default)]
pub struct WriteBufferCommand;

impl Command for WriteBufferCommand {
    fn id(&self) -> CommandId {
        CommandId::new(EDITOR_MODULE, "write")
    }

    fn description(&self) -> &'static str {
        "Write buffer to file"
    }

    fn args(&self) -> Vec<ArgSpec> {
        vec![ArgSpec::optional(
            "file",
            ArgKind::FilePath,
            "Target file path (optional)",
        )]
    }

    fn names(&self) -> &[&'static str] {
        &["w", "write"]
    }
}

impl CommandHandler for WriteBufferCommand {
    fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
        // Get VFS from context
        let Some(vfs) = args.vfs() else {
            return CommandResult::error("VFS not available");
        };

        // Get active buffer
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        let Some(buffer_arc) = ctx.buffers.get(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };

        // Get content and file path from buffer, then release lock
        let (content, buffer_file_path) = {
            let buffer = buffer_arc.read();
            (buffer.content(), buffer.file_path().map(String::from))
        };

        // Try to get path from args first, then from buffer
        let file_path_str = args.string("file").map(String::from).or(buffer_file_path);

        let Some(file_path_str) = file_path_str else {
            return CommandResult::error("No file path specified");
        };

        let file_path = Path::new(&file_path_str);

        // Write to file via VFS
        match vfs.write(file_path, content.as_bytes()) {
            Ok(()) => {
                // Clear modified flag
                buffer_arc.write().set_modified(false);
                CommandResult::Success
            }
            Err(e) => CommandResult::error(format!("Write failed: {e}")),
        }
    }
}
