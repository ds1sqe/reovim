//! Vim-specific commands.
//!
//! This module contains commands that are specific to Vim's mode system.
//! These commands require knowledge of Vim mode IDs (INSERT_ID, NORMAL_ID, etc.)
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

mod change;
mod mode;
mod mode_entry;

pub use {
    change::{ChangeLine, ChangeToEndOfLine},
    mode::{
        EnterCommandLineMode, EnterInsertMode, EnterInsertModeAppend, EnterWindowMode,
        ExitCommandLineMode, ExitOperatorPending, ExitToNormal,
    },
    mode_entry::{EnterInsertEndOfLine, EnterInsertFirstNonBlank, OpenLineAbove, OpenLineBelow},
};

use reovim_driver_command::CommandHandler;

/// Get all Vim mode-related commands as boxed trait objects.
#[must_use]
pub fn mode_commands() -> Vec<Box<dyn CommandHandler>> {
    vec![
        // Mode switching
        Box::new(EnterInsertMode),
        Box::new(EnterInsertModeAppend),
        Box::new(ExitToNormal),
        Box::new(EnterWindowMode),
        Box::new(ExitOperatorPending),
        Box::new(EnterCommandLineMode),
        Box::new(ExitCommandLineMode),
        // Mode entry variations
        Box::new(EnterInsertFirstNonBlank),
        Box::new(EnterInsertEndOfLine),
        Box::new(OpenLineBelow),
        Box::new(OpenLineAbove),
        // Change commands
        Box::new(ChangeLine),
        Box::new(ChangeToEndOfLine),
    ]
}
