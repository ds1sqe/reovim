#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Server-tier registry contracts for display-driver-side theme types.
//!
//! This crate publishes the slim trait surface that `apps/server` and
//! server modules need to wire display-driver impls without taking a
//! compile-time dep on the client-tier `reovim-driver-display` crate.
//! Concrete `Style`/`Color`-aware impls live in `reovim-driver-display`;
//! this crate exposes the traits and pure-data types those impls bind to.
//!
//! # Architecture
//!
//! ```text
//! Server bootstrap  -->  registers Arc<dyn ThemeManager>, etc.
//!                  -->  via traits in this crate
//!
//! Display driver    -->  implements those traits with Style-aware logic
//!                  -->  Style stays in reovim-driver-display
//! ```
//!
//! No `Style`, `Color`, or other display primitives appear here.

pub mod theme;
