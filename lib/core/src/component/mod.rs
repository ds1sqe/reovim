//! UI Component System
//!
//! This module provides a unified architecture for UI components:
//!
//! - [`DisplayComponent`] trait for display-only components (`StatusLine`, `TabLine`)
//! - [`RenderContext`] for passing render-time state to components
//!
//! For input-handling components, see the [`interactor`](crate::interactor) module.
//!
//! # Architecture
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────────┐
//! │                      UI Component System                         │
//! ├─────────────────────────────────────────────────────────────────┤
//! │  Interactor (input-receiving)    │  DisplayComponent (display)  │
//! │  ─────────────────────────────   │  ───────────────────────────  │
//! │  • Editor                        │  • StatusLineComponent       │
//! │  • Explorer                      │  • TabLineComponent          │
//! │  • Telescope                     │                              │
//! │  • CommandLine                   │                              │
//! └─────────────────────────────────────────────────────────────────┘
//! ```

mod display;
mod status_line;
mod tab_line;

pub use {
    display::{DisplayComponent, RenderContext},
    status_line::StatusLineComponent,
    tab_line::TabLineComponent,
};
