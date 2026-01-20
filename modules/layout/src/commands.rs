//! Window management commands.
//!
//! Commands for window splitting, closing, focus navigation, and resizing.
//!
//! # Architecture (Epic #385)
//!
//! These commands are stubs that will be implemented when SessionRuntime
//! provides direct access to window state (#394). The actual window operations
//! will be performed directly instead of returning action intents.

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_session::SessionRuntime,
    reovim_kernel::api::v1::CommandId,
};

use crate::ids;

// =============================================================================
// Window Splitting Commands
// =============================================================================

/// Split window horizontally (`:split`).
#[derive(Debug, Clone, Copy, Default)]
pub struct SplitHorizontalCmd;

impl Command for SplitHorizontalCmd {
    fn id(&self) -> CommandId {
        ids::SPLIT_HORIZONTAL
    }

    fn description(&self) -> &'static str {
        "Split window horizontally"
    }
}

impl CommandHandler for SplitHorizontalCmd {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // TODO(#394): Implement via SessionRuntime (escape hatch until API supports this)
        CommandResult::Success
    }
}

/// Split window vertically (`:vsplit`).
#[derive(Debug, Clone, Copy, Default)]
pub struct SplitVerticalCmd;

impl Command for SplitVerticalCmd {
    fn id(&self) -> CommandId {
        ids::SPLIT_VERTICAL
    }

    fn description(&self) -> &'static str {
        "Split window vertically"
    }
}

impl CommandHandler for SplitVerticalCmd {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // TODO(#394): Implement via SessionRuntime (escape hatch until API supports this)
        CommandResult::Success
    }
}

// =============================================================================
// Window Closing Commands
// =============================================================================

/// Close current window (`:close`).
#[derive(Debug, Clone, Copy, Default)]
pub struct CloseWindowCmd;

impl Command for CloseWindowCmd {
    fn id(&self) -> CommandId {
        ids::CLOSE_WINDOW
    }

    fn description(&self) -> &'static str {
        "Close current window"
    }
}

impl CommandHandler for CloseWindowCmd {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // TODO(#394): Implement via SessionRuntime (escape hatch until API supports this)
        CommandResult::Success
    }
}

/// Close all other windows (`:only`).
#[derive(Debug, Clone, Copy, Default)]
pub struct CloseOthersCmd;

impl Command for CloseOthersCmd {
    fn id(&self) -> CommandId {
        ids::CLOSE_OTHERS
    }

    fn description(&self) -> &'static str {
        "Close all other windows"
    }
}

impl CommandHandler for CloseOthersCmd {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // TODO(#394): Implement via SessionRuntime (escape hatch until API supports this)
        CommandResult::Success
    }
}

// =============================================================================
// Focus Navigation Commands
// =============================================================================

/// Move focus to left window.
#[derive(Debug, Clone, Copy, Default)]
pub struct FocusLeftCmd;

impl Command for FocusLeftCmd {
    fn id(&self) -> CommandId {
        ids::FOCUS_LEFT
    }

    fn description(&self) -> &'static str {
        "Move focus to left window"
    }
}

impl CommandHandler for FocusLeftCmd {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // TODO(#394): Implement via SessionRuntime (escape hatch until API supports this)
        CommandResult::Success
    }
}

/// Move focus to right window.
#[derive(Debug, Clone, Copy, Default)]
pub struct FocusRightCmd;

impl Command for FocusRightCmd {
    fn id(&self) -> CommandId {
        ids::FOCUS_RIGHT
    }

    fn description(&self) -> &'static str {
        "Move focus to right window"
    }
}

impl CommandHandler for FocusRightCmd {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // TODO(#394): Implement via SessionRuntime (escape hatch until API supports this)
        CommandResult::Success
    }
}

/// Move focus to window above.
#[derive(Debug, Clone, Copy, Default)]
pub struct FocusUpCmd;

impl Command for FocusUpCmd {
    fn id(&self) -> CommandId {
        ids::FOCUS_UP
    }

    fn description(&self) -> &'static str {
        "Move focus to window above"
    }
}

impl CommandHandler for FocusUpCmd {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // TODO(#394): Implement via SessionRuntime (escape hatch until API supports this)
        CommandResult::Success
    }
}

/// Move focus to window below.
#[derive(Debug, Clone, Copy, Default)]
pub struct FocusDownCmd;

impl Command for FocusDownCmd {
    fn id(&self) -> CommandId {
        ids::FOCUS_DOWN
    }

    fn description(&self) -> &'static str {
        "Move focus to window below"
    }
}

impl CommandHandler for FocusDownCmd {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // TODO(#394): Implement via SessionRuntime (escape hatch until API supports this)
        CommandResult::Success
    }
}

// =============================================================================
// Focus Cycling Commands
// =============================================================================

/// Cycle focus to next window.
#[derive(Debug, Clone, Copy, Default)]
pub struct CycleForwardCmd;

impl Command for CycleForwardCmd {
    fn id(&self) -> CommandId {
        ids::FOCUS_NEXT
    }

    fn description(&self) -> &'static str {
        "Cycle focus to next window"
    }
}

