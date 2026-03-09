//! Write commands.

use {
    reovim_driver_command::{
        Command, CommandContext, CommandHandler, CommandResult, RuntimeSignal,
    },
    reovim_driver_session::{CommandApi, SessionRuntime},
    reovim_kernel::api::v1::{CommandId, ModuleId},
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

    fn names(&self) -> &[&'static str] {
        &["w", "write"]
    }
}

impl CommandHandler for WriteCommand {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, ctx: &CommandContext) -> CommandResult {
        let Some(_buffer_id) = ctx.buffer_id() else {
            return CommandResult::Error("no buffer".to_string());
        };

        let _filename = ctx.string("file");

        // In the full implementation:
        // 1. Get buffer content
        // 2. Write to file (via VFS driver)
        // 3. Clear modified flag
        // 4. Emit WriteEvent for other handlers

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
mod tests {
    use super::*;

    // ========================================================================
    // WriteCommand tests
    // ========================================================================

    #[test]
    fn test_write_command_id() {
        let cmd = WriteCommand;
        assert_eq!(cmd.id().name(), "write");
        assert_eq!(cmd.id().module().as_str(), "commands");
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
    fn test_write_command_description() {
        let cmd = WriteCommand;
        let desc = cmd.description();
        assert!(!desc.is_empty());
        assert!(desc.contains("Write"));
    }

    #[test]
    fn test_write_command_complete_returns_empty() {
        let cmd = WriteCommand;
        let completions = cmd.complete("some_partial");
        assert!(completions.is_empty());
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

    #[test]
    fn test_write_cmd_id_const() {
        assert_eq!(WRITE_CMD_ID.name(), "write");
        assert_eq!(WRITE_CMD_ID.module().as_str(), "commands");
    }

    // ========================================================================
    // WriteQuitCommand tests
    // ========================================================================

    #[test]
    fn test_write_quit_command_id() {
        let cmd = WriteQuitCommand;
        assert_eq!(cmd.id().name(), "write-quit");
        assert_eq!(cmd.id().module().as_str(), "commands");
    }

    #[test]
    fn test_write_quit_command_names() {
        let cmd = WriteQuitCommand;
        let names = cmd.names();
        assert_eq!(names.len(), 1);
        assert!(names.contains(&"wq"));
    }

    #[test]
    fn test_write_quit_command_description() {
        let cmd = WriteQuitCommand;
        let desc = cmd.description();
        assert!(!desc.is_empty());
        assert!(desc.contains("Write"));
        assert!(desc.contains("quit"));
    }

    #[test]
    fn test_write_quit_command_complete_returns_empty() {
        let cmd = WriteQuitCommand;
        let completions = cmd.complete("anything");
        assert!(completions.is_empty());
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
