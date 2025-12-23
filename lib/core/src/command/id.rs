//! Command identifier types for the registry system

use std::fmt;

/// Unique identifier for a registered command
///
/// Command IDs are string-based identifiers used to reference
/// commands in the registry and keymaps.
#[derive(Clone, PartialEq, Eq, Hash)]
pub struct CommandId(pub &'static str);

impl CommandId {
    /// Create a new command ID
    #[must_use]
    pub const fn new(name: &'static str) -> Self {
        Self(name)
    }

    /// Get the string representation of this ID
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        self.0
    }
}

impl fmt::Debug for CommandId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "CommandId({})", self.0)
    }
}

impl fmt::Display for CommandId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// Predefined command IDs for built-in commands
pub mod builtin {
    use super::CommandId;

    // === Cursor Movement ===
    pub const CURSOR_UP: CommandId = CommandId::new("cursor_up");
    pub const CURSOR_DOWN: CommandId = CommandId::new("cursor_down");
    pub const CURSOR_LEFT: CommandId = CommandId::new("cursor_left");
    pub const CURSOR_RIGHT: CommandId = CommandId::new("cursor_right");
    pub const CURSOR_LINE_START: CommandId = CommandId::new("cursor_line_start");
    pub const CURSOR_LINE_END: CommandId = CommandId::new("cursor_line_end");
    pub const CURSOR_WORD_FORWARD: CommandId = CommandId::new("cursor_word_forward");
    pub const CURSOR_WORD_BACKWARD: CommandId = CommandId::new("cursor_word_backward");
    pub const CURSOR_WORD_END: CommandId = CommandId::new("cursor_word_end");
    pub const GOTO_FIRST_LINE: CommandId = CommandId::new("goto_first_line");
    pub const GOTO_LAST_LINE: CommandId = CommandId::new("goto_last_line");

    // === Mode Switching ===
    pub const ENTER_NORMAL_MODE: CommandId = CommandId::new("enter_normal_mode");
    pub const ENTER_INSERT_MODE: CommandId = CommandId::new("enter_insert_mode");
    pub const ENTER_INSERT_MODE_AFTER: CommandId = CommandId::new("enter_insert_mode_after");
    pub const ENTER_INSERT_MODE_EOL: CommandId = CommandId::new("enter_insert_mode_eol");
    pub const OPEN_LINE_BELOW: CommandId = CommandId::new("open_line_below");
    pub const OPEN_LINE_ABOVE: CommandId = CommandId::new("open_line_above");
    pub const ENTER_VISUAL_MODE: CommandId = CommandId::new("enter_visual_mode");
    pub const ENTER_VISUAL_BLOCK_MODE: CommandId = CommandId::new("enter_visual_block_mode");
    pub const ENTER_VISUAL_LINE_MODE: CommandId = CommandId::new("enter_visual_line_mode");
    pub const ENTER_COMMAND_MODE: CommandId = CommandId::new("enter_command_mode");

    // === Text Operations ===
    pub const INSERT_CHAR: CommandId = CommandId::new("insert_char");
    pub const INSERT_NEWLINE: CommandId = CommandId::new("insert_newline");
    pub const DELETE_CHAR_BACKWARD: CommandId = CommandId::new("delete_char_backward");
    pub const DELETE_CHAR_FORWARD: CommandId = CommandId::new("delete_char_forward");
    pub const DELETE_LINE: CommandId = CommandId::new("delete_line");
    pub const CHANGE_LINE: CommandId = CommandId::new("change_line");

    // === Visual Mode ===
    pub const VISUAL_EXTEND_UP: CommandId = CommandId::new("visual_extend_up");
    pub const VISUAL_EXTEND_DOWN: CommandId = CommandId::new("visual_extend_down");
    pub const VISUAL_EXTEND_LEFT: CommandId = CommandId::new("visual_extend_left");
    pub const VISUAL_EXTEND_RIGHT: CommandId = CommandId::new("visual_extend_right");
    pub const VISUAL_DELETE: CommandId = CommandId::new("visual_delete");
    pub const VISUAL_YANK: CommandId = CommandId::new("visual_yank");

    // === Command Line ===
    pub const COMMAND_LINE_CHAR: CommandId = CommandId::new("command_line_char");
    pub const COMMAND_LINE_BACKSPACE: CommandId = CommandId::new("command_line_backspace");
    pub const COMMAND_LINE_EXECUTE: CommandId = CommandId::new("command_line_execute");
    pub const COMMAND_LINE_CANCEL: CommandId = CommandId::new("command_line_cancel");

    // === Clipboard ===
    pub const PASTE: CommandId = CommandId::new("paste");
    pub const PASTE_BEFORE: CommandId = CommandId::new("paste_before");

    // === System ===
    pub const QUIT: CommandId = CommandId::new("quit");
    pub const NOOP: CommandId = CommandId::new("noop");

    // Completion commands are now in reovim-plugin-completion crate

    // === Jump List ===
    pub const JUMP_OLDER: CommandId = CommandId::new("jump_older");
    pub const JUMP_NEWER: CommandId = CommandId::new("jump_newer");

    // === Undo/Redo ===
    pub const UNDO: CommandId = CommandId::new("undo");
    pub const REDO: CommandId = CommandId::new("redo");

