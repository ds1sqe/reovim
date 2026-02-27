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
        EnterInsertModeAppend, EnterSearchBackward, EnterSearchForward, EnterWindowMode,
        ExitCommandLineMode, ExitToNormal,
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
mod tests {
    use super::*;

    #[test]
    fn test_mode_commands_count() {
        let cmds = mode_commands();
        assert_eq!(cmds.len(), 30);
    }

    #[test]
    fn test_mode_commands_not_empty() {
        let cmds = mode_commands();
        assert!(!cmds.is_empty());
    }

    #[test]
    fn test_mode_commands_all_have_unique_ids() {
        use std::collections::HashSet;
        let cmds = mode_commands();
        let mut ids = HashSet::new();
        for cmd in &cmds {
            let id = cmd.id();
            assert!(ids.insert(id.clone()), "Duplicate command ID: {id:?}");
        }
    }

    #[test]
    fn test_mode_commands_all_have_descriptions() {
        let cmds = mode_commands();
        for cmd in &cmds {
            let desc = cmd.description();
            assert!(!desc.is_empty(), "Empty description for {:?}", cmd.id());
        }
    }

    #[test]
    fn test_mode_commands_contains_enter_insert() {
        use crate::ids;
        let cmds = mode_commands();
        let has_enter_insert = cmds.iter().any(|c| c.id() == ids::ENTER_INSERT);
        assert!(has_enter_insert);
    }

    #[test]
    fn test_mode_commands_contains_exit_insert() {
        use crate::ids;
        let cmds = mode_commands();
        let has_exit_insert = cmds.iter().any(|c| c.id() == ids::EXIT_INSERT);
        assert!(has_exit_insert);
    }

    #[test]
    fn test_mode_commands_contains_dot_repeat() {
        use crate::ids;
        let cmds = mode_commands();
        let has_dot_repeat = cmds.iter().any(|c| c.id() == ids::DOT_REPEAT);
        assert!(has_dot_repeat);
    }

    #[test]
    fn test_mode_commands_contains_enter_commandline() {
        use crate::ids;
        let cmds = mode_commands();
        let has_enter_cmdline = cmds.iter().any(|c| c.id() == ids::ENTER_COMMANDLINE);
        assert!(has_enter_cmdline);
    }

    #[test]
    fn test_mode_commands_contains_execute_find_char() {
        use crate::ids;
        let cmds = mode_commands();
        let has_find_char = cmds.iter().any(|c| c.id() == ids::EXECUTE_FIND_CHAR);
        assert!(has_find_char);
    }

    #[test]
    fn test_mode_commands_contains_change_line() {
        use crate::ids;
        let cmds = mode_commands();
        let has_change_line = cmds.iter().any(|c| c.id() == ids::CHANGE_LINE);
        assert!(has_change_line);
    }

    #[test]
    fn test_mode_commands_contains_change_to_eol() {
        use crate::ids;
        let cmds = mode_commands();
        let has_change_to_eol = cmds.iter().any(|c| c.id() == ids::CHANGE_TO_EOL);
        assert!(has_change_to_eol);
    }

    #[test]
    fn test_mode_commands_contains_open_line_below() {
        use crate::ids;
        let cmds = mode_commands();
        let has_open_line_below = cmds.iter().any(|c| c.id() == ids::OPEN_LINE_BELOW);
        assert!(has_open_line_below);
    }

    #[test]
    fn test_mode_commands_contains_open_line_above() {
        use crate::ids;
        let cmds = mode_commands();
        let has_open_line_above = cmds.iter().any(|c| c.id() == ids::OPEN_LINE_ABOVE);
        assert!(has_open_line_above);
    }

    #[test]
    fn test_mode_commands_contains_search_forward() {
        use crate::ids;
        let cmds = mode_commands();
        let has_search_forward = cmds.iter().any(|c| c.id() == ids::ENTER_SEARCH_FORWARD);
        assert!(has_search_forward);
    }

    #[test]
    fn test_mode_commands_contains_search_backward() {
        use crate::ids;
        let cmds = mode_commands();
        let has_search_backward = cmds.iter().any(|c| c.id() == ids::ENTER_SEARCH_BACKWARD);
        assert!(has_search_backward);
    }

    #[test]
    fn test_mode_commands_contains_enter_window_mode() {
        use crate::ids;
        let cmds = mode_commands();
        let has_enter_window = cmds.iter().any(|c| c.id() == ids::ENTER_WINDOW_MODE);
        assert!(has_enter_window);
    }

    #[test]
    fn test_mode_commands_contains_cancel_to_normal() {
        use crate::ids;
        let cmds = mode_commands();
        let has_cancel = cmds.iter().any(|c| c.id() == ids::CANCEL_TO_NORMAL);
        assert!(has_cancel);
    }
}
