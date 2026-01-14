//! Mode display traits and types.
//!
//! This module defines how modes affect display rendering.
//! Modes implement `ModeDisplay` to control cursor style and status text.
//!
//! # Architecture
//!
//! | Layer | Responsibility |
//! |-------|---------------|
//! | Kernel | Identity: `Mode` trait, `ModeId` |
//! | Display Driver (this) | Display: `ModeDisplay`, `CursorStyle` |
//! | Modules | Policy: actual mode implementations |
//!
//! # Example
//!
//! ```ignore
//! use reovim_driver_display::{ModeDisplay, CursorStyle};
//!
//! #[derive(Debug, Clone, Copy)]
//! enum EditorMode {
//!     Normal,
//!     Insert,
//!     Visual,
//! }
//!
//! impl ModeDisplay for EditorMode {
//!     fn cursor_style(&self) -> CursorStyle {
//!         match self {
//!             Self::Normal => CursorStyle::Block,
//!             Self::Insert => CursorStyle::Bar,
//!             Self::Visual => CursorStyle::Block,
//!         }
//!     }
//!
//!     fn status_text(&self) -> &'static str {
//!         match self {
//!             Self::Normal => "NORMAL",
//!             Self::Insert => "INSERT",
//!             Self::Visual => "VISUAL",
//!         }
//!     }
//! }
//! ```

// ============================================================================
// CursorStyle
// ============================================================================

/// Cursor display style.
///
/// Different modes typically use different cursor styles to provide
/// visual feedback about the current mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum CursorStyle {
    /// Block cursor (full cell, typical for Normal mode).
    #[default]
    Block,
    /// Vertical bar cursor (thin line, typical for Insert mode).
    Bar,
    /// Underline cursor (horizontal line under the character).
    Underline,
    /// Hidden cursor (cursor not visible).
    Hidden,
}

impl CursorStyle {
    /// Check if the cursor is visible.
    #[must_use]
    pub const fn is_visible(&self) -> bool {
        !matches!(self, Self::Hidden)
    }

    /// Get the style name for display.
    #[must_use]
    pub const fn name(&self) -> &'static str {
        match self {
            Self::Block => "block",
            Self::Bar => "bar",
            Self::Underline => "underline",
            Self::Hidden => "hidden",
        }
    }
}

// ============================================================================
// ModeDisplay Trait
// ============================================================================

/// Trait defining how a mode affects display.
///
/// Modes implement this trait to specify their visual characteristics:
/// - Cursor style (block, bar, underline, hidden)
/// - Status text (shown in statusline)
///
/// # Design Philosophy
///
/// This trait separates display concerns from mode identity. The kernel's
/// `Mode` trait provides identity only; this trait adds display behavior.
///
/// # Thread Safety
///
/// This trait requires `Send + Sync` to allow modes to be used in
/// multi-threaded server contexts (tokio runtime).
///
/// # Example
///
/// ```ignore
/// use reovim_driver_display::{ModeDisplay, CursorStyle};
///
/// struct InsertMode;
///
/// impl ModeDisplay for InsertMode {
///     fn cursor_style(&self) -> CursorStyle {
///         CursorStyle::Bar
///     }
///
///     fn status_text(&self) -> &'static str {
///         "INSERT"
///     }
/// }
/// ```
pub trait ModeDisplay: Send + Sync {
    /// Get the cursor style for this mode.
    ///
    /// The cursor style provides visual feedback about the current mode.
    fn cursor_style(&self) -> CursorStyle;

    /// Get the status text for this mode.
    ///
    /// This text is typically displayed in the statusline.
    /// Default is empty string.
    fn status_text(&self) -> &'static str {
        ""
    }
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cursor_style_default() {
        let style = CursorStyle::default();
        assert_eq!(style, CursorStyle::Block);
    }

    #[test]
    fn test_cursor_style_is_visible() {
        assert!(CursorStyle::Block.is_visible());
        assert!(CursorStyle::Bar.is_visible());
        assert!(CursorStyle::Underline.is_visible());
        assert!(!CursorStyle::Hidden.is_visible());
    }

    #[test]
    fn test_cursor_style_name() {
        assert_eq!(CursorStyle::Block.name(), "block");
        assert_eq!(CursorStyle::Bar.name(), "bar");
        assert_eq!(CursorStyle::Underline.name(), "underline");
        assert_eq!(CursorStyle::Hidden.name(), "hidden");
    }

    #[test]
    fn test_mode_display_trait_object_safety() {
        // Verify ModeDisplay trait is object-safe
        fn _accepts_ref(_: &dyn ModeDisplay) {}
        fn _accepts_box(_: Box<dyn ModeDisplay>) {}
    }

    // Test implementation
    struct TestMode {
        cursor: CursorStyle,
        status: &'static str,
    }

    impl ModeDisplay for TestMode {
        fn cursor_style(&self) -> CursorStyle {
            self.cursor
        }

        fn status_text(&self) -> &'static str {
            self.status
        }
    }

    #[test]
    fn test_mode_display_implementation() {
        let mode = TestMode {
            cursor: CursorStyle::Bar,
            status: "TEST",
        };
        assert_eq!(mode.cursor_style(), CursorStyle::Bar);
        assert_eq!(mode.status_text(), "TEST");
    }

    #[test]
    fn test_mode_display_default_status() {
        struct MinimalMode;

        impl ModeDisplay for MinimalMode {
            fn cursor_style(&self) -> CursorStyle {
                CursorStyle::Block
            }
        }

        let mode = MinimalMode;
        assert_eq!(mode.status_text(), "");
    }
}
