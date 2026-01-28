//! Window management commands.
//!
//! Commands for window splitting, closing, focus navigation, and resizing.
//! All commands use the `CompositorApi` trait to interact with the window
//! layout system.

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult},
    reovim_driver_display::{NavigateDirection, SplitDirection},
    reovim_driver_session::{CompositorApi, SessionRuntime},
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
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        match runtime.split(SplitDirection::Horizontal) {
            Ok(window) => {
                if let Err(e) = runtime.focus(window) {
                    return CommandResult::Error(e.to_string());
                }
                CommandResult::Success
            }
            Err(e) => CommandResult::Error(e.to_string()),
        }
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
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        match runtime.split(SplitDirection::Vertical) {
            Ok(window) => {
                if let Err(e) = runtime.focus(window) {
                    return CommandResult::Error(e.to_string());
                }
                CommandResult::Success
            }
            Err(e) => CommandResult::Error(e.to_string()),
        }
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
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        match runtime.close_current_window() {
            Ok(neighbor) => {
                if let Err(e) = runtime.focus(neighbor) {
                    return CommandResult::Error(e.to_string());
                }
                CommandResult::Success
            }
            Err(e) => CommandResult::Error(e.to_string()),
        }
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
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        match runtime.close_others() {
            Ok(()) => CommandResult::Success,
            Err(e) => CommandResult::Error(e.to_string()),
        }
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
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        match runtime.navigate(NavigateDirection::Left) {
            Ok(window) => {
                if let Err(e) = runtime.focus(window) {
                    return CommandResult::Error(e.to_string());
                }
                CommandResult::Success
            }
            Err(e) => CommandResult::Error(e.to_string()),
        }
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
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        match runtime.navigate(NavigateDirection::Right) {
            Ok(window) => {
                if let Err(e) = runtime.focus(window) {
                    return CommandResult::Error(e.to_string());
                }
                CommandResult::Success
            }
            Err(e) => CommandResult::Error(e.to_string()),
        }
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
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        match runtime.navigate(NavigateDirection::Up) {
            Ok(window) => {
                if let Err(e) = runtime.focus(window) {
                    return CommandResult::Error(e.to_string());
                }
                CommandResult::Success
            }
            Err(e) => CommandResult::Error(e.to_string()),
        }
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
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        match runtime.navigate(NavigateDirection::Down) {
            Ok(window) => {
                if let Err(e) = runtime.focus(window) {
                    return CommandResult::Error(e.to_string());
                }
                CommandResult::Success
            }
            Err(e) => CommandResult::Error(e.to_string()),
        }
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
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        match runtime.cycle(true) {
            Ok(window) => {
                if let Err(e) = runtime.focus(window) {
                    return CommandResult::Error(e.to_string());
                }
                CommandResult::Success
            }
            Err(e) => CommandResult::Error(e.to_string()),
        }
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
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        match runtime.cycle(false) {
            Ok(window) => {
                if let Err(e) = runtime.focus(window) {
                    return CommandResult::Error(e.to_string());
                }
                CommandResult::Success
            }
            Err(e) => CommandResult::Error(e.to_string()),
        }
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
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // Increase height = push bottom edge down
        match runtime.resize(NavigateDirection::Down, 1) {
            Ok(()) => CommandResult::Success,
            Err(e) => CommandResult::Error(e.to_string()),
        }
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
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // Decrease height = pull bottom edge up (negative delta)
        match runtime.resize(NavigateDirection::Down, -1) {
            Ok(()) => CommandResult::Success,
            Err(e) => CommandResult::Error(e.to_string()),
        }
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
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // Increase width = push right edge right
        match runtime.resize(NavigateDirection::Right, 1) {
            Ok(()) => CommandResult::Success,
            Err(e) => CommandResult::Error(e.to_string()),
        }
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
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        // Decrease width = pull right edge left (negative delta)
        match runtime.resize(NavigateDirection::Right, -1) {
            Ok(()) => CommandResult::Success,
            Err(e) => CommandResult::Error(e.to_string()),
        }
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
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        match runtime.equalize() {
            Ok(()) => CommandResult::Success,
            Err(e) => CommandResult::Error(e.to_string()),
        }
    }
}

// =============================================================================
// Float Zone Commands (#398)
// =============================================================================

/// Toggle window between tiled and floating zones.
#[derive(Debug, Clone, Copy, Default)]
pub struct ToggleFloatCmd;

impl Command for ToggleFloatCmd {
    fn id(&self) -> CommandId {
        ids::TOGGLE_FLOAT
    }

    fn description(&self) -> &'static str {
        "Toggle window between tiled and floating"
    }
}

impl CommandHandler for ToggleFloatCmd {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        match runtime.toggle_float() {
            Ok(()) => CommandResult::Success,
            Err(e) => CommandResult::Error(e.to_string()),
        }
    }
}

/// Raise floating window to front of float zone.
#[derive(Debug, Clone, Copy, Default)]
pub struct RaiseFloatCmd;

impl Command for RaiseFloatCmd {
    fn id(&self) -> CommandId {
        ids::RAISE_FLOAT
    }

    fn description(&self) -> &'static str {
        "Raise floating window to front"
    }
}

impl CommandHandler for RaiseFloatCmd {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        match runtime.raise_float() {
            Ok(()) => CommandResult::Success,
            Err(e) => CommandResult::Error(e.to_string()),
        }
    }
}

/// Lower floating window to back of float zone.
#[derive(Debug, Clone, Copy, Default)]
pub struct LowerFloatCmd;

impl Command for LowerFloatCmd {
    fn id(&self) -> CommandId {
        ids::LOWER_FLOAT
    }

    fn description(&self) -> &'static str {
        "Lower floating window to back"
    }
}

impl CommandHandler for LowerFloatCmd {
    fn execute(&self, runtime: &mut SessionRuntime<'_>, _args: &CommandContext) -> CommandResult {
        match runtime.lower_float() {
            Ok(()) => CommandResult::Success,
            Err(e) => CommandResult::Error(e.to_string()),
        }
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
        // Float zone (#398)
        Box::new(ToggleFloatCmd),
        Box::new(RaiseFloatCmd),
        Box::new(LowerFloatCmd),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_commands_not_empty() {
        let commands = all_commands();
        assert!(!commands.is_empty());
        assert_eq!(commands.len(), 18); // 2 split + 2 close + 4 focus + 2 cycle + 5 resize + 3 float
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
