//! Search provider key - typed key for search provider lookup.

use reovim_kernel::api::v1::ServiceKey;

/// Typed key for search provider lookup.
///
/// Different variants can represent different search strategies.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SearchKey {
    /// Default regex-based search engine.
    Regex,
}

impl ServiceKey for SearchKey {
    fn service_name() -> &'static str {
        "Search"
    }
}

#[cfg(test)]
#[path = "key_tests.rs"]
mod tests;
