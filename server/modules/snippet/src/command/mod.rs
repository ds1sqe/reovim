//! Snippet command handlers (#136).
//!
//! - `ExpandSnippet` - Expand snippet at cursor
//! - `JumpNext` - Jump to next tab stop
//! - `JumpPrev` - Jump to previous tab stop
//! - `CancelSnippet` - Cancel active snippet

mod cancel;
mod expand;
mod jump;

use {reovim_driver_command::CommandHandler, reovim_kernel::api::v1::ModeId};

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
/// (passed as `Arc` during module init). `return_mode` is the mode to
/// transition to when snippet navigation finishes (resolved at init time).
#[must_use]
pub fn all_commands(
    registry: Arc<SnippetRegistry>,
    return_mode: ModeId,
) -> Vec<Box<dyn CommandHandler>> {
    vec![
        Box::new(ExpandSnippet::new(registry)),
        Box::new(JumpNext::new(return_mode.clone())),
        Box::new(JumpPrev),
        Box::new(CancelSnippet::new(return_mode)),
    ]
}

#[cfg(test)]
mod tests {
    use reovim_kernel::api::v1::ModuleId;

    use super::*;

    fn test_return_mode() -> ModeId {
        ModeId::new(ModuleId::new("test"), "return")
    }

    #[test]
    fn test_all_commands_count() {
        let registry = Arc::new(SnippetRegistry::new());
        let commands = all_commands(registry, test_return_mode());
        assert_eq!(commands.len(), 4);
    }

    #[test]
    fn test_all_commands_unique_ids() {
        let registry = Arc::new(SnippetRegistry::new());
        let commands = all_commands(registry, test_return_mode());
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
