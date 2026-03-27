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
    /// Whether stage/reset/unstage/stage_lines/reset_lines should succeed.
    mutation_result: bool,
    diff_content_calls: AtomicUsize,
    stage_file_calls: AtomicUsize,
    reset_file_calls: AtomicUsize,
    unstage_file_calls: AtomicUsize,
    stage_lines_calls: AtomicUsize,
    reset_lines_calls: AtomicUsize,
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
            mutation_result: true,
            diff_content_calls: AtomicUsize::new(0),
            stage_file_calls: AtomicUsize::new(0),
            reset_file_calls: AtomicUsize::new(0),
            unstage_file_calls: AtomicUsize::new(0),
            stage_lines_calls: AtomicUsize::new(0),
            reset_lines_calls: AtomicUsize::new(0),
        }
    }

    fn with_mutation_result(mut self, result: bool) -> Self {
        self.mutation_result = result;
        self
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

    fn stage_file(&self, _path: &Path) -> bool {
        self.stage_file_calls.fetch_add(1, Ordering::Relaxed);
        self.mutation_result
    }

    fn reset_file(&self, _path: &Path) -> bool {
        self.reset_file_calls.fetch_add(1, Ordering::Relaxed);
        self.mutation_result
    }

    fn unstage_file(&self, _path: &Path) -> bool {
        self.unstage_file_calls.fetch_add(1, Ordering::Relaxed);
        self.mutation_result
    }

    fn stage_lines(&self, _cwd: &Path, _patch: &str) -> bool {
        self.stage_lines_calls.fetch_add(1, Ordering::Relaxed);
        self.mutation_result
    }

    fn reset_lines(&self, _cwd: &Path, _patch: &str) -> bool {
        self.reset_lines_calls.fetch_add(1, Ordering::Relaxed);
        self.mutation_result
    }

    fn diff_content(&self, _path: &Path) -> Option<String> {
        self.diff_content_calls.fetch_add(1, Ordering::Relaxed);
        Some("diff content".to_owned())
    }
}

fn long_ttl() -> Duration {
    Duration::from_mins(1)
}

fn make_cached(ttl: Duration) -> CachedGitProvider<CountingProvider> {
    CachedGitProvider::new(CountingProvider::new(), ttl)
}

