//! Driver-level events (hardware/service abstraction layer).
//!
//! Linux equivalent: Events similar to device driver notifications
//!
//! These events represent hardware-level or service-level notifications:
//! - Display driver: resize, frame rendered
//! - Input driver: key/mouse input
//!
//! # Design Philosophy
//!
//! Driver events bridge the gap between hardware (terminal) and kernel:
//! - **Input driver**: Translates raw terminal events to kernel types
//! - **Display driver**: Manages frame rendering and terminal size
//!
//! # Example
//!
//! ```
//! use reovim_kernel::api::v1::{Event, EventBus, EventResult};
//! use reovim_kernel::api::v1::events::driver::{DisplayResized, KeyInput, Modifiers};
//!
//! let bus = EventBus::new();
//!
//! // Subscribe to display resize
//! let _sub = bus.subscribe::<DisplayResized, _>(0, |event| {
//!     println!("Terminal resized to {}x{}", event.width, event.height);
//!     EventResult::Handled
//! });
//!
//! // Subscribe to key input
//! let _sub = bus.subscribe::<KeyInput, _>(0, |event| {
//!     println!("Key: {:?}", event.key);
//!     EventResult::Handled
//! });
//!
//! bus.emit(DisplayResized { width: 120, height: 40 });
//! ```

use crate::ipc::Event;

// =============================================================================
// Display Driver Events
// =============================================================================

/// Terminal display was resized.
///
/// Emitted by the display driver when the terminal size changes.
/// Handlers should adjust layouts and re-render as needed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DisplayResized {
    /// New width in columns
    pub width: u16,
    /// New height in rows
    pub height: u16,
}

impl Event for DisplayResized {}

/// A frame was rendered to the display.
///
/// Emitted after the display driver completes a frame render.
/// Useful for performance monitoring and synchronization.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FrameRendered {
    /// Frame sequence number
    pub frame_id: u64,
}

impl Event for FrameRendered {}

// =============================================================================
// Input Driver Events
// =============================================================================

/// Key input received from the terminal.
///
/// Emitted by the input driver when a key is pressed.
/// This is the low-level key event before any keybinding processing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyInput {
    /// The key code
    pub key: KeyCode,
    /// Active modifiers (Ctrl, Alt, Shift)
    pub modifiers: Modifiers,
}

impl Event for KeyInput {}

/// Key code representation.
///
/// Represents the physical/logical key pressed.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum KeyCode {
    /// Regular character
    Char(char),
    /// Function key (F1-F12)
    F(u8),
    /// Backspace
    Backspace,
    /// Enter/Return
    Enter,
    /// Left arrow
    Left,
    /// Right arrow
    Right,
    /// Up arrow
    Up,
    /// Down arrow
    Down,
    /// Home
    Home,
    /// End
    End,
    /// Page Up
    PageUp,
    /// Page Down
    PageDown,
    /// Tab
    Tab,
    /// Backab (Shift+Tab)
    BackTab,
    /// Delete
    Delete,
    /// Insert
    Insert,
    /// Escape
    Esc,
    /// Null (Ctrl+Space on some terminals)
    Null,
}

impl KeyCode {
    /// Check if this is a character key.
    #[must_use]
    pub const fn is_char(&self) -> bool {
        matches!(self, Self::Char(_))
    }

    /// Get the character if this is a character key.
    #[must_use]
    pub const fn as_char(&self) -> Option<char> {
        match self {
            Self::Char(c) => Some(*c),
            _ => None,
        }
    }
}

/// Keyboard modifier flags.
///
/// Represents modifier keys that can be combined with other keys.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[allow(clippy::struct_excessive_bools)] // Keyboard modifiers are naturally represented as bools
pub struct Modifiers {
    /// Control key is held
    pub ctrl: bool,
    /// Alt/Option key is held
    pub alt: bool,
    /// Shift key is held
    pub shift: bool,
    /// Super/Meta/Windows key is held
    pub super_key: bool,
}

impl Modifiers {
    /// No modifiers.
    pub const NONE: Self = Self {
        ctrl: false,
        alt: false,
        shift: false,
        super_key: false,
    };

    /// Control modifier only.
    pub const CTRL: Self = Self {
        ctrl: true,
        alt: false,
        shift: false,
        super_key: false,
    };

    /// Alt modifier only.
    pub const ALT: Self = Self {
        ctrl: false,
        alt: true,
        shift: false,
        super_key: false,
    };

    /// Shift modifier only.
    pub const SHIFT: Self = Self {
        ctrl: false,
        alt: false,
        shift: true,
        super_key: false,
    };

    /// Check if no modifiers are active.
    #[must_use]
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub const fn is_empty(&self) -> bool {
        !self.ctrl && !self.alt && !self.shift && !self.super_key
    }

    /// Check if any modifier is active.
    #[must_use]
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub const fn any(&self) -> bool {
        self.ctrl || self.alt || self.shift || self.super_key
    }
}

/// Mouse input received from the terminal.
///
/// Emitted by the input driver for mouse events.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MouseInput {
    /// Type of mouse event
    pub event: MouseEvent,
    /// Column position (0-indexed)
    pub column: u16,
    /// Row position (0-indexed)
    pub row: u16,
    /// Active keyboard modifiers
    pub modifiers: Modifiers,
}

impl Event for MouseInput {}

/// Mouse event types.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseEvent {
    /// Button was pressed
    Down(MouseButton),
    /// Button was released
    Up(MouseButton),
    /// Mouse was dragged (with button held)
    Drag(MouseButton),
    /// Mouse moved (no button)
    Moved,
    /// Scroll wheel
    ScrollUp,
    /// Scroll wheel
    ScrollDown,
    /// Horizontal scroll left
    ScrollLeft,
    /// Horizontal scroll right
    ScrollRight,
}

/// Mouse button identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum MouseButton {
    /// Left mouse button
    Left,
    /// Right mouse button
    Right,
    /// Middle mouse button (scroll wheel click)
    Middle,
}
