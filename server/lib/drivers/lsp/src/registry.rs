//! LSP provider registry.

use {
    super::{key::LspKey, provider::LspProvider},
    reovim_kernel::api::v1::MultiServiceRegistry,
};

/// Registry for LSP providers, keyed by strategy.
///
/// # Example
///
/// ```ignore
/// use reovim_driver_lsp::{LspKey, LspProviderRegistry};
///
/// let registry = LspProviderRegistry::new();
/// registry.register(LspKey::Default, Arc::new(my_lsp_provider));
///
/// let provider = registry.get(&LspKey::Default);
/// ```
pub type LspProviderRegistry = MultiServiceRegistry<LspKey, dyn LspProvider>;
