//! Window management action types for command results.
//!
//! Provides action intents for window operations like splitting, closing,
//! focusing, and resizing.

use reovim_driver_display::NavigateDirection;

/// Window management action intent returned by commands.
///
/// Commands return this to request window operations. The runner handles
/// the actual window creation, layout updates, and focus changes,
/// maintaining separation of concerns between command (policy) and
/// runner (mechanism).
///
/// # Example
///
/// ```ignore
/// fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
///     CommandResult::WindowAction(WindowAction::SplitVertical)
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowAction {
    /// Split current window horizontally (top/bottom).
    SplitHorizontal,
    /// Split current window vertically (left/right).
    SplitVertical,
    /// Close the current window.
    CloseWindow,
    /// Close all windows except current.
    CloseOthers,
    /// Focus window in the specified direction.
    FocusDirection(NavigateDirection),
    /// Cycle focus to the next window.
    CycleForward,
    /// Cycle focus to the previous window.
    CycleBackward,
    /// Increase window height.
    ResizeHeightIncrease,
    /// Decrease window height.
    ResizeHeightDecrease,
    /// Increase window width.
    ResizeWidthIncrease,
    /// Decrease window width.
    ResizeWidthDecrease,
    /// Equalize all window sizes.
    ResizeEqual,
}
