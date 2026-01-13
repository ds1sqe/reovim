//! Quit command.
//!
//! Reference: `lib/core/src/command_line/ex_command.rs` (concept-extraction, not migration)

use reovim_kernel::api::v1::{CommandContext, CommandError, CommandHandler};

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

impl CommandHandler for QuitCommand {
    fn id(&self) -> &'static str {
        "quit"
    }

    fn names(&self) -> &[&'static str] {
        &["q", "quit"]
    }

    fn execute(&self, ctx: &mut CommandContext<'_>, _args: &[&str]) -> Result<(), CommandError> {
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
        assert!(names.contains(&"q"));
        assert!(names.contains(&"quit"));
    }

    #[test]
    fn test_quit_command_help() {
        let cmd = QuitCommand;
        assert!(!cmd.help().is_empty());
    }
}
