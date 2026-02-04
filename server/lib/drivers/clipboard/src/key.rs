//! Clipboard provider key - typed key for clipboard provider lookup.

use reovim_kernel::api::v1::ServiceKey;

/// Typed key for clipboard provider lookup.
///
/// Currently only one variant, but designed to support future extensions
/// like different clipboard backends or scopes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum ClipboardKey {
    /// Default clipboard provider (system clipboard + history).
    Default,
}

impl ServiceKey for ClipboardKey {
    fn service_name() -> &'static str {
        "Clipboard"
    }
}
