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

    // === Completion ===
    pub const COMPLETION_TRIGGER: CommandId = CommandId::new("completion_trigger");
    pub const COMPLETION_NEXT: CommandId = CommandId::new("completion_next");
    pub const COMPLETION_PREV: CommandId = CommandId::new("completion_prev");
    pub const COMPLETION_CONFIRM: CommandId = CommandId::new("completion_confirm");
    pub const COMPLETION_DISMISS: CommandId = CommandId::new("completion_dismiss");

    // === Explorer Navigation ===
    pub const EXPLORER_CURSOR_UP: CommandId = CommandId::new("explorer_cursor_up");
    pub const EXPLORER_CURSOR_DOWN: CommandId = CommandId::new("explorer_cursor_down");
    pub const EXPLORER_PAGE_UP: CommandId = CommandId::new("explorer_page_up");
    pub const EXPLORER_PAGE_DOWN: CommandId = CommandId::new("explorer_page_down");
    pub const EXPLORER_GOTO_FIRST: CommandId = CommandId::new("explorer_goto_first");
    pub const EXPLORER_GOTO_LAST: CommandId = CommandId::new("explorer_goto_last");

    // === Explorer Tree Operations ===
    pub const EXPLORER_TOGGLE_NODE: CommandId = CommandId::new("explorer_toggle_node");
    pub const EXPLORER_OPEN_NODE: CommandId = CommandId::new("explorer_open_node");
    pub const EXPLORER_CLOSE_PARENT: CommandId = CommandId::new("explorer_close_parent");
    pub const EXPLORER_GO_TO_PARENT: CommandId = CommandId::new("explorer_go_to_parent");
    pub const EXPLORER_REFRESH: CommandId = CommandId::new("explorer_refresh");

    // === Explorer Display ===
    pub const EXPLORER_TOGGLE_HIDDEN: CommandId = CommandId::new("explorer_toggle_hidden");

    // === Explorer Window ===
    pub const EXPLORER_CLOSE: CommandId = CommandId::new("explorer_close");
    pub const EXPLORER_FOCUS_EDITOR: CommandId = CommandId::new("explorer_focus_editor");
    pub const TOGGLE_EXPLORER: CommandId = CommandId::new("toggle_explorer");

    // === Explorer File Operations ===
    pub const EXPLORER_CREATE_FILE: CommandId = CommandId::new("explorer_create_file");
    pub const EXPLORER_CREATE_DIR: CommandId = CommandId::new("explorer_create_dir");
    pub const EXPLORER_RENAME: CommandId = CommandId::new("explorer_rename");
    pub const EXPLORER_DELETE: CommandId = CommandId::new("explorer_delete");

    // === Explorer Filter ===
    pub const EXPLORER_FILTER: CommandId = CommandId::new("explorer_filter");
    pub const EXPLORER_CLEAR_FILTER: CommandId = CommandId::new("explorer_clear_filter");

    // === Explorer Input Mode ===
    pub const EXPLORER_CONFIRM_INPUT: CommandId = CommandId::new("explorer_confirm_input");
    pub const EXPLORER_CANCEL_INPUT: CommandId = CommandId::new("explorer_cancel_input");
    pub const EXPLORER_INPUT_BACKSPACE: CommandId = CommandId::new("explorer_input_backspace");

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

    // === Telescope ===
    pub const TELESCOPE_FIND_FILES: CommandId = CommandId::new("telescope_find_files");
    pub const TELESCOPE_FIND_BUFFERS: CommandId = CommandId::new("telescope_find_buffers");
    pub const TELESCOPE_LIVE_GREP: CommandId = CommandId::new("telescope_live_grep");
    pub const TELESCOPE_RECENT: CommandId = CommandId::new("telescope_recent_files");
    pub const TELESCOPE_COMMANDS: CommandId = CommandId::new("telescope_commands");
    pub const TELESCOPE_HELP: CommandId = CommandId::new("telescope_help_tags");
    pub const TELESCOPE_KEYMAPS: CommandId = CommandId::new("telescope_keymaps");
    pub const TELESCOPE_CLOSE: CommandId = CommandId::new("telescope_close");
    pub const TELESCOPE_CONFIRM: CommandId = CommandId::new("telescope_confirm");
    pub const TELESCOPE_NEXT: CommandId = CommandId::new("telescope_select_next");
    pub const TELESCOPE_PREV: CommandId = CommandId::new("telescope_select_prev");
    pub const TELESCOPE_PAGE_UP: CommandId = CommandId::new("telescope_page_up");
    pub const TELESCOPE_PAGE_DOWN: CommandId = CommandId::new("telescope_page_down");
    pub const TELESCOPE_DELETE_CHAR: CommandId = CommandId::new("telescope_backspace");
    pub const TELESCOPE_GOTO_FIRST: CommandId = CommandId::new("telescope_goto_first");
    pub const TELESCOPE_GOTO_LAST: CommandId = CommandId::new("telescope_goto_last");
    pub const TELESCOPE_ENTER_INSERT: CommandId = CommandId::new("telescope_enter_insert");
    pub const TELESCOPE_ENTER_NORMAL: CommandId = CommandId::new("telescope_enter_normal");

    // === Leap Motion ===
    pub const LEAP_FORWARD: CommandId = CommandId::new("leap_forward");
    pub const LEAP_BACKWARD: CommandId = CommandId::new("leap_backward");
    pub const LEAP_CANCEL: CommandId = CommandId::new("leap_cancel");

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

    // === Tab Management ===
    pub const TAB_NEW: CommandId = CommandId::new("tab_new");
    pub const TAB_CLOSE: CommandId = CommandId::new("tab_close");
    pub const TAB_NEXT: CommandId = CommandId::new("tab_next");
    pub const TAB_PREV: CommandId = CommandId::new("tab_prev");

    // === Settings Menu ===
    pub const SETTINGS_MENU_OPEN: CommandId = CommandId::new("settings_menu_open");
    pub const SETTINGS_MENU_CLOSE: CommandId = CommandId::new("settings_menu_close");
    pub const SETTINGS_MENU_NEXT: CommandId = CommandId::new("settings_menu_next");
    pub const SETTINGS_MENU_PREV: CommandId = CommandId::new("settings_menu_prev");
    pub const SETTINGS_MENU_TOGGLE: CommandId = CommandId::new("settings_menu_toggle");
    pub const SETTINGS_MENU_CYCLE_NEXT: CommandId = CommandId::new("settings_menu_cycle_next");
    pub const SETTINGS_MENU_CYCLE_PREV: CommandId = CommandId::new("settings_menu_cycle_prev");
    pub const SETTINGS_MENU_INCREMENT: CommandId = CommandId::new("settings_menu_increment");
    pub const SETTINGS_MENU_DECREMENT: CommandId = CommandId::new("settings_menu_decrement");
    pub const SETTINGS_MENU_EXECUTE: CommandId = CommandId::new("settings_menu_execute");
    pub const SETTINGS_MENU_QUICK_1: CommandId = CommandId::new("settings_menu_quick_1");
    pub const SETTINGS_MENU_QUICK_2: CommandId = CommandId::new("settings_menu_quick_2");
    pub const SETTINGS_MENU_QUICK_3: CommandId = CommandId::new("settings_menu_quick_3");
    pub const SETTINGS_MENU_QUICK_4: CommandId = CommandId::new("settings_menu_quick_4");
    pub const SETTINGS_MENU_QUICK_5: CommandId = CommandId::new("settings_menu_quick_5");
    pub const SETTINGS_MENU_QUICK_6: CommandId = CommandId::new("settings_menu_quick_6");
    pub const SETTINGS_MENU_QUICK_7: CommandId = CommandId::new("settings_menu_quick_7");
    pub const SETTINGS_MENU_QUICK_8: CommandId = CommandId::new("settings_menu_quick_8");
    pub const SETTINGS_MENU_QUICK_9: CommandId = CommandId::new("settings_menu_quick_9");
}
