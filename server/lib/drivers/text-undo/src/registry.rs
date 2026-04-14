//! Undo provider registry.

use {
    super::{UndoKey, UndoProvider},
    reovim_kernel::api::v1::MultiServiceRegistry,
};

/// Registry for undo providers, keyed by strategy.
///
/// # Example
///
/// ```ignore
/// use reovim_driver_text_undo::{UndoKey, UndoProviderRegistry};
///
/// let registry = UndoProviderRegistry::new();
/// registry.register(UndoKey::Buffer, Arc::new(my_undo_provider));
///
/// let provider = registry.get(&UndoKey::Buffer);
/// ```
pub type UndoProviderRegistry = MultiServiceRegistry<UndoKey, dyn UndoProvider>;
