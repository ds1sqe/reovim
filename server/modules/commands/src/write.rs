//! Write commands.
//!
//! Reference: `lib/core/src/command_line/ex_command.rs` (concept-extraction, not migration)

use crate::{CommandError, ExCommandContext, ExCommandHandler};

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

impl ExCommandHandler for WriteCommand {
    fn id(&self) -> &'static str {
        "write"
    }

    fn names(&self) -> &[&'static str] {
        &["w", "write"]
    }

    fn execute(&self, ctx: &mut ExCommandContext<'_>, args: &[&str]) -> Result<(), CommandError> {
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

impl ExCommandHandler for WriteQuitCommand {
    fn id(&self) -> &'static str {
        "write-quit"
    }

    fn names(&self) -> &[&'static str] {
        &["wq"]
    }

    fn execute(&self, ctx: &mut ExCommandContext<'_>, args: &[&str]) -> Result<(), CommandError> {
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
    use {
        super::*,
        reovim_kernel::api::v1::{BufferId, KernelContext},
    };

    // ========================================================================
    // WriteCommand tests
    // ========================================================================

    #[test]
    fn test_write_command_id() {
        let cmd = WriteCommand;
        assert_eq!(cmd.id(), "write");
    }

    #[test]
    fn test_write_command_names() {
        let cmd = WriteCommand;
        let names = cmd.names();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"w"));
        assert!(names.contains(&"write"));
    }

    #[test]
    fn test_write_command_help() {
        let cmd = WriteCommand;
        let help = cmd.help();
        assert!(!help.is_empty());
        assert!(help.contains("Write"));
    }

    #[test]
    fn test_write_command_complete_returns_empty() {
        let cmd = WriteCommand;
        let completions = cmd.complete("some_partial");
        assert!(completions.is_empty());
    }

    #[test]
    fn test_write_command_complete_empty_input() {
        let cmd = WriteCommand;
        let completions = cmd.complete("");
        assert!(completions.is_empty());
    }

    #[test]
    fn test_write_command_execute_no_buffer_returns_error() {
        let kernel = KernelContext::default();
        let mut ctx = ExCommandContext::new(&kernel);
        // buffer_id is None by default

        let cmd = WriteCommand;
        let result = cmd.execute(&mut ctx, &[]);
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.to_string(), "no buffer");
    }

    #[test]
    fn test_write_command_execute_with_buffer_no_args() {
        let kernel = KernelContext::default();
        let buffer_id = BufferId::from_raw(1);
        let mut ctx = ExCommandContext::new(&kernel).with_buffer(buffer_id);

        let cmd = WriteCommand;
        let result = cmd.execute(&mut ctx, &[]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_write_command_execute_with_buffer_and_filename() {
        let kernel = KernelContext::default();
        let buffer_id = BufferId::from_raw(1);
        let mut ctx = ExCommandContext::new(&kernel).with_buffer(buffer_id);

        let cmd = WriteCommand;
        let result = cmd.execute(&mut ctx, &["output.txt"]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_write_command_execute_with_multiple_args() {
        let kernel = KernelContext::default();
        let buffer_id = BufferId::from_raw(1);
        let mut ctx = ExCommandContext::new(&kernel).with_buffer(buffer_id);

        let cmd = WriteCommand;
        // Only the first arg is used as filename
        let result = cmd.execute(&mut ctx, &["file.txt", "extra"]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_write_command_debug() {
        let cmd = WriteCommand;
        let debug = format!("{cmd:?}");
        assert!(debug.contains("WriteCommand"));
    }

    #[test]
    fn test_write_command_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<WriteCommand>();
    }

    // ========================================================================
    // WriteQuitCommand tests
    // ========================================================================

    #[test]
    fn test_write_quit_command_id() {
        let cmd = WriteQuitCommand;
        assert_eq!(cmd.id(), "write-quit");
    }

    #[test]
    fn test_write_quit_command_names() {
        let cmd = WriteQuitCommand;
        let names = cmd.names();
        assert_eq!(names.len(), 1);
        assert!(names.contains(&"wq"));
    }

    #[test]
    fn test_write_quit_command_help() {
        let cmd = WriteQuitCommand;
        let help = cmd.help();
        assert!(!help.is_empty());
        assert!(help.contains("Write"));
        assert!(help.contains("quit"));
    }

    #[test]
    fn test_write_quit_command_complete_returns_empty() {
        let cmd = WriteQuitCommand;
        let completions = cmd.complete("anything");
        assert!(completions.is_empty());
    }

    #[test]
    fn test_write_quit_command_execute_no_buffer_returns_error() {
        let kernel = KernelContext::default();
        let mut ctx = ExCommandContext::new(&kernel);

        let cmd = WriteQuitCommand;
        let result = cmd.execute(&mut ctx, &[]);
        assert!(result.is_err());
        // WriteQuitCommand delegates to WriteCommand first, which fails on NoBuffer
        let err = result.unwrap_err();
        assert_eq!(err.to_string(), "no buffer");
    }

    #[test]
    fn test_write_quit_command_execute_with_buffer() {
        let kernel = KernelContext::default();
        let buffer_id = BufferId::from_raw(1);
        let mut ctx = ExCommandContext::new(&kernel).with_buffer(buffer_id);

        let cmd = WriteQuitCommand;
        let result = cmd.execute(&mut ctx, &[]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_write_quit_command_execute_with_filename() {
        let kernel = KernelContext::default();
        let buffer_id = BufferId::from_raw(1);
        let mut ctx = ExCommandContext::new(&kernel).with_buffer(buffer_id);

        let cmd = WriteQuitCommand;
        let result = cmd.execute(&mut ctx, &["output.txt"]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_write_quit_command_debug() {
        let cmd = WriteQuitCommand;
        let debug = format!("{cmd:?}");
        assert!(debug.contains("WriteQuitCommand"));
    }

    #[test]
    fn test_write_quit_command_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<WriteQuitCommand>();
    }
}
