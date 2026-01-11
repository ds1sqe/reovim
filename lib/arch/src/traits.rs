//! Platform-agnostic trait definitions.
//!
//! These traits define the interface between the kernel and platform-specific
//! implementations. Linux equivalent: platform-independent kernel headers.
//!
//! All types defined here have NO external dependencies (std only).

use std::{io, time::Duration};

// =============================================================================
// Terminal Size
// =============================================================================

/// Terminal size with pixel dimensions for advanced rendering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct TerminalSize {
    /// Number of columns (characters).
    pub cols: u16,
    /// Number of rows (characters).
    pub rows: u16,
    /// Pixel width (if available, 0 otherwise).
    pub pixel_width: u16,
    /// Pixel height (if available, 0 otherwise).
    pub pixel_height: u16,
}

impl TerminalSize {
    /// Create a new terminal size.
    #[must_use]
    pub const fn new(cols: u16, rows: u16) -> Self {
        Self {
            cols,
            rows,
            pixel_width: 0,
            pixel_height: 0,
        }
    }

    /// Create a new terminal size with pixel dimensions.
    #[must_use]
    pub const fn with_pixels(cols: u16, rows: u16, pixel_width: u16, pixel_height: u16) -> Self {
        Self {
            cols,
            rows,
            pixel_width,
            pixel_height,
        }
    }
}

// =============================================================================
// Key Modifiers
// =============================================================================

/// Key modifiers (bitflags-style, no external dependencies).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct Modifiers(u8);

impl Modifiers {
    /// No modifiers.
    pub const NONE: Self = Self(0);
    /// Shift key.
    pub const SHIFT: Self = Self(1 << 0);
    /// Control key.
    pub const CTRL: Self = Self(1 << 1);
    /// Alt/Option key.
    pub const ALT: Self = Self(1 << 2);
    /// Super/Meta/Command key.
    pub const SUPER: Self = Self(1 << 3);
    /// Hyper key (rare).
    pub const HYPER: Self = Self(1 << 4);
    /// Meta key (rare, distinct from Super on some systems).
    pub const META: Self = Self(1 << 5);

    /// Check if a modifier is set.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Combine two modifier sets.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Check if no modifiers are set.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }

    /// Get the raw bits.
    #[must_use]
    pub const fn bits(self) -> u8 {
        self.0
    }

    /// Create from raw bits.
    #[must_use]
    pub const fn from_bits(bits: u8) -> Self {
        Self(bits)
    }
}

impl std::ops::BitOr for Modifiers {
    type Output = Self;

    fn bitor(self, rhs: Self) -> Self::Output {
        self.union(rhs)
    }
}

impl std::ops::BitOrAssign for Modifiers {
    fn bitor_assign(&mut self, rhs: Self) {
        self.0 |= rhs.0;
    }
}

// =============================================================================
// Key Codes
// =============================================================================

/// Platform-agnostic key codes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum KeyCode {
    /// A character key.
    Char(char),
    /// Function key (F1-F12, or higher on some terminals).
    F(u8),
    /// Backspace key.
    Backspace,
    /// Enter/Return key.
    Enter,
    /// Tab key.
    Tab,
    /// Backspace in some terminals.
    BackTab,
    /// Escape key.
    Escape,
    /// Arrow keys.
    Up,
    Down,
    Left,
    Right,
    /// Navigation keys.
    Home,
    End,
    PageUp,
    PageDown,
    /// Editing keys.
    Insert,
    Delete,
    /// Null character (Ctrl+@).
    Null,
    /// Caps lock (if reported).
    CapsLock,
    /// Scroll lock (if reported).
    ScrollLock,
    /// Num lock (if reported).
    NumLock,
    /// Print screen (if reported).
    PrintScreen,
    /// Pause key (if reported).
    Pause,
    /// Menu key (if reported).
    Menu,
    /// Keypad begin (center key on keypad).
    KeypadBegin,
    /// Media keys.
    MediaPlay,
    MediaPause,
    MediaPlayPause,
    MediaStop,
    MediaReverse,
    MediaFastForward,
    MediaRewind,
    MediaNext,
    MediaPrevious,
    MediaRecord,
    MediaLowerVolume,
    MediaRaiseVolume,
    MediaMuteVolume,
    /// Modifier keys (if reported as separate events).
    LeftShift,
    RightShift,
    LeftCtrl,
    RightCtrl,
    LeftAlt,
    RightAlt,
    LeftSuper,
    RightSuper,
    LeftHyper,
    RightHyper,
    LeftMeta,
    RightMeta,
    IsoLevel3Shift,
    IsoLevel5Shift,
}

