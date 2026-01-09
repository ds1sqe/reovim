//! Query compilation and caching
//!
//! Queries are provided by language plugins via the LanguageSupport trait.

use std::{
    collections::HashMap,
    sync::{Arc, RwLock},
};

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
    /// Language injections (embedded code in markdown, etc.)
    Injections,
    /// Context/scope detection (functions, classes, headings)
    Context,
}

/// Cache key for compiled queries
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct QueryKey {
    language_id: String,
    query_type: QueryType,
}

/// Cache for compiled queries
///
/// Queries are compiled lazily on first access from the source provided by language plugins.
/// Uses interior mutability (RwLock) to allow lazy compilation through `&self` references.
/// Uses Arc<Query> to allow cheap cloning for syntax providers.
pub struct QueryCache {
    queries: RwLock<HashMap<QueryKey, Arc<Query>>>,
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
            queries: RwLock::new(HashMap::new()),
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
        self.queries.read().unwrap().get(&key).cloned()
    }

    /// Compile and cache a query from source
    ///
    /// Returns an Arc clone of the compiled query, or None if compilation fails.
    /// Uses interior mutability via RwLock.
    pub fn compile_and_cache(
        &self,
        language_id: &str,
        query_type: QueryType,
        ts_language: &tree_sitter::Language,
        source: &str,
    ) -> Option<Arc<Query>> {
        let key = QueryKey {
            language_id: language_id.to_string(),
            query_type,
        };

        // Return cached query if available (fast path with read lock)
        if let Some(query) = self.queries.read().unwrap().get(&key) {
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
        self.queries
            .write()
            .unwrap()
            .insert(key, Arc::clone(&arc_query));
        tracing::debug!(
            language_id = %language_id,
            query_type = ?query_type,
            "Compiled and cached query"
        );
        Some(arc_query)
    }

    /// Get a cached query, or compile and cache it on first access (lazy compilation)
    ///
    /// This is the primary method for lazy query access. Checks the cache first
    /// with a read lock, then compiles with a write lock if not cached.
    pub fn get_or_compile(
        &self,
        language_id: &str,
        query_type: QueryType,
        ts_language: &tree_sitter::Language,
        source: &str,
    ) -> Option<Arc<Query>> {
        let key = QueryKey {
            language_id: language_id.to_string(),
            query_type,
        };

        // Fast path: check read lock first
        if let Some(query) = self.queries.read().unwrap().get(&key) {
            return Some(Arc::clone(query));
        }

        // Slow path: compile and cache with write lock
        self.compile_and_cache(language_id, query_type, ts_language, source)
    }

    /// Check if a query is cached
    #[must_use]
    pub fn is_cached(&self, language_id: &str, query_type: QueryType) -> bool {
        let key = QueryKey {
            language_id: language_id.to_string(),
            query_type,
        };
        self.queries.read().unwrap().contains_key(&key)
    }

    /// Clear all cached queries
    pub fn clear(&self) {
        self.queries.write().unwrap().clear();
    }

    /// Clear cached queries for a specific language
    pub fn clear_language(&self, language_id: &str) {
        self.queries
            .write()
            .unwrap()
            .retain(|k, _| k.language_id != language_id);
    }
}
