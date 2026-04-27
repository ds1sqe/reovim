//! Benchmark utilities for reovim.
//!
//! This crate provides benchmarks for kernel operations using Criterion.
//!
//! # Running Benchmarks
//!
//! ```bash
//! # Run all benchmarks
//! cargo bench -p reovim-bench
//!
//! # Run specific benchmark
//! cargo bench -p reovim-bench -- buffer_ops
//!
//! # Run with HTML report
//! cargo bench -p reovim-bench -- --save-baseline main
//! ```
//!
//! # Available Benchmarks
//!
//! - `buffer_ops` - Buffer insert/delete operations at various sizes
//! - `event_dispatch` - Event dispatch with varying handler counts
//! - `profiler_overhead` - Profiling infrastructure overhead measurement

/// Re-export kernel types for convenience in benchmarks.
pub use reovim_kernel::api::v1::*;
