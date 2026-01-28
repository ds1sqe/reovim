//! Command line popup window management.
//!
//! Manages the overlay window for the floating command line UI.
//! Uses the `OverlayLayer` trait from the display driver.
//!
//! # Thread Safety
//!
//! `SharedCmdlinePopupState` provides thread-safe access via `RwLock` and
//! implements `Service` for registration in `ServiceRegistry`.

use {
    reovim_arch::sync::RwLock,
    reovim_driver_display::{
        WindowId,
        layout::{Anchor, OverlayConstraints},
    },
    reovim_driver_session::CmdlinePrompt,
    reovim_kernel::api::v1::Service,
};

/// Reserved window ID for the cmdline popup.
///
/// Uses a high ID to avoid collision with buffer windows.
/// This matches the pattern used in layout module (overlays start at 2000).
pub const CMDLINE_POPUP_WINDOW_ID: usize = 3000;

/// Default popup width (percentage of screen width).
pub const POPUP_WIDTH_PERCENT: u16 = 60;

/// Maximum popup width in cells.
pub const POPUP_MAX_WIDTH: u16 = 80;

/// Minimum popup width in cells.
pub const POPUP_MIN_WIDTH: u16 = 20;

/// Popup height (1 line for input + 2 for borders).
pub const POPUP_HEIGHT: u16 = 3;

/// Distance from top of screen (rows).
pub const POPUP_TOP_OFFSET: u16 = 1;

/// State for the cmdline popup window.
///
/// Tracks visibility and current content for the floating cmdline UI.
/// The actual overlay management is delegated to the compositor layer.
#[derive(Debug, Clone, Default)]
pub struct CmdlinePopupState {
    /// Whether the popup is currently visible.
    visible: bool,
    /// Current prompt type.
    prompt: CmdlinePrompt,
    /// Cached input text for rendering.
    input: String,
    /// Cursor position within input.
    cursor: usize,
    /// Screen width (for calculating popup width).
    screen_width: u16,
}

impl CmdlinePopupState {
    /// Create a new popup state.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            visible: false,
            prompt: CmdlinePrompt::Command,
            input: String::new(),
            cursor: 0,
            screen_width: 80, // Default screen width
        }
    }

    /// Get the window ID for this popup.
    #[must_use]
    pub const fn window_id() -> WindowId {
        WindowId::from_raw(CMDLINE_POPUP_WINDOW_ID)
    }

    /// Show the popup with specified prompt.
    pub fn show(&mut self, prompt: CmdlinePrompt) {
        self.visible = true;
        self.prompt = prompt;
        self.input.clear();
        self.cursor = 0;
    }

    /// Hide the popup.
    pub fn hide(&mut self) {
        self.visible = false;
        self.input.clear();
        self.cursor = 0;
    }

    /// Update the screen width (for calculating popup width).
    #[allow(clippy::missing_const_for_fn)] // Mutating self prevents const
    pub fn set_screen_width(&mut self, width: u16) {
        self.screen_width = width;
    }

    /// Update the content to render.
    pub fn update_content(&mut self, input: &str, cursor: usize) {
        self.input = input.to_string();
        self.cursor = cursor;
    }

    /// Check if the popup is visible.
    #[must_use]
    pub const fn is_visible(&self) -> bool {
        self.visible
    }

    /// Get the current prompt.
    #[must_use]
    pub const fn prompt(&self) -> CmdlinePrompt {
        self.prompt
    }

    /// Get the current input text.
    #[must_use]
    pub fn input(&self) -> &str {
        &self.input
    }

    /// Get the cursor position.
    #[must_use]
    pub const fn cursor(&self) -> usize {
        self.cursor
    }

    /// Calculate the popup width based on screen size.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)] // Result clamped to POPUP_MAX_WIDTH (80)
    pub fn popup_width(&self) -> u16 {
        // 60% of screen width, clamped to min/max
        let width = (u32::from(self.screen_width) * u32::from(POPUP_WIDTH_PERCENT) / 100) as u16;
        width.clamp(POPUP_MIN_WIDTH, POPUP_MAX_WIDTH)
    }

    /// Create overlay constraints for the popup.
    ///
    /// Positions the popup centered horizontally, near the top of the screen.
    #[must_use]
    pub fn constraints(&self) -> OverlayConstraints {
        let width = self.popup_width();

        // Center horizontally
        let x = if self.screen_width > width {
            (self.screen_width - width) / 2
        } else {
            0
        };

        OverlayConstraints {
            anchor: Anchor::Screen {
                x,
                y: POPUP_TOP_OFFSET,
            },
            preferred_width: Some(width),
            preferred_height: Some(POPUP_HEIGHT),
            max_width: Some(POPUP_MAX_WIDTH),
            max_height: Some(POPUP_HEIGHT),
        }
    }
}

// ============================================================================
// SharedCmdlinePopupState - Thread-Safe Wrapper
// ============================================================================

/// Thread-safe wrapper for cmdline popup state.
///
/// Provides interior mutability via `RwLock` and implements `Service`
/// for registration in `ServiceRegistry`.
///
/// # Example
///
/// ```ignore
/// use reovim_module_cmdline::popup::SharedCmdlinePopupState;
///
/// let state = SharedCmdlinePopupState::new();
/// state.show(CmdlinePrompt::Command);
/// state.update_content("set number", 10);
/// ```
pub struct SharedCmdlinePopupState {
    inner: RwLock<CmdlinePopupState>,
}

