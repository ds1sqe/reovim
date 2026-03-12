//! Snippet command handlers (#136, #529).
//!
//! - `ExpandSnippet` - Expand snippet at cursor
//! - `JumpNext` - Jump to next tab stop
//! - `JumpPrev` - Jump to previous tab stop
//! - `CancelSnippet` - Cancel active snippet
//! - `SnippetCatalog` - List available snippets
//! - `ReloadSnippets` - Reload snippet files from disk

mod cancel;
mod catalog;
mod expand;
mod jump;
mod reload;

use std::path::PathBuf;

use {reovim_driver_command::CommandHandler, reovim_kernel::api::v1::ModeId};

pub use {
    cancel::CancelSnippet,
    catalog::SnippetCatalog,
    expand::ExpandSnippet,
    jump::{JumpNext, JumpPrev},
    reload::ReloadSnippets,
};

use crate::provider::SnippetRegistryHandle;

/// Create all snippet command handlers.
///
/// The `ExpandSnippet`, `SnippetCatalog`, and `ReloadSnippets` commands need
/// access to the `SnippetRegistryHandle` for hot-reloadable snippet lookup.
/// `return_mode` is the mode to transition to when snippet navigation finishes.
#[must_use]
pub fn all_commands(
    handle: SnippetRegistryHandle,
    return_mode: ModeId,
    data_dir: PathBuf,
) -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(ExpandSnippet::new(handle.clone())),
        Box::new(SnippetCatalog::new(handle.clone())),
        Box::new(ReloadSnippets::new(handle, data_dir)),
        Box::new(JumpNext::new(return_mode.clone())),
        Box::new(JumpPrev),
        Box::new(CancelSnippet::new(return_mode)),
    ]
}

#[cfg(test)]
#[path = "mod_tests.rs"]
mod tests;
