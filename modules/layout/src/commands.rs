//! Window management commands.
//!
//! Commands for window splitting, closing, focus navigation, and resizing.
//! Each command returns a `WindowAction` that the runner handles.
//!
//! # Architecture
//!
//! Following the "mechanism vs policy" principle:
//! - **Mechanism** (runner): `WindowRegistry`, actual window state management
//! - **Policy** (this module): Commands declare WHAT action is needed
//!
//! Commands don't directly manipulate windows. They return `CommandResult::WindowAction`
//! to signal intent, and the runner handles the actual state changes.

use {
    reovim_driver_command::{Command, CommandContext, CommandHandler, CommandResult, WindowAction},
    reovim_driver_display::NavigateDirection,
    reovim_kernel::api::v1::{CommandId, KernelContext},
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
    fn execute(&self, _ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
        CommandResult::WindowAction(WindowAction::SplitHorizontal)
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
    fn execute(&self, _ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
        CommandResult::WindowAction(WindowAction::SplitVertical)
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
    fn execute(&self, _ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
        CommandResult::WindowAction(WindowAction::CloseWindow)
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
    fn execute(&self, _ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
        CommandResult::WindowAction(WindowAction::CloseOthers)
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
    fn execute(&self, _ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
        CommandResult::WindowAction(WindowAction::FocusDirection(NavigateDirection::Left))
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
    fn execute(&self, _ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
        CommandResult::WindowAction(WindowAction::FocusDirection(NavigateDirection::Right))
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
    fn execute(&self, _ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
        CommandResult::WindowAction(WindowAction::FocusDirection(NavigateDirection::Up))
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
    fn execute(&self, _ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
        CommandResult::WindowAction(WindowAction::FocusDirection(NavigateDirection::Down))
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
    fn execute(&self, _ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
        CommandResult::WindowAction(WindowAction::CycleForward)
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
    fn execute(&self, _ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
        CommandResult::WindowAction(WindowAction::CycleBackward)
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
    fn execute(&self, _ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
        CommandResult::WindowAction(WindowAction::ResizeHeightIncrease)
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
    fn execute(&self, _ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
        CommandResult::WindowAction(WindowAction::ResizeHeightDecrease)
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
    fn execute(&self, _ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
        CommandResult::WindowAction(WindowAction::ResizeWidthIncrease)
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
    fn execute(&self, _ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
        CommandResult::WindowAction(WindowAction::ResizeWidthDecrease)
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
    fn execute(&self, _ctx: &mut KernelContext, _args: &CommandContext) -> CommandResult {
        CommandResult::WindowAction(WindowAction::ResizeEqual)
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
    fn test_split_horizontal_cmd_returns_window_action() {
        let cmd = SplitHorizontalCmd;
        let mut ctx = KernelContext::default();
        let args = CommandContext::new();

        let result = cmd.execute(&mut ctx, &args);
        assert!(result.is_window_action());
        assert!(matches!(result, CommandResult::WindowAction(WindowAction::SplitHorizontal)));
    }

    #[test]
    fn test_split_vertical_cmd_returns_window_action() {
        let cmd = SplitVerticalCmd;
        let mut ctx = KernelContext::default();
        let args = CommandContext::new();

        let result = cmd.execute(&mut ctx, &args);
        assert!(result.is_window_action());
        assert!(matches!(result, CommandResult::WindowAction(WindowAction::SplitVertical)));
    }

    #[test]
    fn test_close_window_cmd_returns_window_action() {
        let cmd = CloseWindowCmd;
        let mut ctx = KernelContext::default();
        let args = CommandContext::new();

        let result = cmd.execute(&mut ctx, &args);
        assert!(result.is_window_action());
        assert!(matches!(result, CommandResult::WindowAction(WindowAction::CloseWindow)));
    }

    #[test]
    fn test_close_others_cmd_returns_window_action() {
        let cmd = CloseOthersCmd;
        let mut ctx = KernelContext::default();
        let args = CommandContext::new();

        let result = cmd.execute(&mut ctx, &args);
        assert!(result.is_window_action());
        assert!(matches!(result, CommandResult::WindowAction(WindowAction::CloseOthers)));
    }

    #[test]
    fn test_focus_left_cmd_returns_window_action() {
        let cmd = FocusLeftCmd;
        let mut ctx = KernelContext::default();
        let args = CommandContext::new();

        let result = cmd.execute(&mut ctx, &args);
        assert!(result.is_window_action());
        assert!(matches!(
            result,
            CommandResult::WindowAction(WindowAction::FocusDirection(NavigateDirection::Left))
        ));
    }

    #[test]
    fn test_focus_right_cmd_returns_window_action() {
        let cmd = FocusRightCmd;
        let mut ctx = KernelContext::default();
        let args = CommandContext::new();

        let result = cmd.execute(&mut ctx, &args);
        assert!(result.is_window_action());
        assert!(matches!(
            result,
            CommandResult::WindowAction(WindowAction::FocusDirection(NavigateDirection::Right))
        ));
    }

    #[test]
    fn test_focus_up_cmd_returns_window_action() {
        let cmd = FocusUpCmd;
        let mut ctx = KernelContext::default();
        let args = CommandContext::new();

        let result = cmd.execute(&mut ctx, &args);
        assert!(result.is_window_action());
        assert!(matches!(
            result,
            CommandResult::WindowAction(WindowAction::FocusDirection(NavigateDirection::Up))
        ));
    }

    #[test]
    fn test_focus_down_cmd_returns_window_action() {
        let cmd = FocusDownCmd;
        let mut ctx = KernelContext::default();
        let args = CommandContext::new();

        let result = cmd.execute(&mut ctx, &args);
        assert!(result.is_window_action());
        assert!(matches!(
            result,
            CommandResult::WindowAction(WindowAction::FocusDirection(NavigateDirection::Down))
        ));
    }

    #[test]
    fn test_cycle_forward_cmd_returns_window_action() {
        let cmd = CycleForwardCmd;
        let mut ctx = KernelContext::default();
        let args = CommandContext::new();

        let result = cmd.execute(&mut ctx, &args);
        assert!(result.is_window_action());
        assert!(matches!(result, CommandResult::WindowAction(WindowAction::CycleForward)));
    }

    #[test]
    fn test_cycle_backward_cmd_returns_window_action() {
        let cmd = CycleBackwardCmd;
        let mut ctx = KernelContext::default();
        let args = CommandContext::new();

        let result = cmd.execute(&mut ctx, &args);
        assert!(result.is_window_action());
        assert!(matches!(result, CommandResult::WindowAction(WindowAction::CycleBackward)));
    }

    #[test]
    fn test_resize_height_increase_cmd_returns_window_action() {
        let cmd = ResizeHeightIncreaseCmd;
        let mut ctx = KernelContext::default();
        let args = CommandContext::new();

        let result = cmd.execute(&mut ctx, &args);
        assert!(result.is_window_action());
        assert!(matches!(
            result,
            CommandResult::WindowAction(WindowAction::ResizeHeightIncrease)
        ));
    }

    #[test]
    fn test_resize_equal_cmd_returns_window_action() {
        let cmd = ResizeEqualCmd;
        let mut ctx = KernelContext::default();
        let args = CommandContext::new();

        let result = cmd.execute(&mut ctx, &args);
        assert!(result.is_window_action());
        assert!(matches!(result, CommandResult::WindowAction(WindowAction::ResizeEqual)));
    }

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
