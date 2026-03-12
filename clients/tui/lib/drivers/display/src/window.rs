//! Window management types.
//!
//! `WindowId` is re-exported from the kernel (single source of truth).
//! Geometry and direction types come from the common client model.
//!
//! This module provides type aliases for backward compatibility and
//! any TUI-specific extensions.

// Re-export geometry and direction types from common client model
pub use reovim_client_model::{Direction, Rect, Size, SplitDirection};

/// Type alias for backward compatibility.
///
/// TUI code uses `NavigateDirection`, common model uses `Direction`.
/// They have the same semantics (Up, Down, Left, Right).
pub type NavigateDirection = Direction;

/// Type alias for backward compatibility.
///
/// TUI code uses `TerminalSize`, common model uses `Size`.
/// They have the same semantics (width, height).
pub type TerminalSize = Size;

/// Extension trait for `Size` providing TUI-specific functionality.
pub trait TerminalSizeExt {
    /// Check if the size is valid (non-zero dimensions).
    ///
    /// This is the inverse of `Size::is_empty()`.
    fn is_valid(&self) -> bool;
}

impl TerminalSizeExt for Size {
    fn is_valid(&self) -> bool {
        !self.is_empty()
    }
}

#[cfg(test)]
#[path = "window_tests.rs"]
mod tests;
