#![cfg_attr(coverage_nightly, feature(coverage_attribute))]
//! Syntax highlighting driver for reovim.
//!
//! **IMPORTANT:** This crate defines ONLY the trait interface for syntax
//! highlighting. It does NOT depend on tree-sitter or any parsing library.
//! Those are implementation details of language modules (`server/modules/treesitter-*/`).
//!
//! # Design Philosophy
//!
//! This crate follows the Linux kernel "mechanism vs policy" principle:
//!
//! - **Driver provides MECHANISM**: The [`SyntaxHighlight`] trait defines HOW
//!   highlights are categorized (abstract interface).
//! - **Driver provides POLICY**: The [`HighlightGroup`] enum defines WHAT
//!   categories exist (specific implementation).
//!
//! # Architecture
//!
//! ```text
//! server/lib/drivers/syntax/             <-- SyntaxHighlight trait, HighlightGroup, SyntaxDriver
//!        ^
//!        |  implements
//!        |
//! server/lib/drivers/syntax-treesitter/  <-- Tree-sitter based implementations
//! ```
//!
//! # Components
//!
//! - [`SyntaxDriver`] - Main parsing and highlighting interface
//! - [`SyntaxDriverFactory`] - Creates drivers for languages
//! - [`LanguageRegistry`] - Language detection and metadata
//! - [`SyntaxCache`] - Highlight result caching
//! - [`HighlightGroup`] - Highlight categories (implements `SyntaxHighlight`)
//! - [`HighlightSpan`] - A highlighted byte range
//! - [`SyntaxEdit`] - Edit description for incremental parsing
//! - [`FoldRange`], [`FoldKind`] - Foldable code regions
//! - [`Injection`] - Embedded language regions
//! - [`LanguageInfo`], [`CommentTokens`] - Language metadata
//!
//! # Example
//!
//! ```ignore
//! use reovim_driver_syntax::*;
//!
//! // Factory creates drivers for supported languages
//! let factory: Box<dyn SyntaxDriverFactory> = get_factory();
//!
//! // Create driver for Rust
//! let mut driver = factory.create("rust").unwrap();
//!
//! // Parse content
//! driver.parse("fn main() { println!(\"Hello\"); }");
//!
//! // Get highlights for rendering
//! let highlights = driver.highlights(0..100);
//! for span in highlights {
//!     println!("{:?}: {}", span.byte_range(), span.group.category());
//! }
//! ```
//!
//! # NO tree-sitter dependency!
//!
//! This crate must NOT depend on tree-sitter or any parsing library.
//! Tree-sitter is an implementation detail of language modules.

// ============================================================================
// Modules
// ============================================================================

mod cache;
mod driver;
mod edit;
mod error;
mod factory;
mod fold;
mod highlight;
mod injection;
mod registry;
pub mod state;
mod store;

// ============================================================================
// Re-exports
// ============================================================================

// Core traits
pub use {
    cache::SyntaxCache, driver::SyntaxDriver, factory::SyntaxDriverFactory,
    registry::LanguageRegistry, store::SyntaxFactoryStore,
};

// Types
pub use {
    edit::SyntaxEdit,
    fold::{FoldKind, FoldRange},
    highlight::{HighlightGroup, HighlightSpan},
    injection::Injection,
    registry::{CommentTokens, LanguageInfo},
};

// Error types
pub use error::ModuleError;

// SyntaxHighlight trait (defined in this crate)
pub use highlight::SyntaxHighlight;

// Per-session syntax driver storage
pub use state::SyntaxSessionState;
