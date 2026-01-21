//! Window layout subsystem for nested compositor architecture.
//!
//! This module provides the trait contracts for a Hyprland-inspired
//! nested compositor window management system. Each layer is a
//! self-contained mini-compositor with three zones.
//!
//! # Architecture
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
//! - **This module (Mechanism)**: Defines WHAT can be done via traits
//! - **modules/layout/ (Policy)**: Implements HOW things behave
//!
//! # Modules
//!
//! - [`layer`] - Core types: `Layer`, `Zone`, `WindowPlacement`, `Anchor`
//! - [`compositor`] - `RootCompositor`, `WindowLayerCompositor` traits
//! - [`tiled`] - `TiledLayer` trait for vim-style splits
//! - [`floating`] - `FloatingLayer` trait for free windows (Phase 2)
//! - [`overlay`] - `OverlayLayer` trait for popups (Phase 3)
//! - [`view`] - `ViewManager` trait for per-window content state

mod compositor;
mod floating;
mod layer;
mod overlay;
mod tiled;
mod view;

// Re-export all public types

// Layer types
pub use layer::{
    Anchor, Layer, LayerConfig, LayerId, OverlayConstraints, WindowPlacement, Zone,
};

// Compositor traits and types
pub use compositor::{
    CompositeResult, RootCompositor, WindowError, WindowLayerCompositor,
};

// Zone-specific traits
pub use tiled::{TiledLayer, MIN_WINDOW_HEIGHT, MIN_WINDOW_WIDTH};
pub use floating::{FloatingLayer, FloatingWindow};
pub use overlay::{OverlayLayer, OverlayWindow};

// View management and index types
pub use view::{ColIndex, LineIndex, Position, View, ViewManager};
