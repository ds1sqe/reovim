//! Mode change action types for command results.
//!
//! Provides action intents for mode stack manipulation.

/// Mode change action intent returned by commands.
///
/// Commands return this to request mode changes. The runner handles the
/// actual mode stack manipulation, maintaining separation of concerns.
///
/// # Example
///
/// ```ignore
/// fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
///     // Enter window management mode
///     CommandResult::ModeAction(ModeAction::Push("window".to_string()))
/// }
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModeAction {
    /// Push a new mode onto the stack (e.g., entering window mode).
    Push(String),
    /// Pop the current mode from the stack (return to previous mode).
    Pop,
    /// Replace the current mode with a new one.
    Set(String),
}
