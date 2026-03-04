//! Explorer scroll hot-path benchmarks.
//!
//! Measures the performance-critical operations on the scroll path:
//! `node_count()`, `update_scroll()`, and `ExplorerBridge::snapshot()`.
//!
//! Uses the shared `reovim-bench-utils` infrastructure for VFS fixtures
//! and Criterion re-exports.

use {
    reovim_bench_utils::{
        criterion::{BenchmarkId, Criterion, criterion_group, criterion_main},
        fixtures::TreeFixture,
        scaling::SIZES_MEDIUM,
    },
    reovim_driver_session::{ExtensionMap, bridges::ExtensionStateBridge},
    reovim_module_explorer::{ExplorerBridge, ExplorerState, tree::FileTree},
};

/// Create an `ExtensionMap` with an active `ExplorerState` backed by
/// a tree from the given fixture.
fn make_active_map(fixture: &TreeFixture) -> ExtensionMap {
    let mut map = ExtensionMap::new();
    let state = map.get_or_insert::<ExplorerState>();
    state.active = true;
    state.root_path.clone_from(&fixture.root);
    state.tree = Some(
        FileTree::new(fixture.root.clone(), &fixture.vfs)
            .expect("fixture VFS should produce valid tree"),
    );
    map
}

/// Benchmark `ExplorerState::node_count()`.
///
/// First call computes and caches; subsequent calls hit the cache.
/// This measures cached (hot) performance.
fn bench_node_count(c: &mut Criterion) {
    let mut group = c.benchmark_group("explorer/node_count");

    for &size in SIZES_MEDIUM {
        let fixture = TreeFixture::flat(size);
        let map = make_active_map(&fixture);

        group.bench_with_input(BenchmarkId::new("nodes", size), &size, |b, _| {
            b.iter(|| {
                let state = map.get::<ExplorerState>().unwrap();
                state.node_count()
            });
        });
    }

    group.finish();
}

/// Benchmark `ExplorerState::update_scroll()` with a pre-filled tree.
///
/// Simulates a cursor-down then scroll adjustment — the core scroll path.
fn bench_update_scroll(c: &mut Criterion) {
    let mut group = c.benchmark_group("explorer/update_scroll");

    for &size in SIZES_MEDIUM {
        let fixture = TreeFixture::flat(size);

        group.bench_with_input(BenchmarkId::new("nodes", size), &size, |b, _| {
            let mut map = make_active_map(&fixture);
            let state = map.get_or_insert::<ExplorerState>();
            state.visible_height = 24;
            state.cursor_index = size / 2;

            b.iter(|| {
                state.update_scroll();
            });
        });
    }

    group.finish();
}

/// Benchmark `ExplorerBridge::snapshot()` — full JSON serialization.
///
/// Each iteration invalidates the cache first, forcing a full tree walk
/// and JSON rebuild.
fn bench_snapshot(c: &mut Criterion) {
    let mut group = c.benchmark_group("explorer/snapshot");

    for &size in SIZES_MEDIUM {
        let fixture = TreeFixture::flat(size);
        let map = make_active_map(&fixture);

        group.bench_with_input(BenchmarkId::new("nodes", size), &size, |b, _| {
            b.iter(|| {
                // Invalidate to force full rebuild
                map.get::<ExplorerState>().unwrap().invalidate_tree_cache();
                ExplorerBridge.snapshot(&map)
            });
        });
    }

    group.finish();
}

/// Benchmark `ExplorerBridge::snapshot()` on cache hit (full nodes).
///
/// Nodes JSON is already cached but `snapshot_generation` is reset before
/// each iteration, forcing full node inclusion in the output JSON.
fn bench_snapshot_cached(c: &mut Criterion) {
    let mut group = c.benchmark_group("explorer/snapshot_cached");

    for &size in SIZES_MEDIUM {
        let fixture = TreeFixture::flat(size);
        let map = make_active_map(&fixture);

        // Prime the nodes cache
        ExplorerBridge.snapshot(&map);

        group.bench_with_input(BenchmarkId::new("nodes", size), &size, |b, _| {
            b.iter(|| {
                // Reset snapshot_generation to force full-nodes output
                map.get::<ExplorerState>()
                    .unwrap()
                    .set_snapshot_generation(u64::MAX);
                ExplorerBridge.snapshot(&map)
            });
        });
    }

    group.finish();
}

/// Benchmark `ExplorerBridge::snapshot()` delta (metadata-only).
///
/// When `tree_generation == snapshot_generation`, the bridge omits the
/// `"nodes"` array entirely — only cursor/scroll metadata is serialized.
/// This is the hot path for cursor-only movements (j/k).
fn bench_snapshot_delta(c: &mut Criterion) {
    let mut group = c.benchmark_group("explorer/snapshot_delta");

    for &size in SIZES_MEDIUM {
        let fixture = TreeFixture::flat(size);
        let map = make_active_map(&fixture);

        // Prime the cache and set snapshot_generation
        ExplorerBridge.snapshot(&map);

        group.bench_with_input(BenchmarkId::new("nodes", size), &size, |b, _| {
            b.iter(|| ExplorerBridge.snapshot(&map));
        });
    }

    group.finish();
}

criterion_group!(
    benches,
    bench_node_count,
    bench_update_scroll,
    bench_snapshot,
    bench_snapshot_cached,
    bench_snapshot_delta,
);
criterion_main!(benches);
