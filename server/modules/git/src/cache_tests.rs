use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
    time::Duration,
};

use reovim_driver_git::{
    GitProvider,
    types::{BlameEntry, BranchInfo, DiffHunk, LogEntry, StashEntry, StatusEntry},
};

use super::*;

/// Test provider that counts calls to each method.
#[allow(clippy::struct_field_names)]
struct CountingProvider {
    branch_calls: AtomicUsize,
    status_calls: AtomicUsize,
    hunks_calls: AtomicUsize,
    blame_calls: AtomicUsize,
    branches_calls: AtomicUsize,
    log_calls: AtomicUsize,
    stash_calls: AtomicUsize,
}

impl CountingProvider {
    fn new() -> Self {
        Self {
            branch_calls: AtomicUsize::new(0),
            status_calls: AtomicUsize::new(0),
            hunks_calls: AtomicUsize::new(0),
            blame_calls: AtomicUsize::new(0),
            branches_calls: AtomicUsize::new(0),
            log_calls: AtomicUsize::new(0),
            stash_calls: AtomicUsize::new(0),
        }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl GitProvider for CountingProvider {
    fn current_branch(&self, _cwd: &Path) -> Option<String> {
        self.branch_calls.fetch_add(1, Ordering::Relaxed);
        Some("main".to_owned())
    }

    fn branches(&self, _cwd: &Path) -> Vec<BranchInfo> {
        self.branches_calls.fetch_add(1, Ordering::Relaxed);
        vec![BranchInfo {
            name: "main".to_owned(),
            is_current: true,
            upstream: None,
        }]
    }

    fn status(&self, _cwd: &Path) -> Vec<StatusEntry> {
        self.status_calls.fetch_add(1, Ordering::Relaxed);
        vec![]
    }

    fn log(&self, _cwd: &Path, _query: &str, _limit: usize) -> Vec<LogEntry> {
        self.log_calls.fetch_add(1, Ordering::Relaxed);
        vec![]
    }

    fn stash_list(&self, _cwd: &Path) -> Vec<StashEntry> {
        self.stash_calls.fetch_add(1, Ordering::Relaxed);
        vec![]
    }

    fn diff_hunks(&self, _path: &Path) -> Vec<DiffHunk> {
        self.hunks_calls.fetch_add(1, Ordering::Relaxed);
        vec![DiffHunk {
            old_start: 1,
            old_count: 0,
            new_start: 1,
            new_count: 3,
        }]
    }

    fn blame(&self, _path: &Path) -> Vec<BlameEntry> {
        self.blame_calls.fetch_add(1, Ordering::Relaxed);
        vec![BlameEntry {
            line: 1,
            short_hash: "abc1234".to_owned(),
            author: "Alice".to_owned(),
            date: "2025-01-15".to_owned(),
            summary: "feat: init".to_owned(),
        }]
    }
}

fn long_ttl() -> Duration {
    Duration::from_mins(1)
}

fn make_cached(ttl: Duration) -> CachedGitProvider<CountingProvider> {
    CachedGitProvider::new(CountingProvider::new(), ttl)
}

#[test]
fn branch_cache_hit() {
    let cached = make_cached(long_ttl());
    let cwd = PathBuf::from("/tmp/repo");

    let r1 = cached.current_branch(&cwd);
    let r2 = cached.current_branch(&cwd);

    assert_eq!(r1, Some("main".to_owned()));
    assert_eq!(r2, Some("main".to_owned()));
    assert_eq!(cached.inner.branch_calls.load(Ordering::Relaxed), 1);
}

#[test]
fn branch_cache_miss_after_ttl() {
    let cached = make_cached(Duration::ZERO);
    let cwd = PathBuf::from("/tmp/repo");

    cached.current_branch(&cwd);
    cached.current_branch(&cwd);

    // Zero TTL means every call is a miss
    assert_eq!(cached.inner.branch_calls.load(Ordering::Relaxed), 2);
}

#[test]
fn status_cache_hit() {
    let cached = make_cached(long_ttl());
    let cwd = PathBuf::from("/tmp/repo");

    cached.status(&cwd);
    cached.status(&cwd);

    assert_eq!(cached.inner.status_calls.load(Ordering::Relaxed), 1);
}

#[test]
fn hunks_cache_hit() {
    let cached = make_cached(long_ttl());
    let path = PathBuf::from("/tmp/repo/file.rs");

    let r1 = cached.diff_hunks(&path);
    let r2 = cached.diff_hunks(&path);

    assert_eq!(r1.len(), 1);
    assert_eq!(r2.len(), 1);
    assert_eq!(cached.inner.hunks_calls.load(Ordering::Relaxed), 1);
}

#[test]
fn blame_cache_hit() {
    let cached = make_cached(long_ttl());
    let path = PathBuf::from("/tmp/repo/file.rs");

    let r1 = cached.blame(&path);
    let r2 = cached.blame(&path);

    assert_eq!(r1.len(), 1);
    assert_eq!(r2.len(), 1);
    assert_eq!(cached.inner.blame_calls.load(Ordering::Relaxed), 1);
}

#[test]
fn blame_cache_miss_after_ttl() {
    let cached = make_cached(Duration::ZERO);
    let path = PathBuf::from("/tmp/repo/file.rs");

    cached.blame(&path);
    cached.blame(&path);

    assert_eq!(cached.inner.blame_calls.load(Ordering::Relaxed), 2);
}

#[test]
fn different_paths_independent_caches() {
    let cached = make_cached(long_ttl());
    let path_a = PathBuf::from("/tmp/repo/a.rs");
    let path_b = PathBuf::from("/tmp/repo/b.rs");

    cached.diff_hunks(&path_a);
    cached.diff_hunks(&path_b);
    cached.diff_hunks(&path_a);
    cached.diff_hunks(&path_b);

    // Two paths, each fetched once
    assert_eq!(cached.inner.hunks_calls.load(Ordering::Relaxed), 2);
}

#[test]
fn invalidate_clears_all_caches() {
    let cached = make_cached(long_ttl());
    let cwd = PathBuf::from("/tmp/repo");
    let path = PathBuf::from("/tmp/repo/file.rs");

    // Populate all caches
    cached.current_branch(&cwd);
    cached.status(&cwd);
    cached.diff_hunks(&path);
    cached.blame(&path);

    assert_eq!(cached.inner.branch_calls.load(Ordering::Relaxed), 1);
    assert_eq!(cached.inner.status_calls.load(Ordering::Relaxed), 1);
    assert_eq!(cached.inner.hunks_calls.load(Ordering::Relaxed), 1);
    assert_eq!(cached.inner.blame_calls.load(Ordering::Relaxed), 1);

    // Invalidate
    cached.invalidate();

    // Re-query — all should miss
    cached.current_branch(&cwd);
    cached.status(&cwd);
    cached.diff_hunks(&path);
    cached.blame(&path);

    assert_eq!(cached.inner.branch_calls.load(Ordering::Relaxed), 2);
    assert_eq!(cached.inner.status_calls.load(Ordering::Relaxed), 2);
    assert_eq!(cached.inner.hunks_calls.load(Ordering::Relaxed), 2);
    assert_eq!(cached.inner.blame_calls.load(Ordering::Relaxed), 2);
}

#[test]
fn uncached_branches_passes_through() {
    let cached = make_cached(long_ttl());
    let cwd = PathBuf::from("/tmp/repo");

    cached.branches(&cwd);
    cached.branches(&cwd);

    // No caching — both calls reach inner
    assert_eq!(cached.inner.branches_calls.load(Ordering::Relaxed), 2);
}

#[test]
fn uncached_log_passes_through() {
    let cached = make_cached(long_ttl());
    let cwd = PathBuf::from("/tmp/repo");

    cached.log(&cwd, "", 10);
    cached.log(&cwd, "", 10);

    assert_eq!(cached.inner.log_calls.load(Ordering::Relaxed), 2);
}

#[test]
fn uncached_stash_list_passes_through() {
    let cached = make_cached(long_ttl());
    let cwd = PathBuf::from("/tmp/repo");

    cached.stash_list(&cwd);
    cached.stash_list(&cwd);

    assert_eq!(cached.inner.stash_calls.load(Ordering::Relaxed), 2);
}

#[test]
fn concurrent_reads_do_not_panic() {
    use std::sync::Arc;

    let cached = Arc::new(make_cached(long_ttl()));
    let path = PathBuf::from("/tmp/repo/file.rs");

    // Pre-populate cache
    cached.diff_hunks(&path);

    let handles: Vec<_> = (0..4)
        .map(|_| {
            let c = Arc::clone(&cached);
            let p = path.clone();
            std::thread::spawn(move || {
                for _ in 0..100 {
                    c.diff_hunks(&p);
                }
            })
        })
        .collect();

    for h in handles {
        h.join().expect("thread panicked");
    }

    // Only 1 fetch despite hundreds of reads
    assert_eq!(cached.inner.hunks_calls.load(Ordering::Relaxed), 1);
}
