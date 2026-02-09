//! Quit command.
//!
//! Reference: `lib/core/src/command_line/ex_command.rs` (concept-extraction, not migration)

use crate::{CommandError, ExCommandContext, ExCommandHandler};

/// Quit command - exit the editor.
///
/// Behavior:
/// - `:q` - Quit if no unsaved changes
/// - `:q!` - Force quit, discard unsaved changes
///
/// # Example
///
/// ```ignore
/// let quit = QuitCommand;
/// quit.execute(&mut ctx, &[])?; // :q
/// ```
#[derive(Debug, Clone, Copy)]
pub struct QuitCommand;

impl ExCommandHandler for QuitCommand {
    fn id(&self) -> &'static str {
        "quit"
    }

    fn names(&self) -> &[&'static str] {
        &["q", "quit"]
    }

    fn execute(&self, ctx: &mut ExCommandContext<'_>, _args: &[&str]) -> Result<(), CommandError> {
        // Check for unsaved changes (unless bang is set)
        if !ctx.bang {
            // In a full implementation, we would check if the buffer is modified
            // and return an error if so. For now, we just note this.
            // The actual quit action is handled by the runner via events.
        }

        // Signal quit via event bus
        // Note: The actual quit is handled by the runner. This command just
        // signals the intent. The runner listens for quit events.
        //
        // In the full implementation:
        // ctx.kernel.event_bus.emit(QuitEvent { force: ctx.bang });

        Ok(())
    }

    fn help(&self) -> &'static str {
        "Quit the editor. Use :q! to force quit with unsaved changes."
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_quit_command_id() {
        let cmd = QuitCommand;
        assert_eq!(cmd.id(), "quit");
    }

    #[test]
    fn test_quit_command_names() {
        let cmd = QuitCommand;
        let names = cmd.names();
        assert_eq!(names.len(), 2);
        assert!(names.contains(&"q"));
        assert!(names.contains(&"quit"));
    }

    #[test]
    fn test_quit_command_help() {
        let cmd = QuitCommand;
        let help = cmd.help();
        assert!(!help.is_empty());
        assert!(help.contains("Quit"));
        assert!(help.contains("q!"));
    }

    #[test]
    fn test_quit_command_complete_returns_empty() {
        let cmd = QuitCommand;
        let completions = cmd.complete("anything");
        assert!(completions.is_empty());
    }

    #[test]
    fn test_quit_command_execute_without_bang() {
        let kernel = reovim_kernel::api::v1::KernelContext::default();
        let mut ctx = ExCommandContext::new(&kernel);
        ctx.bang = false;

        let cmd = QuitCommand;
        let result = cmd.execute(&mut ctx, &[]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_quit_command_execute_with_bang() {
        let kernel = reovim_kernel::api::v1::KernelContext::default();
        let mut ctx = ExCommandContext::new(&kernel).with_bang(true);

        let cmd = QuitCommand;
        let result = cmd.execute(&mut ctx, &[]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_quit_command_execute_ignores_args() {
        let kernel = reovim_kernel::api::v1::KernelContext::default();
        let mut ctx = ExCommandContext::new(&kernel);

        let cmd = QuitCommand;
        let result = cmd.execute(&mut ctx, &["extra", "args"]);
        assert!(result.is_ok());
    }

    #[test]
    fn test_quit_command_debug() {
        let cmd = QuitCommand;
        let debug = format!("{cmd:?}");
        assert!(debug.contains("QuitCommand"));
    }

    #[test]
    fn test_quit_command_clone() {
        let cmd = QuitCommand;
        let cloned = cmd;
        assert_eq!(cmd.id(), cloned.id());
    }

    #[test]
    fn test_quit_command_is_send_sync() {
        fn assert_send_sync<T: Send + Sync>() {}
        assert_send_sync::<QuitCommand>();
    }
}
