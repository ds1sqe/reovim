//! Mode display traits and types.
//!
//! This module defines how modes affect display rendering.
//! Modes implement `ModeDisplay` to control cursor style and status text.
//!
//! # Architecture
//!
//! | Layer | Responsibility |
//! |-------|---------------|
//! | Kernel | Identity: `Mode` trait, `ModeId`, `CursorStyle` |
//! | Display Driver (this) | Display: `ModeDisplay` (re-exports `CursorStyle`) |
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
// CursorStyle (re-export from kernel)
// ============================================================================

// Re-export CursorStyle from kernel - the kernel owns this type as part of
// the Mode trait. Display driver re-exports for backwards compatibility.
pub use reovim_kernel::api::v1::CursorStyle;

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
#[path = "mode_tests.rs"]
mod tests;
