#![cfg_attr(coverage_nightly, allow(unused_features))]
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
//! - **Driver provides MECHANISM**: The [`HighlightCategory`] type and [`Annotation`] struct
//!   define HOW highlights are represented (open, string-based categories).
//! - **Modules provide POLICY**: Language modules decide WHAT categories to emit.
//!
//! # Architecture
//!
//! ```text
//! server/lib/drivers/syntax/             <-- HighlightCategory, Annotation, SyntaxDriver
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
//! - [`HighlightCategory`] - Open string-based highlight categories
//! - [`Annotation`] - A highlighted byte range with category and kind
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
//! for ann in highlights {
//!     println!("{:?}: {}", ann.byte_range(), ann.category.as_str());
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

pub mod bracket;
mod bridge;
mod cache;
mod composite;
pub mod decoration;
mod detect;
mod driver;
mod edit;
mod error;
mod factory;
mod fold;
mod highlight;
pub mod indent;
mod injection;
mod lang_store;
mod registry;
mod scope;
pub mod state;
mod store;
pub mod textobject;

// ============================================================================
// Re-exports
// ============================================================================

// Core traits
pub use {
    cache::SyntaxCache, composite::CompositeFactory, detect::language_id_from_path,
    driver::SyntaxDriver, factory::SyntaxDriverFactory, registry::LanguageRegistry,
    store::SyntaxFactoryStore,
};

// Types
pub use {
    bracket::{BracketConfig, BracketConfigStore, BracketPair},
    decoration::{DecorationCapture, DecorationRule, apply_rules},
    edit::SyntaxEdit,
    fold::{FoldKind, FoldRange},
    highlight::{Annotation, HighlightCategory, SyntaxContext},
    indent::{IndentConfig, IndentConfigStore},
    injection::Injection,
    lang_store::LanguageInfoStore,
    registry::{CommentTokens, DefaultLanguageRegistry, LanguageInfo},
    scope::{ContextHierarchy, ScopeKind, ScopeRange},
    textobject::{TextObjectKind, TextObjectRange, TextObjectScope},
};

// Error types
pub use error::ModuleError;

// Per-session syntax driver storage
pub use state::SyntaxSessionState;

// Bridge functions: convert text-domain events to syntax driver inputs
pub use bridge::{compute_end_position, text_event_to_syntax_edit};
