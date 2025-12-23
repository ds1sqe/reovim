//! Query compilation and caching
//!
//! Queries are provided by language plugins via the LanguageSupport trait.

use std::{collections::HashMap, sync::Arc};

use tree_sitter::Query;

/// Type of query
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum QueryType {
    /// Syntax highlighting
    Highlights,
    /// Semantic text objects (function, class, parameter, etc.)
    TextObjects,
    /// Code folding regions
    Folds,
    /// Visual decorations (concealment, icons, backgrounds)
    Decorations,
}

/// Cache key for compiled queries
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct QueryKey {
    language_id: String,
    query_type: QueryType,
}

/// Cache for compiled queries
///
/// Queries are compiled on-demand from the source provided by language plugins.
/// Uses Arc<Query> to allow cheap cloning for syntax providers.
pub struct QueryCache {
    queries: HashMap<QueryKey, Arc<Query>>,
}

impl Default for QueryCache {
    fn default() -> Self {
        Self::new()
    }
}

impl QueryCache {
    /// Create a new empty query cache
    #[must_use]
    pub fn new() -> Self {
        Self {
            queries: HashMap::new(),
        }
    }

    /// Get a cached query (does not compile if not cached)
    ///
    /// Returns an Arc clone for cheap sharing with syntax providers.
    #[must_use]
    pub fn get(&self, language_id: &str, query_type: QueryType) -> Option<Arc<Query>> {
        let key = QueryKey {
            language_id: language_id.to_string(),
            query_type,
        };
        self.queries.get(&key).cloned()
    }

    /// Compile and cache a query from source
    ///
    /// Returns an Arc clone of the compiled query, or None if compilation fails.
    pub fn compile_and_cache(
        &mut self,
        language_id: &str,
        query_type: QueryType,
        ts_language: &tree_sitter::Language,
        source: &str,
    ) -> Option<Arc<Query>> {
        let key = QueryKey {
            language_id: language_id.to_string(),
            query_type,
        };

        // Return cached query if available
        if let Some(query) = self.queries.get(&key) {
            return Some(Arc::clone(query));
        }

        // Compile the query
        let query = match Query::new(ts_language, source) {
            Ok(q) => q,
            Err(e) => {
                tracing::error!(
                    language_id = %language_id,
                    query_type = ?query_type,
                    error = %e,
                    "Failed to compile query"
                );
                return None;
            }
        };

        let arc_query = Arc::new(query);
        self.queries.insert(key.clone(), Arc::clone(&arc_query));
        Some(arc_query)
    }

    /// Check if a query is cached
    #[must_use]
    pub fn is_cached(&self, language_id: &str, query_type: QueryType) -> bool {
        let key = QueryKey {
            language_id: language_id.to_string(),
            query_type,
        };
        self.queries.contains_key(&key)
    }

    /// Clear all cached queries
    pub fn clear(&mut self) {
        self.queries.clear();
    }

    /// Clear cached queries for a specific language
    pub fn clear_language(&mut self, language_id: &str) {
        self.queries.retain(|k, _| k.language_id != language_id);
    }
}
