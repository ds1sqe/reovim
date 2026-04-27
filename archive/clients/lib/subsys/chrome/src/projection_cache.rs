//! Projection display cache — keyed by (`DomainId`, `ProjectionTag`).
//!
//! Stores the display string from persistent projections for chrome modules
//! (statusline, tab bar, etc.) that need the latest value without re-parsing
//! opaque content bytes.

use std::collections::HashMap;

use reovim_subsys_coordination::{DomainId, ProjectionTag};

use reovim_client_subsys_codec::projection::DomainProjection;

/// Cache key combining domain identity and projection tag.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
struct CacheKey {
    domain_id: DomainId,
    tag: ProjectionTag,
}

/// Cached projection display data.
///
/// Stores the display string and version for deduplication.
#[derive(Debug, Clone)]
struct CachedEntry {
    display: String,
    version: u64,
}

/// Cache of projection display strings for chrome modules.
///
/// Chrome modules (statusline, tab bar) use the display string from
/// projections to render domain state. This cache stores the latest
/// display string keyed by `(DomainId, ProjectionTag)` so modules can
/// query without re-parsing opaque content bytes.
///
/// # Transient Projections
///
/// Transient projections (fire-and-forget) skip cache updates. They are
/// delivered to modules via `on_projection()` but do not overwrite the
/// cached display string. This prevents flashing or stale state from
/// one-shot notifications.
///
/// # Eviction
///
/// Entries are evicted on:
/// - Window close: all entries for a specific window are removed
/// - Domain detach: all entries for a domain are removed
pub struct ProjectionDisplayCache {
    entries: HashMap<CacheKey, CachedEntry>,
}

impl ProjectionDisplayCache {
    /// Create a new empty cache.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Update the cache with a projection.
    ///
    /// Transient projections are skipped. Persistent projections update the
    /// cached display string if the version is newer.
    pub fn update(&mut self, projection: &DomainProjection) {
        if projection.transient {
            return;
        }

        let key = CacheKey {
            domain_id: projection.domain_id,
            tag: projection.tag.clone(),
        };

        let entry = self.entries.entry(key).or_insert_with(|| CachedEntry {
            display: String::new(),
            version: 0,
        });

        if projection.version >= entry.version {
            entry.display.clone_from(&projection.display);
            entry.version = projection.version;
        }
    }

    /// Get the cached display string for a (`domain_id`, tag) pair.
    #[must_use]
    pub fn get(&self, domain_id: DomainId, tag: &ProjectionTag) -> Option<&str> {
        let key = CacheKey {
            domain_id,
            tag: tag.clone(),
        };
        self.entries.get(&key).map(|e| e.display.as_str())
    }

    /// Evict all entries for a domain (domain detach).
    pub fn evict_domain(&mut self, domain_id: DomainId) {
        self.entries.retain(|k, _| k.domain_id != domain_id);
    }

    /// Evict all entries (window close — clears the full cache for this client).
    pub fn clear(&mut self) {
        self.entries.clear();
    }

    /// Number of cached entries.
    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the cache is empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Get all display strings for a specific domain (for chrome focus filtering).
    pub fn entries_for_domain(
        &self,
        domain_id: DomainId,
    ) -> impl Iterator<Item = (&ProjectionTag, &str)> {
        self.entries
            .iter()
            .filter(move |(k, _)| k.domain_id == domain_id)
            .map(|(k, v)| (&k.tag, v.display.as_str()))
    }
}

impl Default for ProjectionDisplayCache {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
#[path = "projection_cache_tests.rs"]
mod tests;
