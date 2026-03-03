#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Shared benchmarking utilities for reovim modules.
//!
//! Analogous to [`reovim-testing`] for integration tests, this crate provides
//! reusable infrastructure for module-level Criterion benchmarks.
//!
//! # Components
//!
//! - [`fixtures`] — VFS tree fixture factories (`TreeFixture`)
//! - [`scaling`] — Standard parameter sets for scaling benchmarks
//! - [`criterion`] — Re-exported Criterion crate
//!
//! # Usage
//!
//! In module `Cargo.toml`:
//! ```toml
//! [dev-dependencies]
//! reovim-bench-utils = { workspace = true }
//!
//! [[bench]]
//! name = "my_bench"
//! harness = false
//! ```
//!
//! In bench file:
//! ```ignore
//! use reovim_bench_utils::criterion::*;
//! use reovim_bench_utils::fixtures::TreeFixture;
//! use reovim_bench_utils::scaling::SIZES_MEDIUM;
//!
//! fn bench_operation(c: &mut Criterion) {
//!     let mut group = c.benchmark_group("module/operation");
//!     for &size in SIZES_MEDIUM {
//!         let fixture = TreeFixture::flat(size);
//!         group.bench_with_input(
//!             BenchmarkId::new("nodes", size),
//!             &size,
//!             |b, _| b.iter(|| { /* operation */ }),
//!         );
//!     }
//!     group.finish();
//! }
//!
//! criterion_group!(benches, bench_operation);
//! criterion_main!(benches);
//! ```

pub mod fixtures;
pub mod scaling;

/// Re-export Criterion so modules depend only on this crate.
pub use criterion;

/// Re-export `serde_json` for bridge/serialization benchmarks.
pub use serde_json;

#[cfg(test)]
mod tests {
    use super::*;
    use reovim_driver_vfs::VfsDriver;

    #[test]
    fn test_fixture_flat_creates_files() {
        let fixture = fixtures::TreeFixture::flat(10);
        assert_eq!(fixture.root.to_str().unwrap(), "/bench");
        // MockVfs should have 10 files
        let entries = fixture
            .vfs
            .list_dir(&fixture.root)
            .expect("root dir should exist");
        assert_eq!(entries.len(), 10);
    }

    #[test]
    fn test_fixture_flat_zero_files() {
        let fixture = fixtures::TreeFixture::flat(0);
        let entries = fixture
            .vfs
            .list_dir(&fixture.root)
            .expect("root dir should exist");
        assert_eq!(entries.len(), 0);
    }

    #[test]
    fn test_fixture_nested() {
        let fixture = fixtures::TreeFixture::nested(3, 2);
        // Root should exist
        let entries = fixture
            .vfs
            .list_dir(&fixture.root)
            .expect("root dir should exist");
        // Root has 2 files + 1 subdirectory
        assert_eq!(entries.len(), 3);
    }

    #[test]
    fn test_fixture_project() {
        let fixture = fixtures::TreeFixture::project(30);
        let entries = fixture
            .vfs
            .list_dir(&fixture.root)
            .expect("root dir should exist");
        // Should have src/, tests/, docs/ subdirectories
        assert!(!entries.is_empty());
    }

    #[test]
    fn test_scaling_constants() {
        assert_eq!(scaling::SIZES_SMALL.len(), 3);
        assert_eq!(scaling::SIZES_MEDIUM.len(), 3);
        assert_eq!(scaling::SIZES_LARGE.len(), 3);

        // Each set should be strictly increasing
        for sizes in [
            scaling::SIZES_SMALL,
            scaling::SIZES_MEDIUM,
            scaling::SIZES_LARGE,
        ] {
            for window in sizes.windows(2) {
                assert!(window[0] < window[1]);
            }
        }
    }
}