impl CommandHandler for CycleForwardCmd {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // TODO(#394): Implement via SessionRuntime (escape hatch until API supports this)
        CommandResult::Success
    }
}

/// Cycle focus to previous window.
#[derive(Debug, Clone, Copy, Default)]
pub struct CycleBackwardCmd;

impl Command for CycleBackwardCmd {
    fn id(&self) -> CommandId {
        ids::FOCUS_PREV
    }

    fn description(&self) -> &'static str {
        "Cycle focus to previous window"
    }
}

impl CommandHandler for CycleBackwardCmd {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // TODO(#394): Implement via SessionRuntime (escape hatch until API supports this)
        CommandResult::Success
    }
}

// =============================================================================
// Window Resizing Commands
// =============================================================================

/// Increase window height.
#[derive(Debug, Clone, Copy, Default)]
pub struct ResizeHeightIncreaseCmd;

impl Command for ResizeHeightIncreaseCmd {
    fn id(&self) -> CommandId {
        ids::RESIZE_HEIGHT_INCREASE
    }

    fn description(&self) -> &'static str {
        "Increase window height"
    }
}

impl CommandHandler for ResizeHeightIncreaseCmd {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // TODO(#394): Implement via SessionRuntime (escape hatch until API supports this)
        CommandResult::Success
    }
}

/// Decrease window height.
#[derive(Debug, Clone, Copy, Default)]
pub struct ResizeHeightDecreaseCmd;

impl Command for ResizeHeightDecreaseCmd {
    fn id(&self) -> CommandId {
        ids::RESIZE_HEIGHT_DECREASE
    }

    fn description(&self) -> &'static str {
        "Decrease window height"
    }
}

impl CommandHandler for ResizeHeightDecreaseCmd {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // TODO(#394): Implement via SessionRuntime (escape hatch until API supports this)
        CommandResult::Success
    }
}

/// Increase window width.
#[derive(Debug, Clone, Copy, Default)]
pub struct ResizeWidthIncreaseCmd;

impl Command for ResizeWidthIncreaseCmd {
    fn id(&self) -> CommandId {
        ids::RESIZE_WIDTH_INCREASE
    }

    fn description(&self) -> &'static str {
        "Increase window width"
    }
}

impl CommandHandler for ResizeWidthIncreaseCmd {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // TODO(#394): Implement via SessionRuntime (escape hatch until API supports this)
        CommandResult::Success
    }
}

/// Decrease window width.
#[derive(Debug, Clone, Copy, Default)]
pub struct ResizeWidthDecreaseCmd;

impl Command for ResizeWidthDecreaseCmd {
    fn id(&self) -> CommandId {
        ids::RESIZE_WIDTH_DECREASE
    }

    fn description(&self) -> &'static str {
        "Decrease window width"
    }
}

impl CommandHandler for ResizeWidthDecreaseCmd {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // TODO(#394): Implement via SessionRuntime (escape hatch until API supports this)
        CommandResult::Success
    }
}

/// Equalize all window sizes.
#[derive(Debug, Clone, Copy, Default)]
pub struct ResizeEqualCmd;

impl Command for ResizeEqualCmd {
    fn id(&self) -> CommandId {
        ids::RESIZE_EQUAL
    }

    fn description(&self) -> &'static str {
        "Make all windows equal size"
    }
}

impl CommandHandler for ResizeEqualCmd {
    fn execute(&self, _runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // TODO(#394): Implement via SessionRuntime (escape hatch until API supports this)
        CommandResult::Success
    }
}

// =============================================================================
// Command Collection
// =============================================================================

/// Get all window commands as trait objects.
///
/// Returns a vector of boxed command handlers that can be registered
/// with the command registry.
#[must_use]
pub fn all_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        // Splitting
        Box::new(SplitHorizontalCmd),
        Box::new(SplitVerticalCmd),
        // Closing
        Box::new(CloseWindowCmd),
        Box::new(CloseOthersCmd),
        // Focus navigation
        Box::new(FocusLeftCmd),
        Box::new(FocusRightCmd),
        Box::new(FocusUpCmd),
        Box::new(FocusDownCmd),
        // Focus cycling
        Box::new(CycleForwardCmd),
        Box::new(CycleBackwardCmd),
        // Resizing
        Box::new(ResizeHeightIncreaseCmd),
        Box::new(ResizeHeightDecreaseCmd),
        Box::new(ResizeWidthIncreaseCmd),
        Box::new(ResizeWidthDecreaseCmd),
        Box::new(ResizeEqualCmd),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_commands_not_empty() {
        let commands = all_commands();
        assert!(!commands.is_empty());
        assert_eq!(commands.len(), 15); // 2 split + 2 close + 4 focus + 2 cycle + 5 resize
    }

    #[test]
    fn test_command_ids_are_unique() {
        let commands = all_commands();
        let mut ids = std::collections::HashSet::new();

        for cmd in &commands {
            let id = cmd.id();
            assert!(ids.insert(id.clone()), "Duplicate command ID: {}", id.name());
        }
    }
}
