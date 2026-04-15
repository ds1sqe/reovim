#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Window layout driver for reovim.
//!
//! Linux equivalent: window compositor mechanism in `drivers/video/`
//!
//! # Architecture
//!
//! This crate defines trait contracts for a Hyprland-inspired nested
//! compositor window management system. Each layer is a self-contained
//! mini-compositor with three zones (Tiled, Float, Overlay).
//!
//! ```text
//! RootCompositor (manages layers)
//! │
//! ├── Layer 1 ("main", z=100)
//! │   └── WindowLayerCompositor
//! │       ├── Tiled Zone  (binary split tree)
//! │       ├── Float Zone  (free positioning)
//! │       └── Overlay Zone (popups, menus)
//! │
//! └── Layer 2 ("term", z=200)
//!     └── WindowLayerCompositor
//!         └── ...
//! ```
//!
//! # Mechanism vs Policy
//!
//! - **This crate (Mechanism)**: Defines WHAT can be done via traits
//! - **server/modules/layout/ (Policy)**: Implements HOW things behave

mod compositor;
mod compositor_key;
mod compositor_registry;
mod floating;
mod layer;
mod overlay;
mod tiled;
mod topology;

// Re-export all public types

// Layer types
pub use layer::{
    Anchor, CLICK_THROUGH_THRESHOLD, Layer, LayerConfig, LayerId, OverlayConstraints,
    WindowPlacement, ZOrder, Zone,
};

// Compositor traits and types
pub use compositor::{CompositeResult, RootCompositor, WindowError, WindowLayerCompositor};

// Zone-specific traits
pub use {
    floating::{FloatingLayer, FloatingWindow},
    overlay::{OverlayLayer, OverlayWindow},
    tiled::{MIN_WINDOW_HEIGHT, MIN_WINDOW_WIDTH, TiledLayer},
};

// Typed key and registry (Epic #417 - UniqueProvider abstraction)
pub use {compositor_key::CompositorKey, compositor_registry::CompositorRegistry};

// Layout topology types
pub use topology::{LayoutTopology, Permil, SplitError};

// Re-export geometry and direction types from common client model
pub use reovim_client_model::{Direction, Rect, Size, SplitDirection};

// Re-export kernel ID types
pub use reovim_kernel::api::v1::WindowId;

/// Type alias for backward compatibility.
///
/// Layout code uses `NavigateDirection`, common model uses `Direction`.
/// They have the same semantics (Up, Down, Left, Right).
pub type NavigateDirection = Direction;
