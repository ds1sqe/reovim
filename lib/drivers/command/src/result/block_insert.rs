//! Block insert action types for visual-block mode.
//!
//! Provides action types for visual-block insert operations (I and A in
//! visual-block mode).

/// Block insert action intent returned by visual-block commands.
///
/// Commands return this to request block insert operations. The runner
/// handles the actual text insertion across multiple lines when insert
/// mode exits.
///
/// # Visual-Block Insert
///
/// In Vim, when you press `I` or `A` in visual-block mode:
/// 1. You enter insert mode
/// 2. You type some text
/// 3. When you exit insert mode, that text is replicated on every line
///    of the original block selection
///
/// This action captures the block geometry so the runner knows where
/// to replicate the text.
///
/// # Example
///
/// ```ignore
/// // Visual-block I command implementation
/// fn execute(&self, ctx: &mut KernelContext, args: &CommandContext) -> CommandResult {
///     // Get block selection geometry...
///     CommandResult::BlockInsertAction(BlockInsertAction::InsertStart {
///         start_line: 2,
///         end_line: 5,
///         column: 10,
///     })
/// }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockInsertAction {
    /// Insert at the start column of each line in the block (`I` in visual-block).
    ///
    /// When insert mode exits, accumulated text is inserted at `column` on each
    /// line from `start_line` to `end_line` inclusive.
    InsertStart {
        /// First line of the block (0-indexed).
        start_line: usize,
        /// Last line of the block (0-indexed, inclusive).
        end_line: usize,
        /// Column where text will be inserted (left edge of block).
        column: usize,
    },
    /// Insert at the end column of each line in the block (`A` in visual-block).
    ///
    /// When insert mode exits, accumulated text is inserted after `column` on each
    /// line from `start_line` to `end_line` inclusive.
    InsertEnd {
        /// First line of the block (0-indexed).
        start_line: usize,
        /// Last line of the block (0-indexed, inclusive).
        end_line: usize,
        /// Column after which text will be inserted (right edge of block + 1).
        column: usize,
    },
}
