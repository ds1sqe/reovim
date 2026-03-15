//! Vim-specific commands.
//!
//! This module contains commands that are specific to Vim's mode system.
//! These commands require knowledge of Vim mode IDs (`INSERT_ID`, `NORMAL_ID`, etc.)
//! and are therefore Vim POLICY, not generic editor MECHANISM.
//!
//! # Epic #372 - Mode Ownership
//!
//! Commands that transition between Vim modes belong here, not in the generic
//! editor module, because they need direct access to `VimMode::*_ID` constants.
//!
//! # Structure
//!
//! - `mode` - Mode transition commands (i, a, Escape, Ctrl-W, etc.)
//! - `mode_entry` - Insert mode entry variations (I, A, o, O)
//! - `change` - Change commands that enter insert mode (cc, C)
//! - `find_char` - Find-char motion execution (f, F, t, T via resolver)
//! - `repeat` - Dot repeat command (.)

mod change;
mod find_char;
mod mode;
mod mode_entry;
mod repeat;

pub use {
    change::{ChangeLine, ChangeToEndOfLine},
    find_char::ExecuteFindChar,
    mode::{
        CancelCommandLineMode, CancelToNormal, CmdlineBackspace, CmdlineCompleteNext,
        CmdlineCompletePrev, CmdlineCursorEnd, CmdlineCursorHome, CmdlineCursorLeft,
        CmdlineCursorRight, CmdlineDeleteChar, CmdlineDeleteToStart, CmdlineDeleteWord,
        CmdlineHistoryDown, CmdlineHistoryUp, EnterCommandLineMode, EnterInsertMode,
        EnterInsertModeAppend, EnterReplaceMode, EnterSearchBackward, EnterSearchForward,
        EnterWindowMode, ExitCommandLineMode, ExitToNormal, ReplaceBackspace,
    },
    mode_entry::{EnterInsertEndOfLine, EnterInsertFirstNonBlank, OpenLineAbove, OpenLineBelow},
    repeat::DotRepeat,
};

use reovim_driver_command::CommandHandler;

/// Get all Vim mode-related commands as boxed trait objects.
#[must_use]
pub fn mode_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        // Mode switching
        Box::new(EnterInsertMode),
        Box::new(EnterInsertModeAppend),
        Box::new(EnterReplaceMode),
        Box::new(ReplaceBackspace),
        Box::new(ExitToNormal),
        Box::new(EnterWindowMode),
        Box::new(CancelToNormal),
        Box::new(EnterCommandLineMode),
        Box::new(ExitCommandLineMode),
        Box::new(CancelCommandLineMode),
        // Search mode entry (#435)
        Box::new(EnterSearchForward),
        Box::new(EnterSearchBackward),
        // Mode entry variations
        Box::new(EnterInsertFirstNonBlank),
        Box::new(EnterInsertEndOfLine),
        Box::new(OpenLineBelow),
        Box::new(OpenLineAbove),
        // Change commands
        Box::new(ChangeLine),
        Box::new(ChangeToEndOfLine),
        // Find-char motion execution (Epic #385)
        Box::new(ExecuteFindChar),
        // Dot repeat (Epic #465)
        Box::new(DotRepeat),
        // Command-line editing (#451)
        Box::new(CmdlineCursorLeft),
        Box::new(CmdlineCursorRight),
        Box::new(CmdlineCursorHome),
        Box::new(CmdlineCursorEnd),
        Box::new(CmdlineDeleteChar),
        Box::new(CmdlineBackspace),
        Box::new(CmdlineDeleteWord),
        Box::new(CmdlineDeleteToStart),
        // Command-line history (#451)
        Box::new(CmdlineHistoryUp),
        Box::new(CmdlineHistoryDown),
        // Command-line completion (#451)
        Box::new(CmdlineCompleteNext),
        Box::new(CmdlineCompletePrev),
    ]
}

#[cfg(test)]
mod tests;
