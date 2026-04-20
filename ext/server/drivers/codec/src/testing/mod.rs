//! Test-only utilities for verifying `ContentCodec` implementations.
//!
//! This module is gated behind the `testing` feature flag. Enable it in a
//! dependent crate with:
//!
//! ```toml
//! [dev-dependencies]
//! reovim-driver-codec = { workspace = true, features = ["testing"] }
//! ```
//!
//! The harness is a scaffold for Plan 07 Phase 1. It exposes
//! [`harness::verify_codec`] and [`harness::HarnessFixture`] so every
//! structural (and faithful) codec can be run through the same four
//! Phase 1 gates before Phase 2 format work lands.

pub mod harness;

/// Test-only placeholder tree operation.
///
/// Stands in for real format-specific operations in harness and unit
/// tests that need a `TreeOp` value without depending on any codec
/// module crate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyntheticTreeOp {
    /// Human-readable name for diagnostics.
    pub name: String,
}

crate::impl_tree_op!(SyntheticTreeOp);