impl std::fmt::Debug for SharedCmdlinePopupState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedCmdlinePopupState").finish()
    }
}

impl Default for SharedCmdlinePopupState {
    fn default() -> Self {
        Self::new()
    }
}

impl SharedCmdlinePopupState {
    /// Create new shared popup state.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)] // RwLock::new is not const
    pub fn new() -> Self {
        Self {
            inner: RwLock::new(CmdlinePopupState::new()),
        }
    }

    /// Show the popup with specified prompt.
    pub fn show(&self, prompt: CmdlinePrompt) {
        self.inner.write().show(prompt);
    }

    /// Hide the popup.
    pub fn hide(&self) {
        self.inner.write().hide();
    }

    /// Update the screen width.
    pub fn set_screen_width(&self, width: u16) {
        self.inner.write().set_screen_width(width);
    }

    /// Update the content to render.
    pub fn update_content(&self, input: &str, cursor: usize) {
        self.inner.write().update_content(input, cursor);
    }

    /// Check if the popup is visible.
    #[must_use]
    pub fn is_visible(&self) -> bool {
        self.inner.read().is_visible()
    }

    /// Get the current prompt.
    #[must_use]
    pub fn prompt(&self) -> CmdlinePrompt {
        self.inner.read().prompt()
    }

    /// Get a copy of the current input text.
    #[must_use]
    pub fn input(&self) -> String {
        self.inner.read().input().to_string()
    }

    /// Get the cursor position.
    #[must_use]
    pub fn cursor(&self) -> usize {
        self.inner.read().cursor()
    }

    /// Get the popup width.
    #[must_use]
    pub fn popup_width(&self) -> u16 {
        self.inner.read().popup_width()
    }

    /// Create overlay constraints for the popup.
    #[must_use]
    pub fn constraints(&self) -> OverlayConstraints {
        self.inner.read().constraints()
    }

    /// Get the window ID for this popup.
    #[must_use]
    pub const fn window_id() -> WindowId {
        CmdlinePopupState::window_id()
    }
}

// Implement Service trait for ServiceRegistry registration
impl Service for SharedCmdlinePopupState {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_popup_new_default_hidden() {
        let popup = CmdlinePopupState::new();
        assert!(!popup.is_visible());
        assert_eq!(popup.prompt(), CmdlinePrompt::Command);
        assert!(popup.input().is_empty());
        assert_eq!(popup.cursor(), 0);
    }

    #[test]
    fn test_popup_show_sets_visible() {
        let mut popup = CmdlinePopupState::new();
        popup.show(CmdlinePrompt::SearchForward);

        assert!(popup.is_visible());
        assert_eq!(popup.prompt(), CmdlinePrompt::SearchForward);
    }

    #[test]
    fn test_popup_hide_clears_visible() {
        let mut popup = CmdlinePopupState::new();
        popup.show(CmdlinePrompt::Command);
        popup.update_content("test", 4);

        popup.hide();

        assert!(!popup.is_visible());
        assert!(popup.input().is_empty());
        assert_eq!(popup.cursor(), 0);
    }

    #[test]
    fn test_popup_update_content() {
        let mut popup = CmdlinePopupState::new();
        popup.update_content("set number", 10);

        assert_eq!(popup.input(), "set number");
        assert_eq!(popup.cursor(), 10);
    }

    #[test]
    fn test_popup_constraints_width() {
        let mut popup = CmdlinePopupState::new();
        popup.set_screen_width(100);

        let width = popup.popup_width();
        // 60% of 100 = 60, clamped to max 80
        assert_eq!(width, 60);

        popup.set_screen_width(200);
        let width = popup.popup_width();
        // 60% of 200 = 120, clamped to max 80
        assert_eq!(width, 80);

        popup.set_screen_width(20);
        let width = popup.popup_width();
        // 60% of 20 = 12, clamped to min 20
        assert_eq!(width, 20);
    }

    #[test]
    fn test_popup_constraints_position() {
        let mut popup = CmdlinePopupState::new();
        popup.set_screen_width(100);

        let constraints = popup.constraints();

        // Width is 60 (60% of 100), centered: x = (100 - 60) / 2 = 20
        if let Anchor::Screen { x, y } = constraints.anchor {
            assert_eq!(x, 20);
            assert_eq!(y, POPUP_TOP_OFFSET);
        } else {
            panic!("Expected Screen anchor");
        }

        assert_eq!(constraints.preferred_height, Some(POPUP_HEIGHT));
    }

    #[test]
    fn test_popup_window_id() {
        let id = CmdlinePopupState::window_id();
        assert_eq!(id.as_usize(), CMDLINE_POPUP_WINDOW_ID);
    }

    #[test]
    fn test_popup_show_clears_previous_content() {
        let mut popup = CmdlinePopupState::new();
        popup.update_content("old content", 5);

        popup.show(CmdlinePrompt::SearchBackward);

        assert!(popup.input().is_empty());
        assert_eq!(popup.cursor(), 0);
    }
}
