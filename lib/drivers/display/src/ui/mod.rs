//! UI primitives for rendering.
//!
//! This module provides Unicode-aware text utilities that correctly
//! handle CJK characters, zero-width marks, and other Unicode features.
//!
//! # Example
//!
//! ```ignore
//! use reovim_driver_display::ui::{display_width, truncate_end, align, Alignment};
//!
//! // Calculate display width (CJK = 2 columns)
//! assert_eq!(display_width("Hello"), 5);
//! assert_eq!(display_width("你好"), 4);
//!
//! // Truncate long text
//! let truncated = truncate_end("Very long filename.txt", 15);
//!
//! // Align text
//! let centered = align("Title", 20, Alignment::Center);
//! ```

mod text;

pub use text::{
    Alignment, align, display_width, pad_left, pad_right, truncate_end, truncate_start, wrap_text,
};