    // === Yank ===
    pub const YANK_LINE: CommandId = CommandId::new("yank_line");
    pub const YANK_TO_END: CommandId = CommandId::new("yank_to_end");

    // === Operators ===
    pub const ENTER_DELETE_OPERATOR: CommandId = CommandId::new("enter_delete_operator");
    pub const ENTER_YANK_OPERATOR: CommandId = CommandId::new("enter_yank_operator");
    pub const ENTER_CHANGE_OPERATOR: CommandId = CommandId::new("enter_change_operator");

    // === Folding ===
    pub const FOLD_TOGGLE: CommandId = CommandId::new("fold_toggle");
    pub const FOLD_OPEN: CommandId = CommandId::new("fold_open");
    pub const FOLD_CLOSE: CommandId = CommandId::new("fold_close");
    pub const FOLD_OPEN_ALL: CommandId = CommandId::new("fold_open_all");
    pub const FOLD_CLOSE_ALL: CommandId = CommandId::new("fold_close_all");

    // === Window Navigation ===
    pub const WINDOW_FOCUS_LEFT: CommandId = CommandId::new("window_focus_left");
    pub const WINDOW_FOCUS_DOWN: CommandId = CommandId::new("window_focus_down");
    pub const WINDOW_FOCUS_UP: CommandId = CommandId::new("window_focus_up");
    pub const WINDOW_FOCUS_RIGHT: CommandId = CommandId::new("window_focus_right");

    // === Window Movement ===
    pub const WINDOW_MOVE_LEFT: CommandId = CommandId::new("window_move_left");
    pub const WINDOW_MOVE_DOWN: CommandId = CommandId::new("window_move_down");
    pub const WINDOW_MOVE_UP: CommandId = CommandId::new("window_move_up");
    pub const WINDOW_MOVE_RIGHT: CommandId = CommandId::new("window_move_right");

    // === Window Splits ===
    pub const WINDOW_SPLIT_HORIZONTAL: CommandId = CommandId::new("window_split_horizontal");
    pub const WINDOW_SPLIT_VERTICAL: CommandId = CommandId::new("window_split_vertical");
    pub const WINDOW_CLOSE: CommandId = CommandId::new("window_close");
    pub const WINDOW_ONLY: CommandId = CommandId::new("window_only");
    pub const WINDOW_EQUALIZE: CommandId = CommandId::new("window_equalize");

    // === Window Mode (Ctrl-W) ===
    pub const ENTER_WINDOW_MODE: CommandId = CommandId::new("enter_window_mode");
    pub const WINDOW_MODE_FOCUS_LEFT: CommandId = CommandId::new("window_mode_focus_left");
    pub const WINDOW_MODE_FOCUS_DOWN: CommandId = CommandId::new("window_mode_focus_down");
    pub const WINDOW_MODE_FOCUS_UP: CommandId = CommandId::new("window_mode_focus_up");
    pub const WINDOW_MODE_FOCUS_RIGHT: CommandId = CommandId::new("window_mode_focus_right");
    pub const WINDOW_MODE_MOVE_LEFT: CommandId = CommandId::new("window_mode_move_left");
    pub const WINDOW_MODE_MOVE_DOWN: CommandId = CommandId::new("window_mode_move_down");
    pub const WINDOW_MODE_MOVE_UP: CommandId = CommandId::new("window_mode_move_up");
    pub const WINDOW_MODE_MOVE_RIGHT: CommandId = CommandId::new("window_mode_move_right");
    pub const WINDOW_MODE_SWAP_LEFT: CommandId = CommandId::new("window_mode_swap_left");
    pub const WINDOW_MODE_SWAP_DOWN: CommandId = CommandId::new("window_mode_swap_down");
    pub const WINDOW_MODE_SWAP_UP: CommandId = CommandId::new("window_mode_swap_up");
    pub const WINDOW_MODE_SWAP_RIGHT: CommandId = CommandId::new("window_mode_swap_right");
    pub const WINDOW_MODE_SPLIT_H: CommandId = CommandId::new("window_mode_split_h");
    pub const WINDOW_MODE_SPLIT_V: CommandId = CommandId::new("window_mode_split_v");
    pub const WINDOW_MODE_CLOSE: CommandId = CommandId::new("window_mode_close");
    pub const WINDOW_MODE_ONLY: CommandId = CommandId::new("window_mode_only");
    pub const WINDOW_MODE_EQUALIZE: CommandId = CommandId::new("window_mode_equalize");

    // === Tab Management ===
    pub const TAB_NEW: CommandId = CommandId::new("tab_new");
    pub const TAB_CLOSE: CommandId = CommandId::new("tab_close");
    pub const TAB_NEXT: CommandId = CommandId::new("tab_next");
    pub const TAB_PREV: CommandId = CommandId::new("tab_prev");

    // === Buffer Navigation ===
    pub const BUFFER_PREV: CommandId = CommandId::new("buffer_prev");
    pub const BUFFER_NEXT: CommandId = CommandId::new("buffer_next");
    pub const BUFFER_DELETE: CommandId = CommandId::new("buffer_delete");

    // Settings Menu commands are registered by the settings-menu plugin (reovim-plugin-settings-menu)
}
