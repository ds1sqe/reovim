//! Window management trait.
//!
//! Provides the mechanism for window lifecycle management.
//!
//! # Design Principle - MECHANISM VS POLICY
//!
//! **CRITICAL**: Kernel Window has **NO POSITION** fields.
//! Position is a **POLICY decision** made by `LayoutPolicy` in `modules/layout/`.
//!
//! This follows the River/wlroots model where compositor (mechanism) and
//! window manager (policy) are separate.
//!
//! | Component | Location | Type | Responsibility |
//! |-----------|----------|------|----------------|
//! | `Window` | Kernel | Data | `buffer_id`, viewport (**NO POSITION**) |
//! | `WindowManager` | Kernel | Service | Lifecycle: create/get/close |
//! | `WindowView` | Display Driver | Data | Window + screen position |
//! | `LayoutPolicy` | Display Driver | Contract | Interface for layout |
//! | `TilingLayout` | Layout Module | **Policy** | WHERE windows go |
//!
//! # Example
//!
//! ```ignore
//! use reovim_kernel::api::v1::*;
//!
//! // Create a window (mechanism) - no position specified
//! let window_id = windows.create(buffer_id);
//!
//! // Position is decided by LayoutPolicy (policy) - not shown here
//! ```

use std::sync::Arc;

use reovim_arch::sync::RwLock;

use crate::mm::BufferId;

// ============================================================================
// WindowId
// ============================================================================

/// Unique window identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowId(u64);

impl WindowId {
    /// Create a new unique window ID.
    #[must_use]
    pub fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT_ID: AtomicU64 = AtomicU64::new(1);
        Self(NEXT_ID.fetch_add(1, Ordering::Relaxed))
    }

    /// Get the raw ID value.
    #[must_use]
    pub const fn as_u64(self) -> u64 {
        self.0
    }
}

impl Default for WindowId {
    fn default() -> Self {
        Self::new()
    }
}

// ============================================================================
// Viewport
// ============================================================================

/// Viewport into a buffer (visible line range).
///
/// This is **MECHANISM** - what portion of the buffer is visible.
/// Position on screen is **POLICY** (decided by `LayoutPolicy` in `modules/layout/`).
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::Viewport;
///
/// // View lines 0-49 (50 visible lines)
/// let viewport = Viewport {
///     top_line: 0,
///     height: 50,
/// };
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewport {
    /// First visible line (0-indexed).
    pub top_line: usize,
    /// Number of visible lines.
    pub height: usize,
}

impl Viewport {
    /// Create a new viewport.
    #[must_use]
    pub const fn new(top_line: usize, height: usize) -> Self {
        Self { top_line, height }
    }

    /// Get the last visible line (exclusive).
    #[must_use]
    pub const fn bottom_line(&self) -> usize {
        self.top_line + self.height
    }

    /// Check if a line is visible in this viewport.
    #[must_use]
    pub const fn contains_line(&self, line: usize) -> bool {
        line >= self.top_line && line < self.top_line + self.height
    }

    /// Scroll the viewport by offset (positive = down, negative = up).
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // saturating_add with cast
    pub fn scrolled_by(self, offset: isize) -> Self {
        let new_top = if offset < 0 {
            self.top_line.saturating_sub(offset.unsigned_abs())
        } else {
            #[allow(clippy::cast_sign_loss)] // Positive value checked above
            self.top_line.saturating_add(offset as usize)
        };
        Self {
            top_line: new_top,
            height: self.height,
        }
    }
}

impl Default for Viewport {
    fn default() -> Self {
        Self {
            top_line: 0,
            height: 24, // Reasonable default terminal height
        }
    }
}

// ============================================================================
// Window
// ============================================================================

/// Window data - **MECHANISM ONLY**.
///
/// **CRITICAL**: This struct has **NO POSITION FIELDS**.
/// Position is a **POLICY decision** made by `LayoutPolicy` trait
/// (implemented in `modules/layout/`).
///
/// The kernel only tracks:
/// - Which buffer is displayed
/// - What portion of the buffer is visible (viewport)
///
/// Screen position (x, y, width) is handled by the Display Driver
/// via `WindowView` and `LayoutPolicy`.
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::{Window, WindowId, Viewport, BufferId};
///
/// let window = Window {
///     id: WindowId::new(),
///     buffer_id: BufferId::new(),
///     viewport: Viewport::new(0, 50),
///     // NOTE: No x, y, width fields!
/// };
/// ```
#[derive(Debug)]
pub struct Window {
    /// Unique window identifier.
    pub id: WindowId,
    /// Buffer displayed in this window.
    pub buffer_id: BufferId,
    /// Viewport (which lines are visible).
    pub viewport: Viewport,
    // ========================================
    // NO POSITION FIELDS!
    // ========================================
    // Position (x, y, width, height) is POLICY.
    // Decided by LayoutPolicy in modules/layout/.
    // Display Driver uses WindowView for positioned windows.
}

