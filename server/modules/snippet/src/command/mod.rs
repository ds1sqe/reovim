//! Snippet command handlers (#136).
//!
//! - `ExpandSnippet` - Expand snippet at cursor
//! - `JumpNext` - Jump to next tab stop
//! - `JumpPrev` - Jump to previous tab stop
//! - `CancelSnippet` - Cancel active snippet

mod cancel;
mod expand;
mod jump;

use reovim_driver_command::CommandHandler;

pub use {
    cancel::CancelSnippet,
    expand::ExpandSnippet,
    jump::{JumpNext, JumpPrev},
};

use std::sync::Arc;

use crate::provider::SnippetRegistry;

/// Create all snippet command handlers.
///
/// The `ExpandSnippet` command needs access to the `SnippetRegistry`
/// (passed as `Arc` during module init).
#[must_use]
pub fn all_commands(registry: Arc<SnippetRegistry>) -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(ExpandSnippet::new(registry)),
        Box::new(JumpNext),
        Box::new(JumpPrev),
        Box::new(CancelSnippet),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_all_commands_count() {
        let registry = Arc::new(SnippetRegistry::new());
        let commands = all_commands(registry);
        assert_eq!(commands.len(), 4);
    }

    #[test]
    fn test_all_commands_unique_ids() {
        let registry = Arc::new(SnippetRegistry::new());
        let commands = all_commands(registry);
        let ids: Vec<_> = commands.iter().map(|c| c.id()).collect();
        for (i, a) in ids.iter().enumerate() {
            for (j, b) in ids.iter().enumerate() {
                if i != j {
                    assert_ne!(a, b, "command ids {i} and {j} should be unique");
                }
            }
        }
    }
}
