//! File operations.
//!
//! Provides file operation commands:
//! - `WriteBufferCommand` (`:w`)

use std::path::Path;

use {
    reovim_driver_command::{
        ArgKind, ArgSpec, Command, CommandContext, CommandHandler, CommandResult,
    },
    reovim_driver_session::{
            BufferApi, SessionRuntime},
    reovim_kernel::api::v1::CommandId,
};

use crate::ids;

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
        ids::WRITE
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

    // No names() — WriteCommand in commands module owns ["w", "write"].
    // This command is only accessible via CommandId for programmatic use.
}

impl CommandHandler for WriteBufferCommand {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, args: &CommandContext) -> CommandResult {
        // Get VFS from context
        let Some(vfs) = args.vfs() else {
            return CommandResult::error("VFS not available");
        };

        // Get active buffer
        let Some(buffer_id) = args.buffer_id() else {
            return CommandResult::error("No active buffer");
        };

        // Get content and file path from buffer via API
        let Some(content) = runtime.buffer_content(buffer_id) else {
            return CommandResult::error("Buffer not found");
        };
        let buffer_file_path = runtime.buffer_file_path(buffer_id);

        // Try to get path from args first, then from buffer
        let file_path_str = args.string("file").map(String::from).or(buffer_file_path);

        let Some(file_path_str) = file_path_str else {
            return CommandResult::error("No file path specified");
        };

        let file_path = Path::new(&file_path_str);

        // Write to file via VFS
        match vfs.write(file_path, content.as_bytes()) {
            Ok(()) => {
                // Clear modified flag via API
                runtime.set_buffer_modified(buffer_id, false);
                CommandResult::Success
            }
            Err(e) => CommandResult::error(&format!("Write failed: {e}")),
        }
    }
}

