//! Syntax token caching for client-side syntax highlighting.
//!
//! This module provides token caching and byte-to-position conversion
//! for applying server-provided syntax tokens to rendered content.
//!
//! # Architecture
//!
//! The server streams `TokenUpdate` messages containing:
//! - `buffer_id`: Which buffer the tokens belong to
//! - `tokens[]`: Array of `TokenSpan` (`start_byte`, `end_byte`, category)
//! - `start_line`, `end_line`: Affected line range
//! - `full_refresh`: If true, replace all cached tokens
//!
//! The `TokenCache` converts byte offsets to (line, col) positions and
//! provides efficient lookup for rendering.
//!
//! # Example
//!
//! ```ignore
//! use reovim_driver_display::syntax::TokenCacheManager;
//!
//! let mut manager = TokenCacheManager::new();
//!
//! // Apply update from StreamTokens
//! manager.apply_update(&update, buffer_content);
//!
//! // Get tokens for rendering a line
//! for token in manager.tokens_for_line(buffer_id, line) {
//!     let style = theme.get_style(&token.category);
//!     render_span(token.start_col, token.end_col, style);
//! }
//! ```

mod cache;

pub use cache::{CachedToken, TokenCache, TokenCacheManager, TokenSpan};
