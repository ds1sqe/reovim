//! UI Component System
//!
//! This module provides UI components for rendering:
//!
//! - [`RenderContext`] for passing render-time state to components
//! - [`RenderState`] for bundling all runtime state needed for rendering
//! - [`TabLineComponent`] for displaying open tabs

mod display;
mod tab_line;

pub use {
    display::{RenderContext, RenderState},
    tab_line::TabLineComponent,
};
