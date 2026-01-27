//! Commands for the which-key popup.
//!
//! These commands control the which-key popup visibility and filtering:
//! - `WhichKeyShowCommand` - Shows the popup immediately
//! - `WhichKeyCloseCommand` - Hides the popup and cancels timer
//! - `WhichKeyFilterCommand` - Filters displayed bindings by typed key
//!
//! # Architecture
//!
//! The commands access the which-key state via the session's extension map
//! (`WhichKeySessionExt`) and control the overlay via `CompositorApi`.
//!
//! # Overlay Positioning
//!
//! The which-key popup is positioned at the bottom of the screen:
//! - Full screen width
//! - Height based on number of bindings
//! - Positioned at y = screen_height - popup_height

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::{SessionRuntime, api::CompositorApi, api::ExtensionApi},
    reovim_kernel::api::v1::CommandId,
};

use crate::{ids, render::bottom_overlay_constraints, state::WhichKeySessionExt};

/// Command to show the which-key popup immediately.
///
/// This is typically bound to `?` after a prefix key (e.g., `g?`).
/// It shows available bindings without waiting for the timeout.
pub struct WhichKeyShowCommand;

impl Command for WhichKeyShowCommand {
    fn id(&self) -> CommandId {
        ids::WHICH_KEY_SHOW
    }

    fn description(&self) -> &'static str {
        "Show which-key popup immediately"
    }
}

impl CommandHandler for WhichKeyShowCommand {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // Get screen size for positioning
        let (width, height) = runtime.session().terminal_size();

        // Default popup height (will be updated when bindings are rendered)
        let popup_height: u16 = 10;

        // Create bottom-anchored overlay constraints
        let constraints = bottom_overlay_constraints(width, height, popup_height);

        // Show the overlay
        match runtime.show_overlay(constraints) {
            Ok(window_id) => {
                // Store the overlay window ID in session extension
                let ext = runtime.ext_mut::<WhichKeySessionExt>();
                ext.set_overlay_window_id(window_id);

                tracing::debug!(
                    "which-key: popup shown at bottom, size {}x{}",
                    width,
                    popup_height
                );
                CommandResult::Success
            }
            Err(e) => {
                tracing::warn!("which-key: failed to show overlay: {}", e);
                CommandResult::Error(format!("Failed to show which-key popup: {}", e))
            }
        }
    }
}

/// Command to close the which-key popup.
///
/// This is typically bound to `<Escape>` when the popup is visible.
/// It cancels any pending timer and hides the popup.
pub struct WhichKeyCloseCommand;

impl Command for WhichKeyCloseCommand {
    fn id(&self) -> CommandId {
        ids::WHICH_KEY_CLOSE
    }

    fn description(&self) -> &'static str {
        "Close which-key popup"
    }
}

impl CommandHandler for WhichKeyCloseCommand {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // Get the which-key extension
        let ext = runtime.ext_mut::<WhichKeySessionExt>();

        // Get the overlay window ID
        if let Some(window_id) = ext.overlay_window_id() {
            // Clear the overlay ID first
            ext.clear_overlay_window_id();

            // Hide the overlay
            if let Err(e) = runtime.hide_overlay(window_id) {
                tracing::warn!("which-key: failed to hide overlay: {}", e);
                return CommandResult::Error(format!("Failed to hide which-key popup: {}", e));
            }

            tracing::debug!("which-key: popup closed");
        } else {
            tracing::trace!("which-key: close called but no popup visible");
        }

        CommandResult::Success
    }
}

/// Command to filter displayed bindings by a typed key.
///
/// When the popup is visible, typing keys that don't directly match
/// a binding will narrow the displayed options.
pub struct WhichKeyFilterCommand;

impl Command for WhichKeyFilterCommand {
    fn id(&self) -> CommandId {
        ids::WHICH_KEY_FILTER
    }

    fn description(&self) -> &'static str {
        "Filter which-key bindings by typed key"
    }
}

impl CommandHandler for WhichKeyFilterCommand {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // TODO: Implement filter (Phase 9)
        // 1. Get the typed key from args
        // 2. Push to filter in cache
        // 3. Re-filter displayed bindings
        tracing::debug!("which-key: filter command executed (stub)");
        CommandResult::Success
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ids::MODULE;

    #[test]
    fn test_show_command_id() {
        let cmd = WhichKeyShowCommand;
        assert_eq!(cmd.id().name(), "which-key-show");
        assert_eq!(cmd.id().module(), &MODULE);
    }

    #[test]
    fn test_show_command_description() {
        let cmd = WhichKeyShowCommand;
        assert!(!cmd.description().is_empty());
        assert!(cmd.description().contains("Show"));
    }

    #[test]
    fn test_close_command_id() {
        let cmd = WhichKeyCloseCommand;
        assert_eq!(cmd.id().name(), "which-key-close");
        assert_eq!(cmd.id().module(), &MODULE);
    }

    #[test]
    fn test_close_command_description() {
        let cmd = WhichKeyCloseCommand;
        assert!(!cmd.description().is_empty());
        assert!(cmd.description().contains("Close"));
    }

    #[test]
    fn test_filter_command_id() {
        let cmd = WhichKeyFilterCommand;
        assert_eq!(cmd.id().name(), "which-key-filter");
        assert_eq!(cmd.id().module(), &MODULE);
    }

    #[test]
    fn test_filter_command_description() {
        let cmd = WhichKeyFilterCommand;
        assert!(!cmd.description().is_empty());
        assert!(cmd.description().contains("Filter"));
    }

    #[test]
    fn test_all_commands_belong_to_module() {
        assert_eq!(WhichKeyShowCommand.id().module(), &MODULE);
        assert_eq!(WhichKeyCloseCommand.id().module(), &MODULE);
        assert_eq!(WhichKeyFilterCommand.id().module(), &MODULE);
    }
}
