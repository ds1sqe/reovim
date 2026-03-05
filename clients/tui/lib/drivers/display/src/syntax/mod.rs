//! Layered annotation cache for client-side syntax highlighting.
//!
//! This module provides token caching and byte-to-position conversion
//! for applying server-provided syntax tokens to rendered content.
//!
//! # Architecture
//!
//! The server streams `TokenUpdate` messages containing:
//! - `buffer_id`: Which buffer the tokens belong to
//! - `tokens[]`: Array of `TokenSpan` (`start_byte`, `end_byte`, category, kind)
//! - `start_line`, `end_line`: Affected line range
//! - `full_refresh`: If true, replace all cached tokens for that layer
//! - `layer`: Layer name (e.g., "syntax", "lsp.semantic")
//! - `priority`: Layer priority (higher = on top)
//!
//! The `LayeredTokenCache` converts byte offsets to (line, col) positions
//! and merges tokens across layers by priority for rendering.
//!
//! # Example
//!
//! ```ignore
//! use reovim_driver_display::syntax::AnnotationCacheManager;
//!
//! let mut manager = AnnotationCacheManager::new();
//!
//! // Apply update from StreamTokens
//! manager.apply_token_update(buffer_id, &spans, 0, 100, true, content, "syntax", 0);
//!
//! // Get tokens for rendering a line (merged across layers)
//! for token in manager.tokens_for_line(buffer_id, line) {
//!     let style = theme.get_style(&token.category);
//!     render_span(token.start_col, token.end_col, style);
//! }
//! ```

mod cache;

pub use cache::{
    AnnotationCacheManager, CachedAnnotationKind, CachedToken, LayeredTokenCache, TokenSpan,
};
