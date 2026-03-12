//! TTL-based caching wrapper for any [`GitProvider`].
//!
//! Caches frequently-queried methods (`current_branch`, `status`,
//! `diff_hunks`, `blame`) with per-path entries that expire after a
//! configurable TTL. Infrequent or user-triggered methods (`branches`,
//! `log`, `stash_list`) pass through directly.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::RwLock,
    time::{Duration, Instant},
};

use reovim_driver_git::{
    GitProvider,
    types::{BlameEntry, BranchInfo, DiffHunk, LogEntry, StashEntry, StatusEntry},
};

/// A cached value with its creation timestamp.
struct CacheEntry<T> {
    value: T,
    created: Instant,
}

impl<T> CacheEntry<T> {
    fn new(value: T) -> Self {
        Self {
            value,
            created: Instant::now(),
        }
    }

    fn is_expired(&self, ttl: Duration) -> bool {
        self.created.elapsed() >= ttl
    }
}

/// TTL-based caching wrapper around a [`GitProvider`].
///
/// Caches results keyed by path. Entries expire after `ttl`. Methods
/// that are only called on user interaction (`branches`, `log`,
/// `stash_list`) are not cached and pass through directly.
pub struct CachedGitProvider<P> {
    inner: P,
    branch_cache: RwLock<HashMap<PathBuf, CacheEntry<Option<String>>>>,
    status_cache: RwLock<HashMap<PathBuf, CacheEntry<Vec<StatusEntry>>>>,
    hunks_cache: RwLock<HashMap<PathBuf, CacheEntry<Vec<DiffHunk>>>>,
    blame_cache: RwLock<HashMap<PathBuf, CacheEntry<Vec<BlameEntry>>>>,
    ttl: Duration,
}

impl<P: GitProvider> CachedGitProvider<P> {
    /// Create a new caching wrapper with the given TTL.
    pub fn new(inner: P, ttl: Duration) -> Self {
        Self {
            inner,
            branch_cache: RwLock::new(HashMap::new()),
            status_cache: RwLock::new(HashMap::new()),
            hunks_cache: RwLock::new(HashMap::new()),
            blame_cache: RwLock::new(HashMap::new()),
            ttl,
        }
    }

    /// Clear all cached entries.
    #[cfg(test)]
    pub fn invalidate(&self) {
        self.branch_cache.write().expect("lock poisoned").clear();
        self.status_cache.write().expect("lock poisoned").clear();
        self.hunks_cache.write().expect("lock poisoned").clear();
        self.blame_cache.write().expect("lock poisoned").clear();
    }
}

impl<P: GitProvider> GitProvider for CachedGitProvider<P> {
    fn current_branch(&self, cwd: &Path) -> Option<String> {
        let key = cwd.to_path_buf();

        {
            let cache = self.branch_cache.read().expect("lock poisoned");
            if let Some(entry) = cache.get(&key)
                && !entry.is_expired(self.ttl)
            {
                return entry.value.clone();
            }
        }

        let value = self.inner.current_branch(cwd);
        self.branch_cache
            .write()
            .expect("lock poisoned")
            .insert(key, CacheEntry::new(value.clone()));
        value
    }

    fn branches(&self, cwd: &Path) -> Vec<BranchInfo> {
        self.inner.branches(cwd)
    }

    fn status(&self, cwd: &Path) -> Vec<StatusEntry> {
        let key = cwd.to_path_buf();

        {
            let cache = self.status_cache.read().expect("lock poisoned");
            if let Some(entry) = cache.get(&key)
                && !entry.is_expired(self.ttl)
            {
                return entry.value.clone();
            }
        }

        let value = self.inner.status(cwd);
        self.status_cache
            .write()
            .expect("lock poisoned")
            .insert(key, CacheEntry::new(value.clone()));
        value
    }

    fn log(&self, cwd: &Path, query: &str, limit: usize) -> Vec<LogEntry> {
        self.inner.log(cwd, query, limit)
    }

    fn stash_list(&self, cwd: &Path) -> Vec<StashEntry> {
        self.inner.stash_list(cwd)
    }

    fn diff_hunks(&self, path: &Path) -> Vec<DiffHunk> {
        let key = path.to_path_buf();

        {
            let cache = self.hunks_cache.read().expect("lock poisoned");
            if let Some(entry) = cache.get(&key)
                && !entry.is_expired(self.ttl)
            {
                return entry.value.clone();
            }
        }

        let value = self.inner.diff_hunks(path);
        self.hunks_cache
            .write()
            .expect("lock poisoned")
            .insert(key, CacheEntry::new(value.clone()));
        value
    }

    fn blame(&self, path: &Path) -> Vec<BlameEntry> {
        let key = path.to_path_buf();

        {
            let cache = self.blame_cache.read().expect("lock poisoned");
            if let Some(entry) = cache.get(&key)
                && !entry.is_expired(self.ttl)
            {
                return entry.value.clone();
            }
        }

        let value = self.inner.blame(path);
        self.blame_cache
            .write()
            .expect("lock poisoned")
            .insert(key, CacheEntry::new(value.clone()));
        value
    }
}

#[cfg(test)]
#[path = "cache_tests.rs"]
mod tests;