/// Key event kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum KeyEventKind {
    /// Key was pressed.
    #[default]
    Press,
    /// Key is being held (repeat).
    Repeat,
    /// Key was released.
    Release,
}

/// Key event state (for enhanced keyboard protocols).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub struct KeyEventState(u8);

impl KeyEventState {
    /// No state.
    pub const NONE: Self = Self(0);
    /// Key is from keypad.
    pub const KEYPAD: Self = Self(1 << 0);
    /// Caps lock is active.
    pub const CAPS_LOCK: Self = Self(1 << 1);
    /// Num lock is active.
    pub const NUM_LOCK: Self = Self(1 << 2);

    /// Check if a state flag is set.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        (self.0 & other.0) == other.0
    }

    /// Combine two state sets.
    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    /// Check if no state flags are set.
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.0 == 0
    }
}

/// Key event with modifiers and kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyEvent {
    /// The key code.
    pub code: KeyCode,
    /// Active modifiers.
    pub modifiers: Modifiers,
    /// Event kind (press, repeat, release).
    pub kind: KeyEventKind,
    /// Additional state (keypad, caps lock, etc.).
    pub state: KeyEventState,
}

impl KeyEvent {
    /// Create a new key event with just a key code.
    #[must_use]
    pub const fn new(code: KeyCode) -> Self {
        Self {
            code,
            modifiers: Modifiers::NONE,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }

    /// Create a key event with modifiers.
    #[must_use]
    pub const fn with_modifiers(code: KeyCode, modifiers: Modifiers) -> Self {
        Self {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: KeyEventState::NONE,
        }
    }
}

// =============================================================================
// Mouse Events
// =============================================================================

/// Mouse button.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    /// Left mouse button.
    Left,
    /// Right mouse button.
    Right,
    /// Middle mouse button (wheel click).
    Middle,
}

/// Mouse event kind.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseEventKind {
    /// Button pressed down.
    Down(MouseButton),
    /// Button released.
    Up(MouseButton),
    /// Mouse dragged with button held.
    Drag(MouseButton),
    /// Mouse moved (without button).
    Moved,
    /// Scroll up.
    ScrollUp,
    /// Scroll down.
    ScrollDown,
    /// Scroll left (horizontal).
    ScrollLeft,
    /// Scroll right (horizontal).
    ScrollRight,
}

/// Mouse event with position and modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MouseEvent {
    /// The event kind.
    pub kind: MouseEventKind,
    /// Column (0-based).
    pub column: u16,
    /// Row (0-based).
    pub row: u16,
    /// Active modifiers.
    pub modifiers: Modifiers,
}

// =============================================================================
// Input Events
// =============================================================================

/// Union of all input events from the terminal.
#[derive(Debug, Clone)]
pub enum InputEvent {
    /// Keyboard event.
    Key(KeyEvent),
    /// Mouse event.
    Mouse(MouseEvent),
    /// Terminal resize event.
    Resize(TerminalSize),
    /// Terminal gained focus.
    FocusGained,
    /// Terminal lost focus.
    FocusLost,
    /// Bracketed paste content.
    Paste(String),
}

// =============================================================================
// Clear Type
// =============================================================================

