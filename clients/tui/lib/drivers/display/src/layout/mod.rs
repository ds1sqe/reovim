//! Window layout subsystem - re-exported from `reovim-subsys-layout`.
//!
//! All layout types (compositor traits, layer types, view management)
//! now live in `server/lib/subsys/layout/` as server-side mechanism.
//! This module re-exports everything for backward compatibility.

pub use reovim_subsys_layout::{
    // Layer types
    Anchor,
    CLICK_THROUGH_THRESHOLD,
    // Compositor traits and types
    CompositeResult,
    CompositorKey,
    CompositorRegistry,
    Direction,
    // Zone-specific traits
    FloatingLayer,
    FloatingWindow,
    Layer,
    LayerConfig,
    LayerId,
    MIN_WINDOW_HEIGHT,
    MIN_WINDOW_WIDTH,
    OverlayConstraints,
    OverlayLayer,
    OverlayWindow,
    RootCompositor,
    SplitDirection,
    TiledLayer,
    WindowError,
    WindowLayerCompositor,
    WindowPlacement,
    ZOrder,
    Zone,
};
