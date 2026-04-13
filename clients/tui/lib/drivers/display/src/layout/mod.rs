//! Window layout subsystem - re-exported from `reovim-subsys-layout`.
//!
//! All layout types (compositor traits, layer types, view management)
//! now live in `server/lib/subsys/layout/` as server-side mechanism.
//! This module re-exports everything for backward compatibility.

pub use reovim_subsys_layout::{
    // Layer types
    Anchor,
    CLICK_THROUGH_THRESHOLD,
    // View management and index types
    ColIndex,
    // Compositor traits and types
    CompositeResult,
    CompositorKey,
    CompositorRegistry,
    // Zone-specific traits
    FloatingLayer,
    FloatingWindow,
    Layer,
    LayerConfig,
    LayerId,
    LineIndex,
    MIN_WINDOW_HEIGHT,
    MIN_WINDOW_WIDTH,
    OverlayConstraints,
    OverlayLayer,
    OverlayWindow,
    Position,
    RootCompositor,
    TiledLayer,
    View,
    ViewManager,
    WindowError,
    WindowLayerCompositor,
    WindowPlacement,
    ZOrder,
    Zone,
};
