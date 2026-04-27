#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Client platform subsystem.
//!
//! Provides the module-side platform-requirement declaration contract:
//! `Requirements` with `const fn` constructors, and the `Platform` trait
//! that concrete platform crates implement.
//!
//! Modules declare their requirements as a `const REQUIRES: Requirements`
//! associated constant. The platform loader reads this at load time to
//! filter incompatible modules without instantiating them.

pub mod requirements;

pub use requirements::{Platform, Requirements};

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
