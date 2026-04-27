//! Undo provider key - typed key for undo provider lookup.

use reovim_kernel::api::v1::ServiceKey;

/// Typed key for undo provider lookup.
///
/// Currently only one variant, but designed to support future extensions
/// like different undo strategies or scopes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UndoKey {
    /// Per-buffer undo registry (default strategy).
    Buffer,
}

impl ServiceKey for UndoKey {
    fn service_name() -> &'static str {
        "Undo"
    }
}

#[cfg(test)]
#[path = "key_tests.rs"]
mod tests;
