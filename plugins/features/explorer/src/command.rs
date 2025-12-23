//! Explorer commands (unified command-event types)

use reovim_core::{declare_counted_event_command, declare_event_command, event_bus::Event};

// === Counted navigation commands ===

declare_counted_event_command! {
    ExplorerCursorUp,
    id: "explorer_cursor_up",
    description: "Move cursor up",
}

declare_counted_event_command! {
    ExplorerCursorDown,
    id: "explorer_cursor_down",
    description: "Move cursor down",
}

// === Zero-sized navigation commands ===

declare_event_command! {
    ExplorerPageUp,
    id: "explorer_page_up",
    description: "Page up",
}

declare_event_command! {
    ExplorerPageDown,
    id: "explorer_page_down",
    description: "Page down",
}

declare_event_command! {
    ExplorerGotoFirst,
    id: "explorer_goto_first",
    description: "Go to first item",
}

declare_event_command! {
    ExplorerGotoLast,
    id: "explorer_goto_last",
    description: "Go to last item",
}

// === Node operations ===

declare_event_command! {
    ExplorerToggleNode,
    id: "explorer_toggle_node",
    description: "Toggle expand/collapse on current node",
}

declare_event_command! {
    ExplorerOpenNode,
    id: "explorer_open_node",
    description: "Open file or toggle directory",
}

declare_event_command! {
    ExplorerCloseParent,
    id: "explorer_close_parent",
    description: "Close parent directory",
}

declare_event_command! {
    ExplorerGoToParent,
    id: "explorer_goto_parent",
    description: "Go to parent directory",
}

// === Display & refresh ===

declare_event_command! {
    ExplorerRefresh,
    id: "explorer_refresh",
    description: "Refresh tree from filesystem",
}

declare_event_command! {
    ExplorerToggleHidden,
    id: "explorer_toggle_hidden",
    description: "Toggle showing hidden files",
}

declare_event_command! {
    ExplorerToggleSizes,
    id: "explorer_toggle_sizes",
    description: "Toggle showing file sizes",
}

// === Clipboard operations ===

declare_event_command! {
    ExplorerYank,
    id: "explorer_yank",
    description: "Yank (copy) current item to clipboard",
}

declare_event_command! {
    ExplorerCut,
    id: "explorer_cut",
    description: "Cut current item to clipboard",
}

declare_event_command! {
    ExplorerPaste,
    id: "explorer_paste",
    description: "Paste from clipboard",
}

// === Window management ===

declare_event_command! {
    ExplorerClose,
    id: "explorer_close",
    description: "Close explorer (switch to editor)",
}

declare_event_command! {
    ExplorerFocusEditor,
    id: "explorer_focus_editor",
    description: "Focus editor window",
}

declare_event_command! {
    ExplorerToggle,
    id: "explorer_toggle",
    description: "Toggle explorer visibility",
}

// === File operations ===

declare_event_command! {
    ExplorerCreateFile,
    id: "explorer_create_file",
    description: "Start creating a new file",
}

declare_event_command! {
    ExplorerCreateDir,
    id: "explorer_create_dir",
    description: "Start creating a new directory",
}

declare_event_command! {
    ExplorerRename,
    id: "explorer_rename",
    description: "Start renaming current item",
}

declare_event_command! {
    ExplorerDelete,
    id: "explorer_delete",
    description: "Delete current item",
}

// === Filter & search ===

declare_event_command! {
    ExplorerStartFilter,
    id: "explorer_start_filter",
    description: "Start filtering by pattern",
}

declare_event_command! {
    ExplorerClearFilter,
    id: "explorer_clear_filter",
    description: "Clear current filter",
}

declare_event_command! {
    ExplorerToggleGitIgnore,
    id: "explorer_toggle_gitignore",
    description: "Toggle gitignore filtering",
}

// === Advanced operations ===

declare_event_command! {
    ExplorerRevealCurrentFile,
    id: "explorer_reveal_current_file",
    description: "Reveal current file in explorer",
}

declare_event_command! {
    ExplorerCollapseAll,
    id: "explorer_collapse_all",
    description: "Collapse all directories",
}

declare_event_command! {
    ExplorerExpandAll,
    id: "explorer_expand_all",
    description: "Expand all directories",
}

declare_event_command! {
    ExplorerChangeRoot,
    id: "explorer_change_root",
    description: "Change root to selected directory",
}

declare_event_command! {
    ExplorerOpenInSplit,
    id: "explorer_open_in_split",
    description: "Open file in split",
}

declare_event_command! {
    ExplorerOpenInVSplit,
    id: "explorer_open_in_vsplit",
    description: "Open file in vertical split",
}

declare_event_command! {
    ExplorerOpenExternal,
    id: "explorer_open_external",
    description: "Open with external program",
}

declare_event_command! {
    ExplorerShowInfo,
    id: "explorer_show_info",
    description: "Show file/directory info",
}

declare_event_command! {
    ExplorerClosePopup,
    id: "explorer_close_popup",
    description: "Close file details popup",
}

declare_event_command! {
    ExplorerCopyPath,
    id: "explorer_copy_path",
    description: "Copy file path to clipboard",
}

// === Input mode commands ===

declare_event_command! {
    ExplorerConfirmInput,
    id: "explorer_confirm_input",
    description: "Confirm input operation",
}

declare_event_command! {
    ExplorerCancelInput,
    id: "explorer_cancel_input",
    description: "Cancel input operation",
}

/// Insert character in input mode
#[derive(Debug, Clone, Copy)]
pub struct ExplorerInputChar {
    pub c: char,
}

impl ExplorerInputChar {
    #[must_use]
    pub const fn new(c: char) -> Self {
        Self { c }
    }
}

impl Event for ExplorerInputChar {
    fn priority(&self) -> u32 {
        100
    }
}

declare_event_command! {
    ExplorerInputBackspace,
    id: "explorer_input_backspace",
    description: "Delete character in input",
}

// === Visual mode commands ===

declare_event_command! {
    ExplorerVisualMode,
    id: "explorer_visual_mode",
    description: "Enter visual selection mode",
}

declare_event_command! {
    ExplorerToggleSelect,
    id: "explorer_toggle_select",
    description: "Toggle selection of current item",
}

declare_event_command! {
    ExplorerSelectAll,
    id: "explorer_select_all",
    description: "Select all items",
}

declare_event_command! {
    ExplorerExitVisual,
    id: "explorer_exit_visual",
    description: "Exit visual mode",
}
