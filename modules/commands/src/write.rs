//! Write commands.
//!
//! Reference: `lib/core/src/command_line/ex_command.rs` (concept-extraction, not migration)

use crate::{CommandContext, CommandError, CommandHandler};

/// Write command - save the buffer to disk.
///
/// Behavior:
/// - `:w` - Write current buffer to its file
/// - `:w filename` - Write current buffer to specified file
///
/// # Example
///
/// ```ignore
/// let write = WriteCommand;
/// write.execute(&mut ctx, &[])?; // :w (save current file)
/// write.execute(&mut ctx, &["newfile.txt"])?; // :w newfile.txt
/// ```
#[derive(Debug, Clone, Copy)]
pub struct WriteCommand;

impl CommandHandler for WriteCommand {
    fn id(&self) -> &'static str {
        "write"
    }

    fn names(&self) -> &[&'static str] {
        &["w", "write"]
    }

    fn execute(&self, ctx: &mut CommandContext<'_>, args: &[&str]) -> Result<(), CommandError> {
        let _buffer_id = ctx.buffer_id.ok_or(CommandError::NoBuffer)?;

        let _filename = if args.is_empty() {
            // Use buffer's current filename
            // In full implementation: ctx.kernel.buffers.get(buffer_id).path()
            None
        } else {
            Some(args[0])
        };

        // In the full implementation:
        // 1. Get buffer content
        // 2. Write to file (via VFS driver)
        // 3. Clear modified flag
        // 4. Emit WriteEvent for other handlers

        Ok(())
    }

    fn complete(&self, partial: &str) -> Vec<String> {
        // File path completion would go here
        // For now, return empty - actual implementation uses VFS driver
        let _ = partial;
        vec![]
    }

    fn help(&self) -> &'static str {
        "Write the current buffer to disk. Use :w filename to save to a specific file."
    }
}

/// Write and quit command - save and exit.
///
/// Behavior:
/// - `:wq` - Write current buffer and quit
///
/// # Example
///
/// ```ignore
/// let wq = WriteQuitCommand;
/// wq.execute(&mut ctx, &[])?; // :wq
/// ```
#[derive(Debug, Clone, Copy)]
pub struct WriteQuitCommand;

impl CommandHandler for WriteQuitCommand {
    fn id(&self) -> &'static str {
        "write-quit"
    }

    fn names(&self) -> &[&'static str] {
        &["wq"]
    }

    fn execute(&self, ctx: &mut CommandContext<'_>, args: &[&str]) -> Result<(), CommandError> {
        // Write first
        WriteCommand.execute(ctx, args)?;

        // Then signal quit
        // In full implementation: emit QuitEvent

        Ok(())
    }

    fn help(&self) -> &'static str {
        "Write the current buffer and quit the editor."
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_write_command_id() {
        let cmd = WriteCommand;
        assert_eq!(cmd.id(), "write");
    }

    #[test]
    fn test_write_command_names() {
        let cmd = WriteCommand;
        let names = cmd.names();
        assert!(names.contains(&"w"));
        assert!(names.contains(&"write"));
    }

    #[test]
    fn test_write_quit_command_id() {
        let cmd = WriteQuitCommand;
        assert_eq!(cmd.id(), "write-quit");
    }

    #[test]
    fn test_write_quit_command_names() {
        let cmd = WriteQuitCommand;
        let names = cmd.names();
        assert!(names.contains(&"wq"));
    }
}