/// Terminal clear type.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClearType {
    /// Clear entire screen.
    All,
    /// Clear from cursor to end of screen.
    FromCursorDown,
    /// Clear from cursor to start of screen.
    FromCursorUp,
    /// Clear current line.
    CurrentLine,
    /// Clear from cursor to end of line.
    UntilNewLine,
    /// Purge scrollback buffer (if supported).
    Purge,
}

// =============================================================================
// Raw Mode Guard
// =============================================================================

/// RAII guard for raw mode - restores terminal on drop.
///
/// When this guard is dropped, it automatically restores the terminal
/// to its previous state (typically cooked mode).
pub struct RawModeGuard {
    restore_fn: Option<Box<dyn FnOnce() -> io::Result<()> + Send>>,
}

impl RawModeGuard {
    /// Create a new raw mode guard with a restore function.
    pub fn new<F>(restore_fn: F) -> Self
    where
        F: FnOnce() -> io::Result<()> + Send + 'static,
    {
        Self {
            restore_fn: Some(Box::new(restore_fn)),
        }
    }

    /// Create a disabled guard that does nothing on drop.
    #[must_use]
    pub fn disabled() -> Self {
        Self { restore_fn: None }
    }

    /// Take the restore function without running it.
    ///
    /// After calling this, the guard will not restore on drop.
    pub fn take(&mut self) -> Option<Box<dyn FnOnce() -> io::Result<()> + Send>> {
        self.restore_fn.take()
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        if let Some(f) = self.restore_fn.take() {
            // Ignore errors in drop - can't propagate them
            let _ = f();
        }
    }
}

// =============================================================================
// Terminal Trait
// =============================================================================

/// Terminal backend abstraction.
///
/// Provides size queries, raw mode control, cursor manipulation, and screen management.
/// This trait is the primary interface for terminal I/O operations.
#[allow(clippy::missing_errors_doc)]
pub trait Terminal: Send + Sync {
    /// Get terminal dimensions.
    ///
    /// Returns the current terminal size in characters and optionally pixels.
    fn size(&self) -> io::Result<TerminalSize>;

    /// Enable raw mode.
    ///
    /// In raw mode:
    /// - Input is not line-buffered
    /// - Echo is disabled
    /// - Special key combinations are passed through (Ctrl+C, etc.)
    ///
    /// Returns a guard that restores the terminal on drop.
    fn enable_raw_mode(&mut self) -> io::Result<RawModeGuard>;

    /// Check if keyboard enhancement protocol is supported.
    ///
    /// The keyboard enhancement protocol (kitty keyboard protocol) allows
    /// distinguishing keys like Ctrl+I from Tab.
    fn supports_keyboard_enhancement(&self) -> bool;

    /// Enable keyboard enhancement protocol if supported.
    fn enable_keyboard_enhancement(&mut self) -> io::Result<()>;

    /// Disable keyboard enhancement protocol.
    fn disable_keyboard_enhancement(&mut self) -> io::Result<()>;

    /// Enter alternate screen buffer.
    ///
    /// The alternate screen isolates the editor UI from the shell.
    fn enter_alternate_screen(&mut self) -> io::Result<()>;

    /// Leave alternate screen buffer.
    ///
    /// Restores the original shell content.
    fn leave_alternate_screen(&mut self) -> io::Result<()>;

    /// Hide the cursor.
    fn hide_cursor(&mut self) -> io::Result<()>;

    /// Show the cursor.
    fn show_cursor(&mut self) -> io::Result<()>;

    /// Move cursor to position (0-based).
    fn move_cursor(&mut self, col: u16, row: u16) -> io::Result<()>;

    /// Clear terminal.
    fn clear(&mut self, clear_type: ClearType) -> io::Result<()>;

    /// Enable mouse capture.
    ///
    /// Mouse events will be reported as [`InputEvent::Mouse`].
    fn enable_mouse_capture(&mut self) -> io::Result<()>;

    /// Disable mouse capture.
    fn disable_mouse_capture(&mut self) -> io::Result<()>;

