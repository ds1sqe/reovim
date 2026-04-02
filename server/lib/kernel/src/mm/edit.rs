//! Edit operations for undo/redo support.
//!
//! Re-exported from `reovim-types-text`. This module exists for backward
//! compatibility during the kernel buffer extraction (#740).

pub use reovim_types_text::{Edit, TextDimensions, delete_end, text_dimensions, transform_position};
