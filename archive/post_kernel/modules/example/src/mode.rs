//! Example mode implementation.
//!
//! Demonstrates implementing Mode (kernel) and `ModeDisplay` (display driver)
//! traits for a simple mode system.

use {
    reovim_driver_display::ModeDisplay,
    reovim_kernel::api::v1::{CursorStyle, Mode, ModeId, ModuleId},
};

use crate::EXAMPLE_MODULE;

/// Example editor mode enum.
///
/// This demonstrates how modules define modes by implementing:
/// - [`Mode`] from kernel (identity + behavior, including `accepts_char_input()`)
/// - [`ModeDisplay`] from display driver (rendering, delegates to Mode)
///
/// # Architecture
///
/// ```text
/// Module (this)                Policy: actual mode behavior
///     |
///     +-- impl Mode            Identity + behavior (kernel)
///     +-- impl ModeDisplay     Rendering (delegates to Mode)
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Hash)]
#[repr(u16)]
pub enum ExampleMode {
    /// Normal mode - commands, navigation.
    #[default]
    Normal = 0,
    /// Insert mode - text input.
    Insert = 1,
}

// ============================================================================
// Kernel: Mode trait (identity + behavior)
// ============================================================================

impl Mode for ExampleMode {
    fn module() -> ModuleId {
        EXAMPLE_MODULE
    }

    fn discriminant(&self) -> u16 {
        *self as u16
    }

    fn id(&self) -> ModeId {
        // Use lowercase names for ModeId (for programmatic matching)
        // display_name() returns uppercase for statusline display
        let name = match self {
            Self::Normal => "normal",
            Self::Insert => "insert",
        };
        ModeId::with_discriminant(EXAMPLE_MODULE, name, self.discriminant())
    }

    fn display_name(&self) -> &'static str {
        // Uppercase names for statusline display
        match self {
            Self::Normal => "NORMAL",
            Self::Insert => "INSERT",
        }
    }

    fn cursor_style(&self) -> CursorStyle {
        match self {
            Self::Normal => CursorStyle::Block,
            Self::Insert => CursorStyle::Bar,
        }
    }

    fn accepts_char_input(&self) -> bool {
        matches!(self, Self::Insert)
    }
}

// ============================================================================
// Display Driver: ModeDisplay trait (delegates to Mode)
// ============================================================================

impl ModeDisplay for ExampleMode {
    fn cursor_style(&self) -> CursorStyle {
        // Delegate to Mode trait
        Mode::cursor_style(self)
    }

    fn status_text(&self) -> &'static str {
        // Delegate to Mode trait's display_name
        Mode::display_name(self)
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
    fn test_mode_discriminants() {
        assert_eq!(ExampleMode::Normal.discriminant(), 0);
        assert_eq!(ExampleMode::Insert.discriminant(), 1);
    }

    #[test]
    fn test_mode_id() {
        let normal = ExampleMode::Normal;
        let insert = ExampleMode::Insert;

        // ModeId names are lowercase (for programmatic matching)
        assert_eq!(normal.id().name(), "normal");
        assert_eq!(insert.id().name(), "insert");

        // Both belong to the same module
        assert_eq!(normal.id().module(), &EXAMPLE_MODULE);
        assert_eq!(insert.id().module(), &EXAMPLE_MODULE);

        // Discriminants are unique
        assert_eq!(normal.id().discriminant(), 0);
        assert_eq!(insert.id().discriminant(), 1);
    }

    #[test]
    fn test_mode_module() {
        assert_eq!(ExampleMode::module(), EXAMPLE_MODULE);
    }

    #[test]
    fn test_cursor_style() {
        // Use Mode trait (canonical source)
        assert_eq!(Mode::cursor_style(&ExampleMode::Normal), CursorStyle::Block);
        assert_eq!(Mode::cursor_style(&ExampleMode::Insert), CursorStyle::Bar);
    }

    #[test]
    fn test_display_name() {
        assert_eq!(Mode::display_name(&ExampleMode::Normal), "NORMAL");
        assert_eq!(Mode::display_name(&ExampleMode::Insert), "INSERT");
    }

    #[test]
    fn test_accepts_char_input() {
        // Use Mode trait (canonical source)
        assert!(!Mode::accepts_char_input(&ExampleMode::Normal));
        assert!(Mode::accepts_char_input(&ExampleMode::Insert));
    }

    #[test]
    fn test_mode_display_delegates_to_mode() {
        // ModeDisplay should delegate to Mode trait
        assert_eq!(
            <ExampleMode as ModeDisplay>::cursor_style(&ExampleMode::Normal),
            Mode::cursor_style(&ExampleMode::Normal)
        );
        assert_eq!(
            <ExampleMode as ModeDisplay>::status_text(&ExampleMode::Insert),
            Mode::display_name(&ExampleMode::Insert)
        );
    }

    #[test]
    fn test_mode_display_trait_object() {
        // Verify ModeDisplay can be used as trait object
        let mode: &dyn ModeDisplay = &ExampleMode::Insert;
        assert_eq!(mode.cursor_style(), CursorStyle::Bar);
    }

    #[test]
    fn test_mode_id_equality() {
        // Same mode produces equal IDs
        assert_eq!(ExampleMode::Normal.id(), ExampleMode::Normal.id());
        assert_eq!(ExampleMode::Insert.id(), ExampleMode::Insert.id());

        // Different modes produce different IDs
        assert_ne!(ExampleMode::Normal.id(), ExampleMode::Insert.id());
    }

    #[test]
    fn test_mode_into_mode_id() {
        // Test blanket impl From<M> for ModeId
        let mode = ExampleMode::Insert;
        let id: ModeId = mode.into();
        assert_eq!(id.discriminant(), ExampleMode::Insert.discriminant());
    }
}
