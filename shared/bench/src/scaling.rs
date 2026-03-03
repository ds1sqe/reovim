//! Standard parameter sets for scaling benchmarks.
//!
//! Use these constants for consistent parameterization across modules.
//! Each set provides three sizes: small, medium, large.

/// Small parameter set for quick sanity benchmarks.
///
/// Suitable for operations that should be fast at any scale.
pub const SIZES_SMALL: &[usize] = &[10, 50, 100];

/// Medium parameter set for typical module benchmarks.
///
/// Good balance between coverage and benchmark runtime.
pub const SIZES_MEDIUM: &[usize] = &[25, 100, 500];

/// Large parameter set for stress and regression benchmarks.
///
/// Use for operations where scaling behavior matters.
pub const SIZES_LARGE: &[usize] = &[100, 1_000, 10_000];
