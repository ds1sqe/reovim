//! Tree-sitter based syntax driver implementation.
//!
//! This crate provides a tree-sitter based implementation of the `SyntaxDriver`
//! trait from `reovim-driver-syntax`. It handles the generic tree-sitter parsing
//! and highlighting logic, but does NOT include any language grammars.
//!
//! # Architecture
//!
//! ```text
//! Language Modules                  This Crate                   Trait Crate
//! ================                  ==========                   ===========
//! treesitter-rust     ──────────►  TreeSitterDriver  ──impl──►  SyntaxDriver
//! treesitter-markdown
//! treesitter-python
//! ```
//!
//! # Three-Layer Design
//!
//! 1. **`reovim-driver-syntax`** (trait crate): Defines `SyntaxDriver`, `HighlightCategory`
//! 2. **`reovim-driver-syntax-treesitter`** (this crate): Generic tree-sitter implementation
//! 3. **Language modules** (e.g., `treesitter-rust`): Provide grammars and queries
//!
//! This separation keeps language-specific dependencies (tree-sitter-rust, etc.)
//! in their own modules rather than bundled into the driver.
//!
//! # Example
//!
//! ```ignore
//! use std::sync::Arc;
//! use reovim_driver_syntax_treesitter::TreeSitterDriver;
//! use tree_sitter::Query;
//!
//! // Language module provides grammar and query
//! let language = tree_sitter_rust::LANGUAGE;
//! let query = Query::new(&language.into(), RUST_HIGHLIGHTS_QUERY).unwrap();
//!
//! // Create driver
//! let mut driver = TreeSitterDriver::new(
//!     "rust",
//!     &language.into(),
//!     Arc::new(query),
//! ).unwrap();
//!
//! // Parse and highlight
//! driver.parse("fn main() {}");
//! let highlights = driver.highlights(0..12);
//! ```
//!
//! # Thread Safety
//!
//! `TreeSitterDriver` is `Send + Sync` through interior mutability:
//! - `Parser` and `QueryCursor` use `Mutex` (exclusive access needed)
//! - `Tree` and content use `RwLock` (read-heavy access pattern)

mod driver;
mod injection;

pub use {
    driver::{TreeSitterDriver, TreeSitterDriverBuilder},
    injection::{InjectionLayer, InjectionLayerFactory, InjectionLayerStore, InjectionManager},
};

// Re-export tree_sitter types that language modules need
pub use tree_sitter::{Language, Query};
