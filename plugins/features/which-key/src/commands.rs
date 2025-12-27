//! Which-key commands

use reovim_core::declare_event_command;

// Re-export WhichKeyOpen from core (shared type used by CommandHandler and plugin)
pub use reovim_core::which_key::WhichKeyOpen;

declare_event_command! {
    WhichKeyClose,
    id: "which_key_close",
    description: "Close which-key panel",
}

declare_event_command! {
    WhichKeyBackspace,
    id: "which_key_backspace",
    description: "Remove last key from filter",
}
