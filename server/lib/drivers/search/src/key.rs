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
mod tests {
    use super::*;

    #[test]
    fn service_name() {
        assert_eq!(SearchKey::service_name(), "Search");
    }

    #[test]
    fn debug() {
        assert_eq!(format!("{:?}", SearchKey::Regex), "Regex");
    }

    #[test]
    fn clone_copy_eq() {
        let a = SearchKey::Regex;
        let b = a;
        assert_eq!(a, b);
    }

    #[test]
    fn hash() {
        use std::collections::HashSet;
        let mut set = HashSet::new();
        set.insert(SearchKey::Regex);
        set.insert(SearchKey::Regex);
        assert_eq!(set.len(), 1);
    }
}