    /// Write bytes to the terminal.
    fn write(&mut self, buf: &[u8]) -> io::Result<usize>;

    /// Write a string to the terminal.
    fn write_str(&mut self, s: &str) -> io::Result<()> {
        self.write(s.as_bytes())?;
        Ok(())
    }

    /// Flush output buffer.
    fn flush(&mut self) -> io::Result<()>;
}

// =============================================================================
// Input Source Trait
// =============================================================================

/// Input source abstraction.
///
/// Provides a blocking API for reading terminal input events.
/// This is the low-level interface; higher layers may wrap this with async.
#[allow(clippy::missing_errors_doc)]
pub trait InputSource: Send {
    /// Poll for input with timeout.
    ///
    /// Returns `true` if an event is available, `false` if timeout elapsed.
    fn poll(&mut self, timeout: Duration) -> io::Result<bool>;

    /// Read next input event.
    ///
    /// This is a blocking call. Use `poll` first to check for availability.
    fn read_event(&mut self) -> io::Result<InputEvent>;

    /// Drain all pending events.
    ///
    /// Returns a vector of all currently queued events.
    fn drain(&mut self) -> Vec<InputEvent> {
        let mut events = Vec::new();
        while self.poll(Duration::ZERO).unwrap_or(false) {
            if let Ok(event) = self.read_event() {
                events.push(event);
            }
        }
        events
    }
}

// =============================================================================
// Signal Handler Trait
// =============================================================================

/// Signal handler abstraction.
///
/// Provides callbacks for OS signals. Note that on Unix with crossterm,
/// resize signals (SIGWINCH) are typically delivered as [`InputEvent::Resize`]
/// through the input source rather than through this handler.
pub trait SignalHandler: Send + Sync {
    /// Register handler for terminal resize.
    ///
    /// Note: On most platforms with crossterm, resize is delivered via
    /// [`InputEvent::Resize`] instead of through this handler.
    fn on_resize(&mut self, handler: Box<dyn Fn(TerminalSize) + Send + Sync>);

    /// Register handler for interrupt (Ctrl+C / SIGINT).
    fn on_interrupt(&mut self, handler: Box<dyn Fn() + Send + Sync>);

    /// Register handler for suspend (Ctrl+Z / SIGTSTP).
    fn on_suspend(&mut self, handler: Box<dyn Fn() + Send + Sync>);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_modifiers_operations() {
        let shift = Modifiers::SHIFT;
        let ctrl = Modifiers::CTRL;
        let combined = shift | ctrl;

        assert!(combined.contains(Modifiers::SHIFT));
        assert!(combined.contains(Modifiers::CTRL));
        assert!(!combined.contains(Modifiers::ALT));
        assert!(!combined.is_empty());
        assert!(Modifiers::NONE.is_empty());
    }

    #[test]
    fn test_terminal_size() {
        let size = TerminalSize::new(80, 24);
        assert_eq!(size.cols, 80);
        assert_eq!(size.rows, 24);
        assert_eq!(size.pixel_width, 0);
        assert_eq!(size.pixel_height, 0);

        let size_with_pixels = TerminalSize::with_pixels(80, 24, 800, 480);
        assert_eq!(size_with_pixels.pixel_width, 800);
        assert_eq!(size_with_pixels.pixel_height, 480);
    }

    #[test]
    fn test_key_event_creation() {
        let key = KeyEvent::new(KeyCode::Char('a'));
        assert_eq!(key.code, KeyCode::Char('a'));
        assert_eq!(key.modifiers, Modifiers::NONE);
        assert_eq!(key.kind, KeyEventKind::Press);

        let ctrl_a = KeyEvent::with_modifiers(KeyCode::Char('a'), Modifiers::CTRL);
        assert!(ctrl_a.modifiers.contains(Modifiers::CTRL));
    }

    #[test]
    fn test_raw_mode_guard_disabled() {
        let guard = RawModeGuard::disabled();
        drop(guard); // Should not panic
    }
}
