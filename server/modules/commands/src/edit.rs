//! Edit command - open/reload files.
//!
//! Implements the `:e` (edit) command for opening files in buffers.
//!
//! # Behavior
//!
//! - `:e filename` - Open file in current buffer
//! - `:e` - Reload current file (not yet implemented)
//!
//! # Note
//!
//! This is a minimal implementation sufficient for the `vim_commands.rs`
//! integration tests. The `IntegrationTest` harness uses `:e` to open
//! files with specific content for testing.

use std::path::Path;

use crate::{CommandError, ExCommandContext, ExCommandHandler};

/// Edit command - open a file in the current buffer.
///
/// # Example
///
/// ```ignore
/// let edit = EditCommand;
/// edit.execute(&mut ctx, &["file.txt"])?; // :e file.txt
/// ```
#[derive(Debug, Clone, Copy)]
pub struct EditCommand;

impl ExCommandHandler for EditCommand {
    fn id(&self) -> &'static str {
        "edit"
    }

    fn names(&self) -> &[&'static str] {
        &["e", "edit"]
    }

    fn execute(&self, ctx: &mut ExCommandContext<'_>, args: &[&str]) -> Result<(), CommandError> {
        // Get the filename argument
        let filename = match args.first() {
            Some(f) => *f,
            None => {
                // :e without argument - reload current file (not implemented)
                return Err(CommandError::InvalidArguments(
                    "No filename specified. Reload not yet implemented.".to_string(),
                ));
            }
        };

        // Get the buffer to operate on
        let buffer_id = ctx.buffer_id.ok_or(CommandError::NoBuffer)?;

        // Get VFS for file operations
        let vfs = ctx
            .vfs()
            .ok_or_else(|| CommandError::ExecutionFailed("VFS not available".to_string()))?;

        // Read file content
        let content = match vfs.read(Path::new(filename)) {
            Ok(bytes) => match String::from_utf8(bytes) {
                Ok(text) => text,
                Err(_) => {
                    return Err(CommandError::ExecutionFailed(
                        "File is not valid UTF-8".to_string(),
                    ));
                }
            },
            Err(e) => {
                return Err(CommandError::ExecutionFailed(format!(
                    "Cannot read file '{filename}': {e}"
                )));
            }
        };

        // Get buffer and set content
        let buffer_arc = ctx.kernel.buffers.get(buffer_id).ok_or_else(|| {
            CommandError::ExecutionFailed(format!("Buffer {} not found", buffer_id.as_usize()))
        })?;

        // Update buffer content
        {
            let mut buffer = buffer_arc.write();

            // Set new content (this also resets cursor to origin)
            buffer.set_content(&content);

            // Set metadata - mark as unmodified since we just loaded
            buffer.set_file_path(Some(filename.to_string()));
            buffer.set_modified(false);
        }

        Ok(())
    }

    fn complete(&self, partial: &str) -> Vec<String> {
        // File path completion would go here
        // For now, return empty - actual implementation uses VFS driver
        let _ = partial;
        vec![]
    }

    fn help(&self) -> &'static str {
        "Edit (open) a file in the current buffer"
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_edit_command_id() {
        let cmd = EditCommand;
        assert_eq!(cmd.id(), "edit");
    }

    #[test]
    fn test_edit_command_names() {
        let cmd = EditCommand;
        let names = cmd.names();
        assert!(names.contains(&"e"));
        assert!(names.contains(&"edit"));
    }

    #[test]
    fn test_edit_command_help() {
        let cmd = EditCommand;
        assert!(!cmd.help().is_empty());
    }
}
