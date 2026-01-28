//! Clipboard provider registry.

use {
    super::{ClipboardKey, ClipboardProvider},
    reovim_kernel::api::v1::MultiServiceRegistry,
};

/// Registry for clipboard providers, keyed by implementation.
///
/// # Example
///
/// ```ignore
/// use reovim_driver_clipboard::{ClipboardKey, ClipboardProviderRegistry};
///
/// let registry = ClipboardProviderRegistry::new();
/// registry.register(ClipboardKey::Default, Arc::new(my_clipboard_provider));
///
/// let provider = registry.get(&ClipboardKey::Default);
/// ```
pub type ClipboardProviderRegistry = MultiServiceRegistry<ClipboardKey, dyn ClipboardProvider>;
