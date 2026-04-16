//! Projection display cache — keyed by (DomainId, ProjectionTag).
//!
//! Stores the display string from persistent projections for chrome modules
//! (statusline, tab bar, etc.) that need the latest value without re-parsing
//! opaque content bytes.

use std::collections::HashMap;

use reovim_subsys_coordination::{DomainId, ProjectionTag};

use super::DomainProjection;

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

    /// Get the cached display string for a (domain_id, tag) pair.
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
mod tests {
    use super::*;

    fn make_projection(tag: &str, domain_id: u32, display: &str, version: u64) -> DomainProjection {
        DomainProjection {
            tag: ProjectionTag::new(tag),
            domain_id: DomainId(domain_id),
            window_id: None,
            content: vec![],
            display: display.to_string(),
            transient: false,
            version,
            client_id: 0,
        }
    }

    fn make_transient(tag: &str, domain_id: u32, display: &str) -> DomainProjection {
        DomainProjection {
            tag: ProjectionTag::new(tag),
            domain_id: DomainId(domain_id),
            window_id: None,
            content: vec![],
            display: display.to_string(),
            transient: true,
            version: 0,
            client_id: 0,
        }
    }

    #[test]
    fn cache_miss_returns_none() {
        let cache = ProjectionDisplayCache::new();
        assert!(
            cache
                .get(DomainId(1), &ProjectionTag::new("text.mode"))
                .is_none()
        );
    }

    #[test]
    fn cache_hit_after_update() {
        let mut cache = ProjectionDisplayCache::new();
        let proj = make_projection("text.mode", 1, "NORMAL", 1);
        cache.update(&proj);

        assert_eq!(cache.get(DomainId(1), &ProjectionTag::new("text.mode")), Some("NORMAL"));
    }

    #[test]
    fn newer_version_overwrites() {
        let mut cache = ProjectionDisplayCache::new();
        cache.update(&make_projection("text.mode", 1, "NORMAL", 1));
        cache.update(&make_projection("text.mode", 1, "INSERT", 2));

        assert_eq!(cache.get(DomainId(1), &ProjectionTag::new("text.mode")), Some("INSERT"));
    }

    #[test]
    fn older_version_does_not_overwrite() {
        let mut cache = ProjectionDisplayCache::new();
        cache.update(&make_projection("text.mode", 1, "INSERT", 5));
        cache.update(&make_projection("text.mode", 1, "NORMAL", 3));

        assert_eq!(cache.get(DomainId(1), &ProjectionTag::new("text.mode")), Some("INSERT"));
    }

    #[test]
    fn transient_projections_skip_cache() {
        let mut cache = ProjectionDisplayCache::new();
        cache.update(&make_projection("text.mode", 1, "NORMAL", 1));
        cache.update(&make_transient("text.mode", 1, "FLASH"));

        assert_eq!(cache.get(DomainId(1), &ProjectionTag::new("text.mode")), Some("NORMAL"));
    }

    #[test]
    fn transient_does_not_create_entry() {
        let mut cache = ProjectionDisplayCache::new();
        cache.update(&make_transient("text.flash", 1, "highlight"));

        assert!(cache.is_empty());
    }

    #[test]
    fn evict_domain() {
        let mut cache = ProjectionDisplayCache::new();
        cache.update(&make_projection("text.mode", 1, "NORMAL", 1));
        cache.update(&make_projection("text.cursor", 1, "1:0", 1));
        cache.update(&make_projection("3d.transform", 2, "xyz", 1));

        assert_eq!(cache.len(), 3);
        cache.evict_domain(DomainId(1));
        assert_eq!(cache.len(), 1);
        assert!(
            cache
                .get(DomainId(1), &ProjectionTag::new("text.mode"))
                .is_none()
        );
        assert_eq!(cache.get(DomainId(2), &ProjectionTag::new("3d.transform")), Some("xyz"));
    }

    #[test]
    fn clear_removes_all() {
        let mut cache = ProjectionDisplayCache::new();
        cache.update(&make_projection("text.mode", 1, "NORMAL", 1));
        cache.update(&make_projection("3d.mesh", 2, "cube", 1));

        assert_eq!(cache.len(), 2);
        cache.clear();
        assert!(cache.is_empty());
    }

    #[test]
    fn entries_for_domain_filters() {
        let mut cache = ProjectionDisplayCache::new();
        cache.update(&make_projection("text.mode", 1, "NORMAL", 1));
        cache.update(&make_projection("text.cursor", 1, "1:0", 1));
        cache.update(&make_projection("3d.transform", 2, "xyz", 1));

        let domain1: Vec<_> = cache.entries_for_domain(DomainId(1)).collect();
        assert_eq!(domain1.len(), 2);

        let domain2: Vec<_> = cache.entries_for_domain(DomainId(2)).collect();
        assert_eq!(domain2.len(), 1);
    }

    #[test]
    fn different_domains_same_tag_independent() {
        let mut cache = ProjectionDisplayCache::new();
        cache.update(&make_projection("mode", 1, "TEXT-NORMAL", 1));
        cache.update(&make_projection("mode", 2, "3D-ORBIT", 1));

        assert_eq!(cache.get(DomainId(1), &ProjectionTag::new("mode")), Some("TEXT-NORMAL"));
        assert_eq!(cache.get(DomainId(2), &ProjectionTag::new("mode")), Some("3D-ORBIT"));
    }
}
