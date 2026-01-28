//! Search provider registry.

use {
    super::{SearchKey, SearchProvider},
    reovim_kernel::api::v1::MultiServiceRegistry,
};

/// Registry for search providers, keyed by strategy.
///
/// # Example
///
/// ```ignore
/// use reovim_driver_search::{SearchKey, SearchProviderRegistry};
///
/// let registry = SearchProviderRegistry::new();
/// registry.register(SearchKey::Regex, Arc::new(regex_search_engine));
///
/// let provider = registry.get(&SearchKey::Regex);
/// ```
pub type SearchProviderRegistry = MultiServiceRegistry<SearchKey, dyn SearchProvider>;
