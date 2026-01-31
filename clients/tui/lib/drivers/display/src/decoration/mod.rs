//! Decoration system for text styling and concealment.
//!
//! This module provides a full decoration store and provider system for
//! visual modifications to text without changing the underlying content.
//!
//! # Architecture
//!
//! The decoration system follows mechanism vs policy separation:
//!
//! - **Mechanism** (this module): Defines types, traits, and storage for decorations.
//! - **Policy** (plugins): Plugins implement `DecorationProvider` to supply decorations.
//!
//! # Priority System
//!
//! Decorations are organized by priority groups (lowest to highest):
//!
//! 1. **Language** (0): Language-specific concealment (markdown, org-mode)
//! 2. **Syntax** (10): Syntax highlighting decorations
//! 3. **Search** (20): Search match highlighting
//! 4. **Diagnostic** (30): Diagnostic underlines and inline text
//! 5. **Visual** (40): Visual selection (always on top)
//!
//! # Example
//!
//! ```ignore
//! use reovim_driver_display::decoration::{
//!     DecorationStore, DecorationGroup, Decoration, Span,
//! };
//!
//! // Create store (runner owns this)
//! let mut store = DecorationStore::new();
//!
//! // Get or create buffer decorations
//! let buffer = store.get_or_create(buffer_id);
//!
//! // Add decorations from various sources
//! buffer.add(DecorationGroup::Language, Decoration::conceal(
//!     Span::line(5, 0, 20),
//!     "[link]",
//!     None,
//! ));
//!
//! buffer.add(DecorationGroup::Search, Decoration::inline_style(
//!     Span::line(5, 0, 4),
//!     search_highlight_style,
//! ));
//!
//! // Query decorations for rendering
//! let line_decorations = buffer.for_line(5);
//! ```

mod conceal;
mod key;
mod provider;
mod registry;
mod store;
mod types;

pub use {
    conceal::{ConcealedLine, apply_conceals, display_to_source_col, source_to_display_col},
    key::{DecorationProviderKey, DecorationSourceKey},
    provider::{BufferDecorationSource, DecorationProvider, DecorationProviderFactory},
    registry::{BufferDecorationSourceRegistry, DecorationProviderRegistry},
    store::{BufferDecorations, DecorationRef, DecorationStore},
    types::{Decoration, DecorationGroup, Span},
};
