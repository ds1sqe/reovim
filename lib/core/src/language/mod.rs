//! Language-specific modules
//!
//! This module contains language-specific implementations that extend
//! the editor's functionality for particular file types.
//!
//! Each language module implements the `LanguageRenderer` trait from
//! the `decoration` module to provide visual decorations.

pub mod markdown;

pub use markdown::{MarkdownConfig, MarkdownRenderer};
