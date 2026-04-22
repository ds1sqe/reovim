#![cfg_attr(coverage_nightly, allow(unused_features))]
#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Client chrome subsystem.
//!
//! Provides chrome rendering utilities, the bounded surface abstraction,
//! projection display cache, and the default viewport renderer.

pub mod chrome_utils;
pub mod conceal;
pub mod projection_cache;
pub mod scoped_surface;
pub mod ui;
pub mod viewport;

pub use {
    projection_cache::ProjectionDisplayCache,
    scoped_surface::ScopedSurface,
    viewport::{
        CBF8_DIMMED, CBF8_PALETTE, DefaultViewportRenderer, LOCAL_SELECTION_BG,
        buffer_to_screen_row_vl, client_color, dimmed_client_color, is_line_folded,
    },
};
