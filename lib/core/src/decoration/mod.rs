//! Decoration system for language-aware visual rendering
//!
//! This module provides the infrastructure for language-specific decorations
//! such as markdown rendering, org-mode, etc.
//!
//! ## Architecture
//!
//! The decoration system is built around a trait-based API:
//!
//! - **`LanguageRenderer`**: Trait that language modules implement to provide
//!   decorations for their file types.
//! - **`DecorationStore`**: Per-buffer storage for decorations with efficient
//!   line-based lookup.
//! - **`LanguageRendererRegistry`**: Central registry for language renderers
//!   enabling dynamic dispatch.
//!
//! ## Usage
//!
//! Language modules (e.g., `language/markdown`) implement `LanguageRenderer`
//! and register with the registry. The runtime queries the registry to get
//! the appropriate renderer for each buffer.

mod registry;
mod store;
mod types;

pub use {
    registry::LanguageRendererRegistry,
    store::{BufferDecorations, DecorationRef, DecorationStore},
    types::{Decoration, DecorationContext, DecorationGroup, LanguageId, LanguageRenderer},
};
