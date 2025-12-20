//! UI Component System
//!
//! This module provides UI components that implement the unified [`UIComponent`] trait:
//!
//! - [`RenderContext`] for passing render-time state to components
//! - [`RenderState`] for bundling all runtime state needed for rendering
//! - [`StatusLineComponent`] for displaying mode, file info, and cursor position
//! - [`TabLineComponent`] for displaying open tabs
//!
//! All components implement the [`UIComponent`](crate::ui_component::UIComponent) trait
//! from the [`ui_component`](crate::ui_component) module.

mod display;
mod status_line;
mod tab_line;

pub use {
    display::{RenderContext, RenderState},
    status_line::StatusLineComponent,
    tab_line::TabLineComponent,
};
