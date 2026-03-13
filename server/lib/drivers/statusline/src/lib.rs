//! Display-agnostic statusline data types for reovim.
//!
//! This crate provides the data contract between server modules and the
//! display layer for statusline rendering. Server modules implement
//! [`ComponentDataProvider`] to produce raw text data; the display driver
//! wraps these with presentation (Style, Color) via an adapter.
//!
//! # Architecture
//!
//! ```text
//! Server modules  -->  ComponentDataProvider  -->  ComponentData (text only)
//! Display driver  -->  adapts to ComponentProvider  -->  ComponentOutput (with Style)
//! ```
//!
//! No `Style`, `Color`, or any display types exist in this crate.

mod component;
mod context;
mod diagnostic;

pub use {
    component::{
        ComponentData, ComponentDataProvider, ComponentDataProviderKey,
        ComponentDataProviderRegistry,
    },
    context::ComponentDataContext,
    diagnostic::DiagnosticCounts,
};

#[cfg(test)]
mod component_tests;
#[cfg(test)]
mod context_tests;
#[cfg(test)]
mod diagnostic_tests;
