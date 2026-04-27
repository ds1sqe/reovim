#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Client capability subsystem.
//!
//! Provides the generic `Capability` marker trait, `CapabilityId` and
//! `FeatureFlag` newtypes, the drawing-primitive types used in
//! capability-tier trait method signatures (`Style`, `Rect`,
//! `Attributes`, `Color`, `ColorDepth`, `RenderingModel`, `Insets`),
//! and the `ChromeSurface` and `PlatformCapabilities` contracts
//! relocated from `subsys/module`.
//!
//! Drawing primitives live here (not in `subsys/module` or
//! `subsys/chrome`) because they appear in `ChromeSurface` and
//! `PlatformCapabilities` method signatures and the locked DAG places
//! `capability` at the bottom — nothing the traits' parameter types
//! reference can sit in a higher tier without inverting the layer
//! order.

pub mod draw;
pub mod platform_caps;
pub mod surface;

pub use {
    draw::{Attributes, Color, ColorDepth, Insets, Rect, RenderingModel, Style},
    platform_caps::PlatformCapabilities,
    surface::ChromeSurface,
};

// =============================================================================
// Capability trait and slot types
// =============================================================================

/// Zero-method marker trait for capability implementors.
///
/// Each concrete capability (e.g., `CellCapability`) implements this trait so
/// the slot system can store heterogeneous capabilities behind a single bound.
pub trait Capability {}

/// Stable numeric identity for a capability kind.
///
/// `#[repr(C)]` and copy-safe for FFI: the loader reads capability IDs from a
/// `'static` slice without instantiating the module.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct CapabilityId(pub u32);

/// Stable numeric identity for a feature flag.
///
/// Same FFI contract as `CapabilityId`; used to declare optional behaviors
/// a module enables or disables based on platform support.
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct FeatureFlag(pub u32);

#[cfg(test)]
#[path = "lib_tests.rs"]
mod tests;
