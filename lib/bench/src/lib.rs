//! Shared benchmarking utilities for reovim modules.
//!
//! Analogous to [`reovim-testing`] for integration tests, this crate provides
//! reusable infrastructure for module-level Criterion benchmarks.
//!
//! # Components
//!
//! - [`large_file`] — Programmatic large-file fixture generators (UTF-8 log, ELF binary)
//! - [`rss`] — Linux RSS measurement via `/proc/self/status`
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
//! use reovim_bench_utils::scaling::SIZES_MEDIUM;
//!
//! fn bench_operation(c: &mut Criterion) {
//!     let mut group = c.benchmark_group("module/operation");
//!     for &size in SIZES_MEDIUM {
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

pub mod large_file;
pub mod rss;
pub mod scaling;

/// Re-export Criterion so modules depend only on this crate.
pub use criterion;

/// Re-export `serde_json` for bridge/serialization benchmarks.
pub use serde_json;

use std::time::Duration;

/// Return a [`criterion::Criterion`] configured for slow, large-file benchmarks.
///
/// Settings: `sample_size(10)`, `measurement_time(30 s)`.
/// Use this for benchmarks that exercise multi-GB fixtures where each
/// iteration takes seconds, not microseconds.
///
/// # Example
///
/// ```ignore
/// use reovim_bench_utils::slow_bench_config;
///
/// fn bench_large_open(c: &mut criterion::Criterion) { /* ... */ }
///
/// criterion::criterion_group! {
///     name = large_file;
///     config = slow_bench_config();
///     targets = bench_large_open
/// }
/// criterion::criterion_main!(large_file);
/// ```
#[must_use]
pub fn slow_bench_config() -> criterion::Criterion {
    criterion::Criterion::default()
        .sample_size(10)
        .measurement_time(Duration::from_secs(30))
}

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
