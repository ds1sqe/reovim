//! Treesitter navigation commands

use reovim_core::declare_event_command;

// === Scope navigation commands ===

declare_event_command! {
    JumpToParentScope,
    id: "jump_to_parent_scope",
    description: "Jump to parent scope header",
}

declare_event_command! {
    JumpToPrevScope,
    id: "jump_to_prev_scope",
    description: "Jump to previous scope header",
}

declare_event_command! {
    JumpToNextScope,
    id: "jump_to_next_scope",
    description: "Jump to next scope header",
}
