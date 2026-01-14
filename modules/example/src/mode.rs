//! Example mode implementation.
//!
//! Demonstrates implementing Mode (kernel), `ModeDisplay` (display driver),
//! and `ModeInput` (input driver) traits for a simple mode system.

use {
    reovim_driver_display::{CursorStyle, ModeDisplay},
    reovim_driver_input::ModeInput,
    reovim_kernel::api::v1::{Mode, ModeId},
};

use crate::EXAMPLE_MODULE;

/// Example editor mode enum.
///
/// This demonstrates how modules define modes by implementing:
/// - [`Mode`] from kernel (identity)
/// - [`ModeDisplay`] from display driver (rendering)
/// - [`ModeInput`] from input driver (input handling)
///
/// # Architecture
///
/// ```text
/// Module (this)                Policy: actual mode behavior
///     |
///     +-- impl Mode            Identity (kernel)
///     +-- impl ModeDisplay     Rendering (display driver)
///     +-- impl ModeInput       Input handling (input driver)
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ExampleMode {
    /// Normal mode - commands, navigation.
    #[default]
    Normal,
    /// Insert mode - text input.
    Insert,
}

// ============================================================================
// Kernel: Mode trait (identity only)
// ============================================================================

impl Mode for ExampleMode {
    fn id(&self) -> ModeId {
        ModeId::new(
            EXAMPLE_MODULE,
            match self {
                Self::Normal => "normal",
                Self::Insert => "insert",
            },
        )
    }
}

// ============================================================================
// Display Driver: ModeDisplay trait (rendering)
// ============================================================================

impl ModeDisplay for ExampleMode {
    fn cursor_style(&self) -> CursorStyle {
        match self {
            Self::Normal => CursorStyle::Block,
            Self::Insert => CursorStyle::Bar,
        }
    }

    fn status_text(&self) -> &'static str {
        match self {
            Self::Normal => "NORMAL",
            Self::Insert => "INSERT",
        }
    }
}

// ============================================================================
// Input Driver: ModeInput trait (input handling)
// ============================================================================

impl ModeInput for ExampleMode {
    fn accepts_char_input(&self) -> bool {
        matches!(self, Self::Insert)
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_example_mode_default() {
        let mode = ExampleMode::default();
        assert_eq!(mode, ExampleMode::Normal);
    }

    #[test]
    fn test_mode_id() {
        let normal = ExampleMode::Normal;
        let insert = ExampleMode::Insert;

        assert_eq!(normal.id().name(), "normal");
        assert_eq!(insert.id().name(), "insert");

        // Both belong to the same module
        assert_eq!(normal.id().module(), &EXAMPLE_MODULE);
        assert_eq!(insert.id().module(), &EXAMPLE_MODULE);
    }

    #[test]
    fn test_cursor_style() {
        assert_eq!(ExampleMode::Normal.cursor_style(), CursorStyle::Block);
        assert_eq!(ExampleMode::Insert.cursor_style(), CursorStyle::Bar);
    }

    #[test]
    fn test_status_text() {
        assert_eq!(ExampleMode::Normal.status_text(), "NORMAL");
        assert_eq!(ExampleMode::Insert.status_text(), "INSERT");
    }

    #[test]
    fn test_accepts_char_input() {
        assert!(!ExampleMode::Normal.accepts_char_input());
        assert!(ExampleMode::Insert.accepts_char_input());
    }

    #[test]
    fn test_mode_trait_object() {
        // Verify Mode can be used as trait object
        let mode: &dyn Mode = &ExampleMode::Normal;
        assert_eq!(mode.id().name(), "normal");
    }

    #[test]
    fn test_mode_display_trait_object() {
        // Verify ModeDisplay can be used as trait object
        let mode: &dyn ModeDisplay = &ExampleMode::Insert;
        assert_eq!(mode.cursor_style(), CursorStyle::Bar);
    }

    #[test]
    fn test_mode_input_trait_object() {
        // Verify ModeInput can be used as trait object
        let mode: &dyn ModeInput = &ExampleMode::Insert;
        assert!(mode.accepts_char_input());
    }
}