fn make_cached_with(
    ttl: Duration,
    provider: CountingProvider,
) -> CachedGitProvider<CountingProvider> {
    CachedGitProvider::new(provider, ttl)
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
    cached.invalidate_all();

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

// =========================================================================
// Mutation methods: stage_file, reset_file, unstage_file
// =========================================================================

#[test]
fn stage_file_success_invalidates_cache() {
    let cached = make_cached(long_ttl());
    let path = PathBuf::from("/tmp/repo/file.rs");

    // Populate hunks and blame cache
    cached.diff_hunks(&path);
    cached.blame(&path);

    assert_eq!(cached.inner.hunks_calls.load(Ordering::Relaxed), 1);
    assert_eq!(cached.inner.blame_calls.load(Ordering::Relaxed), 1);

    // Stage succeeds — should invalidate hunks, status, blame for that path
    assert!(cached.stage_file(&path));
    assert_eq!(cached.inner.stage_file_calls.load(Ordering::Relaxed), 1);

    // Re-query — should miss (caches were invalidated)
    cached.diff_hunks(&path);
    cached.blame(&path);
    assert_eq!(cached.inner.hunks_calls.load(Ordering::Relaxed), 2);
    assert_eq!(cached.inner.blame_calls.load(Ordering::Relaxed), 2);
}

#[test]
fn stage_file_failure_does_not_invalidate() {
    let provider = CountingProvider::new().with_mutation_result(false);
    let cached = make_cached_with(long_ttl(), provider);
    let path = PathBuf::from("/tmp/repo/file.rs");

    // Populate cache
    cached.diff_hunks(&path);
    assert_eq!(cached.inner.hunks_calls.load(Ordering::Relaxed), 1);

    // Stage fails — cache should remain
    assert!(!cached.stage_file(&path));

    // Re-query — should hit cache
    cached.diff_hunks(&path);
    assert_eq!(cached.inner.hunks_calls.load(Ordering::Relaxed), 1);
}

#[test]
fn reset_file_success_invalidates_cache() {
    let cached = make_cached(long_ttl());
    let path = PathBuf::from("/tmp/repo/file.rs");

    cached.diff_hunks(&path);
    assert_eq!(cached.inner.hunks_calls.load(Ordering::Relaxed), 1);

    assert!(cached.reset_file(&path));
    assert_eq!(cached.inner.reset_file_calls.load(Ordering::Relaxed), 1);

    cached.diff_hunks(&path);
    assert_eq!(cached.inner.hunks_calls.load(Ordering::Relaxed), 2);
}

#[test]
fn reset_file_failure_does_not_invalidate() {
    let provider = CountingProvider::new().with_mutation_result(false);
    let cached = make_cached_with(long_ttl(), provider);
    let path = PathBuf::from("/tmp/repo/file.rs");

    cached.diff_hunks(&path);
    assert_eq!(cached.inner.hunks_calls.load(Ordering::Relaxed), 1);

    assert!(!cached.reset_file(&path));

    cached.diff_hunks(&path);
    assert_eq!(cached.inner.hunks_calls.load(Ordering::Relaxed), 1);
}

#[test]
fn unstage_file_success_invalidates_cache() {
    let cached = make_cached(long_ttl());
    let path = PathBuf::from("/tmp/repo/file.rs");

    cached.diff_hunks(&path);
    cached.blame(&path);
    assert_eq!(cached.inner.hunks_calls.load(Ordering::Relaxed), 1);
    assert_eq!(cached.inner.blame_calls.load(Ordering::Relaxed), 1);

    assert!(cached.unstage_file(&path));
    assert_eq!(cached.inner.unstage_file_calls.load(Ordering::Relaxed), 1);

    cached.diff_hunks(&path);
    cached.blame(&path);
    assert_eq!(cached.inner.hunks_calls.load(Ordering::Relaxed), 2);
    assert_eq!(cached.inner.blame_calls.load(Ordering::Relaxed), 2);
}

#[test]
fn unstage_file_failure_does_not_invalidate() {
    let provider = CountingProvider::new().with_mutation_result(false);
    let cached = make_cached_with(long_ttl(), provider);
    let path = PathBuf::from("/tmp/repo/file.rs");

    cached.diff_hunks(&path);
    assert_eq!(cached.inner.hunks_calls.load(Ordering::Relaxed), 1);

    assert!(!cached.unstage_file(&path));

    cached.diff_hunks(&path);
    assert_eq!(cached.inner.hunks_calls.load(Ordering::Relaxed), 1);
}

// =========================================================================
// Patch-based mutations: stage_lines, reset_lines
// =========================================================================

#[test]
fn stage_lines_success_clears_hunks_and_status() {
    let cached = make_cached(long_ttl());
    let cwd = PathBuf::from("/tmp/repo");
    let file = PathBuf::from("/tmp/repo/file.rs");

    // Populate hunks and status caches
    cached.diff_hunks(&file);
    cached.status(&cwd);
    assert_eq!(cached.inner.hunks_calls.load(Ordering::Relaxed), 1);
    assert_eq!(cached.inner.status_calls.load(Ordering::Relaxed), 1);

    // stage_lines succeeds — clears hunks and status entirely (not path-specific)
    assert!(cached.stage_lines(&cwd, "patch data"));
    assert_eq!(cached.inner.stage_lines_calls.load(Ordering::Relaxed), 1);

    // Re-query — should miss
    cached.diff_hunks(&file);
    cached.status(&cwd);
    assert_eq!(cached.inner.hunks_calls.load(Ordering::Relaxed), 2);
    assert_eq!(cached.inner.status_calls.load(Ordering::Relaxed), 2);
}

#[test]
fn stage_lines_failure_does_not_clear() {
    let provider = CountingProvider::new().with_mutation_result(false);
    let cached = make_cached_with(long_ttl(), provider);
    let cwd = PathBuf::from("/tmp/repo");
    let file = PathBuf::from("/tmp/repo/file.rs");

    cached.diff_hunks(&file);
    cached.status(&cwd);
    assert_eq!(cached.inner.hunks_calls.load(Ordering::Relaxed), 1);
    assert_eq!(cached.inner.status_calls.load(Ordering::Relaxed), 1);

    assert!(!cached.stage_lines(&cwd, "patch data"));

    // Caches should remain
    cached.diff_hunks(&file);
    cached.status(&cwd);
    assert_eq!(cached.inner.hunks_calls.load(Ordering::Relaxed), 1);
    assert_eq!(cached.inner.status_calls.load(Ordering::Relaxed), 1);
}

#[test]
fn reset_lines_success_clears_hunks_and_status() {
    let cached = make_cached(long_ttl());
    let cwd = PathBuf::from("/tmp/repo");
    let file = PathBuf::from("/tmp/repo/file.rs");

    cached.diff_hunks(&file);
    cached.status(&cwd);
    assert_eq!(cached.inner.hunks_calls.load(Ordering::Relaxed), 1);
    assert_eq!(cached.inner.status_calls.load(Ordering::Relaxed), 1);

    assert!(cached.reset_lines(&cwd, "patch data"));
    assert_eq!(cached.inner.reset_lines_calls.load(Ordering::Relaxed), 1);

    cached.diff_hunks(&file);
    cached.status(&cwd);
    assert_eq!(cached.inner.hunks_calls.load(Ordering::Relaxed), 2);
    assert_eq!(cached.inner.status_calls.load(Ordering::Relaxed), 2);
}

#[test]
fn reset_lines_failure_does_not_clear() {
    let provider = CountingProvider::new().with_mutation_result(false);
    let cached = make_cached_with(long_ttl(), provider);
    let cwd = PathBuf::from("/tmp/repo");
    let file = PathBuf::from("/tmp/repo/file.rs");

    cached.diff_hunks(&file);
    cached.status(&cwd);
    assert_eq!(cached.inner.hunks_calls.load(Ordering::Relaxed), 1);
    assert_eq!(cached.inner.status_calls.load(Ordering::Relaxed), 1);

    assert!(!cached.reset_lines(&cwd, "patch data"));

    cached.diff_hunks(&file);
    cached.status(&cwd);
    assert_eq!(cached.inner.hunks_calls.load(Ordering::Relaxed), 1);
    assert_eq!(cached.inner.status_calls.load(Ordering::Relaxed), 1);
}

// =========================================================================
// diff_content pass-through (uncached)
// =========================================================================

#[test]
fn diff_content_passes_through() {
    let cached = make_cached(long_ttl());
    let path = PathBuf::from("/tmp/repo/file.rs");

    let r1 = cached.diff_content(&path);
    let r2 = cached.diff_content(&path);

    assert_eq!(r1, Some("diff content".to_owned()));
    assert_eq!(r2, Some("diff content".to_owned()));
    // Not cached — both calls reach inner
    assert_eq!(cached.inner.diff_content_calls.load(Ordering::Relaxed), 2);
}

// =========================================================================
// invalidate() path-specific behavior
// =========================================================================

#[test]
fn invalidate_removes_only_target_path() {
    let cached = make_cached(long_ttl());
    let path_a = PathBuf::from("/tmp/repo/a.rs");
    let path_b = PathBuf::from("/tmp/repo/b.rs");

    // Populate caches for both paths
    cached.diff_hunks(&path_a);
    cached.diff_hunks(&path_b);
    cached.blame(&path_a);
    cached.blame(&path_b);

    assert_eq!(cached.inner.hunks_calls.load(Ordering::Relaxed), 2);
    assert_eq!(cached.inner.blame_calls.load(Ordering::Relaxed), 2);

    // Invalidate only path_a
    cached.invalidate(&path_a);

    // path_a should miss, path_b should still hit
    cached.diff_hunks(&path_a);
    cached.diff_hunks(&path_b);
    cached.blame(&path_a);
    cached.blame(&path_b);

    assert_eq!(cached.inner.hunks_calls.load(Ordering::Relaxed), 3); // a re-fetched
    assert_eq!(cached.inner.blame_calls.load(Ordering::Relaxed), 3); // a re-fetched
}

// =========================================================================
// TTL expiry for status and hunks
// =========================================================================

#[test]
fn status_cache_miss_after_ttl() {
    let cached = make_cached(Duration::ZERO);
    let cwd = PathBuf::from("/tmp/repo");

    cached.status(&cwd);
    cached.status(&cwd);

    assert_eq!(cached.inner.status_calls.load(Ordering::Relaxed), 2);
}

#[test]
fn hunks_cache_miss_after_ttl() {
    let cached = make_cached(Duration::ZERO);
    let path = PathBuf::from("/tmp/repo/file.rs");

    cached.diff_hunks(&path);
    cached.diff_hunks(&path);

    assert_eq!(cached.inner.hunks_calls.load(Ordering::Relaxed), 2);
}

// =========================================================================
// CacheEntry direct tests
// =========================================================================

#[test]
fn cache_entry_new_is_not_expired_with_long_ttl() {
    let entry = CacheEntry::new(42);
    assert!(!entry.is_expired(Duration::from_secs(60)));
    assert_eq!(entry.value, 42);
}

#[test]
fn cache_entry_is_expired_with_zero_ttl() {
    let entry = CacheEntry::new("test");
    assert!(entry.is_expired(Duration::ZERO));
}
