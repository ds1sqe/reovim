//! Decoration system for language-aware visual rendering
//!
//! This module provides the infrastructure for language-specific decorations
//! such as markdown rendering, org-mode, etc.
//!
//! ## Architecture
//!
//! The decoration system is built around a trait-based API:
//!
//! - **`DecorationProvider`**: Trait that language modules implement to provide
//!   decorations for their file types.
//! - **`DecorationStore`**: Per-buffer storage for decorations with efficient
//!   line-based lookup.
//!
//! ## Usage
//!
//! Language modules (e.g., `language/markdown`) implement `DecorationProvider`
//! and register with the plugin state registry. The runtime queries the registry
//! to get the appropriate provider for each buffer.

mod provider;
mod store;
mod types;

pub use {
    provider::{DecorationFactory, DecorationProvider, SharedDecorationFactory},
    store::{BufferDecorations, DecorationRef, DecorationStore},
    types::{Decoration, DecorationGroup},
};
