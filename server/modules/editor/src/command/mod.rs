//! Editor commands - cursor movement and text operations.
//!
//! This module provides the basic commands for editor operation:
//! - Cursor movement: up, down, left, right
//! - Display line movement: gj, gk
//! - Insert mode edits: newline, tab
//! - Delete operations: x, X, dd, D
//! - Replace operations: r, .
//! - Undo/redo: u, Ctrl-R
//! - Yank: yy, Y
//! - Paste: p, P
//! - File operations: :w
//!
//! # Mode Commands
//!
//! Mode-specific commands (enter insert, exit to normal, etc.) are in
//! `reovim-module-vim::commands` because they require `VimMode` constants.
//!
//! # Cursor Movement Philosophy
//!
//! Cursor movement commands follow Vim semantics:
//! - j/k (down/up) preserve the "preferred column" - the column the user
//!   intended, even if shorter lines force temporary repositioning
//! - h/l (left/right) clear the preferred column
//! - Movements clamp to valid positions (no-op at boundaries)

mod cursor;
mod delete;
mod display_line;
mod file;
mod insert_edit;
mod mark;
mod operators;
mod paste;
mod replace;
mod undo;
mod yank;

// Re-export all command types for external use
pub use {
    cursor::{CursorDown, CursorLeft, CursorRight, CursorUp},
    delete::{DeleteChar, DeleteCharBefore, DeleteLine, DeleteToEndOfLine},
    display_line::{CursorDisplayDown, CursorDisplayUp},
    file::WriteBufferCommand,
    insert_edit::{InsertNewline, InsertTab, get_line_indent},
    mark::{GotoMarkExact, GotoMarkLine, SetMark},
    operators::{
        EnterChangeOperator, EnterDedentOperator, EnterDeleteOperator, EnterIndentOperator,
        EnterYankOperator,
    },
    paste::{PasteAfter, PasteBefore},
    replace::{JoinLines, RepeatDot, ReplaceChar, ReplaceCharStart},
    undo::{RedoCommand, UndoCommand},
    yank::YankLine,
};

use reovim_driver_command::CommandHandler;

// =============================================================================
// Command Registration Helper
// =============================================================================

/// Get all editor commands as boxed trait objects.
///
/// This is useful for registering all commands at once.
/// Note: Mode commands (enter insert, etc.) are in the vim module.
#[must_use]
pub fn all_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        // Cursor movement
        Box::new(CursorUp),
        Box::new(CursorDown),
        Box::new(CursorLeft),
        Box::new(CursorRight),
        // Display line movement
        Box::new(CursorDisplayDown),
        Box::new(CursorDisplayUp),
        // Insert mode edits
        Box::new(InsertNewline),
        Box::new(InsertTab),
        // Enter operator commands
        Box::new(EnterDeleteOperator),
        Box::new(EnterYankOperator),
        Box::new(EnterChangeOperator),
        Box::new(EnterIndentOperator),
        Box::new(EnterDedentOperator),
        // Delete operations
        Box::new(DeleteChar),
        Box::new(DeleteCharBefore),
        Box::new(DeleteLine),
        Box::new(DeleteToEndOfLine),
        Box::new(JoinLines),
        // Yank
        Box::new(YankLine),
        // Paste
        Box::new(PasteAfter),
        Box::new(PasteBefore),
        // Replace
        Box::new(ReplaceCharStart),
        Box::new(ReplaceChar),
        // Repeat
        Box::new(RepeatDot),
        // Undo/redo
        Box::new(UndoCommand),
        Box::new(RedoCommand),
        // File operations
        Box::new(WriteBufferCommand),
        // Mark operations
        Box::new(SetMark),
        Box::new(GotoMarkLine),
        Box::new(GotoMarkExact),
    ]
}

/// Get all cursor movement commands.
#[must_use]
pub fn cursor_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(CursorUp),
        Box::new(CursorDown),
        Box::new(CursorLeft),
        Box::new(CursorRight),
    ]
}

/// Get display line movement commands (gj, gk).
#[must_use]
pub fn display_line_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![Box::new(CursorDisplayDown), Box::new(CursorDisplayUp)]
}

/// Get all insert mode edit commands.
#[must_use]
pub fn insert_edit_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![Box::new(InsertNewline), Box::new(InsertTab)]
}

/// Get all delete commands.
#[must_use]
pub fn delete_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(DeleteChar),
        Box::new(DeleteCharBefore),
        Box::new(DeleteLine),
        Box::new(DeleteToEndOfLine),
        Box::new(JoinLines),
    ]
}

/// Get all undo/redo commands.
#[must_use]
pub fn undo_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![Box::new(UndoCommand), Box::new(RedoCommand)]
}

/// Get all yank commands.
#[must_use]
pub fn yank_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![Box::new(YankLine)]
}

/// Get all paste commands.
#[must_use]
pub fn paste_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![Box::new(PasteAfter), Box::new(PasteBefore)]
}

/// Get all mark commands.
#[must_use]
pub fn mark_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(SetMark),
        Box::new(GotoMarkLine),
        Box::new(GotoMarkExact),
    ]
}

#[cfg(test)]
mod tests;
