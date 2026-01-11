//! Syntax highlighting driver for reovim.
//!
//! **IMPORTANT:** This crate defines ONLY the trait interface for syntax
//! highlighting. It does NOT depend on tree-sitter or any parsing library.
//! Those are implementation details of language modules (plugins/languages/*).
//!
//! # Architecture
//!
//! ```text
//! lib/drivers/syntax/     <-- Trait definitions (this crate)
//!        ^
//!        |
//! plugins/languages/*     <-- Implementations (tree-sitter, etc.)
//! ```
//!
//! # Components
//!
//! - `SyntaxDriver` trait (parsing, highlighting)
//! - `LanguageRegistry` (language discovery)
//! - Highlight cache infrastructure

// Placeholder - SyntaxDriver trait will be added in Phase 3
// NO tree-sitter imports allowed here!
