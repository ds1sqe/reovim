//! Unified z-layer compositor for rendering
//!
//! This module provides a unified compositing system that manages all renderable
//! elements with granular z-ordering control. It unifies the existing `Layer` and
//! `Overlay` systems under a single `Composable` trait.

mod types;

pub use types::{Bounds, Composable, ComposableId, Compositor, CompositorEntry, ZGroup, ZOrder};
