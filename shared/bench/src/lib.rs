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
#[path = "lib_tests.rs"]
mod tests;