impl Window {
    /// Create a new window for a buffer.
    #[must_use]
    pub fn new(id: WindowId, buffer_id: BufferId) -> Self {
        Self {
            id,
            buffer_id,
            viewport: Viewport::default(),
        }
    }

    /// Create a window with a specific viewport.
    #[must_use]
    pub const fn with_viewport(id: WindowId, buffer_id: BufferId, viewport: Viewport) -> Self {
        Self {
            id,
            buffer_id,
            viewport,
        }
    }
}

// ============================================================================
// WindowError
// ============================================================================

/// Window management errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WindowError {
    /// Window not found.
    NotFound(WindowId),
    /// Buffer not found (cannot create window for missing buffer).
    BufferNotFound(BufferId),
}

impl std::fmt::Display for WindowError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::NotFound(id) => write!(f, "window not found: {id:?}"),
            Self::BufferNotFound(id) => write!(f, "buffer not found: {id:?}"),
        }
    }
}

impl std::error::Error for WindowError {}

// ============================================================================
// WindowManager Trait
// ============================================================================

/// Window lifecycle manager - **MECHANISM**.
///
/// Provides: create, get, close, list windows.
///
/// Does **NOT** provide:
/// - Positioning (x, y, width) - that's **POLICY** (`LayoutPolicy`)
/// - Layout rules - that's **POLICY** (`TilingLayout`, etc.)
/// - Focus order - that's **POLICY** (`FocusPolicy`)
///
/// # Implementors
///
/// The kernel provides a default implementation. Modules use this
/// trait to manage window lifecycle without knowing layout details.
///
/// # Example
///
/// ```ignore
/// use reovim_kernel::api::v1::*;
///
/// // Create window (mechanism) - no position specified
/// let id = windows.create(buffer_id);
///
/// // Get window to access its buffer/viewport
/// if let Some(win) = windows.get(id) {
///     let w = win.read();
///     let buffer_id = w.buffer_id;
///     let top_line = w.viewport.top_line;
/// }
///
/// // Close window
/// windows.close(id)?;
/// ```
pub trait WindowManager: Send + Sync {
    /// Create a new window for a buffer.
    ///
    /// Returns the new window's ID. Position is NOT set here -
    /// that's handled by `LayoutPolicy` in the display driver.
    fn create(&self, buffer_id: BufferId) -> WindowId;

    /// Get a window by ID.
    ///
    /// Returns `None` if the window doesn't exist.
    fn get(&self, id: WindowId) -> Option<Arc<RwLock<Window>>>;

    /// Close a window.
    ///
    /// # Errors
    ///
    /// Returns `WindowError::NotFound` if the window doesn't exist.
    fn close(&self, id: WindowId) -> Result<(), WindowError>;

    /// List all window IDs.
    fn list(&self) -> Vec<WindowId>;

    /// Get the count of open windows.
    fn count(&self) -> usize;
}

// ============================================================================
// Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_window_id_unique() {
        let id1 = WindowId::new();
        let id2 = WindowId::new();
        assert_ne!(id1, id2);
    }

    #[test]
    fn test_viewport_contains_line() {
        let viewport = Viewport::new(10, 20); // lines 10-29
        assert!(!viewport.contains_line(9));
        assert!(viewport.contains_line(10));
        assert!(viewport.contains_line(20));
        assert!(viewport.contains_line(29));
        assert!(!viewport.contains_line(30));
    }

    #[test]
    fn test_viewport_scrolled_by() {
        let viewport = Viewport::new(10, 20);

        let scrolled_down = viewport.scrolled_by(5);
        assert_eq!(scrolled_down.top_line, 15);
        assert_eq!(scrolled_down.height, 20);

        let scrolled_up = viewport.scrolled_by(-5);
        assert_eq!(scrolled_up.top_line, 5);

        // Test saturation at 0
        let scrolled_way_up = viewport.scrolled_by(-100);
        assert_eq!(scrolled_way_up.top_line, 0);
    }

    #[test]
    fn test_window_new() {
        let id = WindowId::new();
        let buffer_id = BufferId::new();
        let window = Window::new(id, buffer_id);

        assert_eq!(window.id, id);
        assert_eq!(window.buffer_id, buffer_id);
        assert_eq!(window.viewport.top_line, 0);
    }

    #[test]
    fn test_window_error_display() {
        let err = WindowError::NotFound(WindowId::new());
        assert!(err.to_string().contains("window not found"));

        let err = WindowError::BufferNotFound(BufferId::new());
        assert!(err.to_string().contains("buffer not found"));
    }
}
