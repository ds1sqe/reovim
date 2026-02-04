//! Common UI rendering helpers for plugins.
//!
//! This module provides shared utilities for building plugin UIs:
//!
//! - **Text utilities**: Truncation, alignment, padding, Unicode-aware width
//! - **Border rendering**: Re-exports from `screen::border` for convenience
//!
//! # Examples
//!
//! ```
//! use reovim_core::ui::{text, BorderStyle, BorderConfig};
//!
//! // Text truncation
//! let truncated = text::truncate_end("Very long text", 10);
//! assert_eq!(truncated, "Very lo...");
//!
//! // Text alignment
//! let centered = text::align("Title", 20, text::Alignment::Center);
//! ```

pub mod text;

// Re-export text utilities at module level for convenience
pub use text::{
    Alignment, align, display_width, pad_left, pad_right, truncate_end, truncate_start,
};

// Re-export border utilities from screen::border for unified access
pub use crate::screen::border::{
    BorderChars, BorderConfig, BorderInsets, BorderMode, BorderSides, BorderStyle, TitleAlignment,
    WindowAdjacency, render_border_to_buffer,
};
