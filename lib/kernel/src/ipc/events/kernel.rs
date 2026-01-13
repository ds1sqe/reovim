//! Kernel-level events (mechanism layer).
//!
//! Linux equivalent: Events similar to `struct file_operations` callbacks
//!
//! These events represent fundamental state changes that occurred in the kernel.
//! They are pure **notifications** - they tell you what happened, not what to do about it.
//!
//! # Design Philosophy
//!
//! Following "mechanism, not policy":
//! - Kernel events = notifications (something happened)
//! - Request events = policy (stay in lib/core or plugins)
//!
//! # Priority Constants
//!
//! ```ignore
//! CRITICAL (0): Lifecycle events (Shutdown)
//! CORE (10): System state changes (Buffer/Window lifecycle)
//! NORMAL (50): Content changes (BufferModified, CursorMoved)
//! PLUGIN (100): Default for plugin handlers
//! LOW (200): Cleanup/finalization handlers
//! ```
//!
//! # Example
//!
//! ```
//! use reovim_kernel::api::v1::{Event, EventBus, EventResult};
//! use reovim_kernel::api::v1::events::kernel::{BufferCreated, BufferModified, Modification};
//!
//! let bus = EventBus::new();
//!
//! // Subscribe to buffer creation
//! let _sub = bus.subscribe::<BufferCreated, _>(100, |event| {
//!     println!("Buffer {} created", event.buffer_id);
//!     EventResult::Handled
//! });
//!
//! // Emit when a buffer is created
//! bus.emit(BufferCreated { buffer_id: 1 });
//! ```

use crate::ipc::Event;

/// Priority constants for event handlers.
///
/// Lower numbers = higher priority (run first).
pub mod priority {
    /// Highest priority - for critical handlers (lifecycle events)
    pub const CRITICAL: u32 = 0;
    /// High priority - for core system handlers
    pub const CORE: u32 = 10;
    /// Normal priority - for standard handlers
    pub const NORMAL: u32 = 50;
    /// Default priority for plugins
    pub const PLUGIN: u32 = 100;
    /// Low priority - for cleanup/finalization handlers
    pub const LOW: u32 = 200;
}

// =============================================================================
// Buffer Events
// =============================================================================

/// A buffer was created in the kernel.
///
/// Emitted when `mm::Buffer` is allocated and registered.
/// This is a pure notification - no action is expected from handlers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BufferCreated {
    /// ID of the created buffer
    pub buffer_id: u64,
}

impl Event for BufferCreated {}

/// A buffer was closed/destroyed.
///
/// Emitted when a buffer is deallocated from the kernel.
/// Handlers should clean up any state associated with this buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BufferClosed {
    /// ID of the closed buffer
    pub buffer_id: u64,
}

impl Event for BufferClosed {}

/// A buffer's content was modified.
///
/// Emitted after any change to buffer content (insert, delete, replace).
/// Handlers can use this to trigger re-parsing, update highlights, etc.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferModified {
    /// ID of the modified buffer
    pub buffer_id: u64,
    /// Type of modification that occurred
    pub modification: Modification,
}

impl Event for BufferModified {}

/// Type of buffer modification.
///
/// Describes what kind of change was made to the buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Modification {
    /// Text was inserted at a position.
    Insert {
        /// Start position (line, column) - 0-indexed
        start: (u32, u32),
        /// The inserted text
        text: String,
    },
    /// Text was deleted from a range.
    Delete {
        /// Start position (line, column) - 0-indexed
        start: (u32, u32),
        /// End position (line, column) - 0-indexed
        end: (u32, u32),
        /// The deleted text
        text: String,
    },
    /// Text was replaced (delete + insert combined).
    Replace {
        /// Start position (line, column) - 0-indexed
        start: (u32, u32),
        /// End position (line, column) - 0-indexed
        end: (u32, u32),
        /// Old text that was replaced
        old_text: String,
        /// New text
        new_text: String,
    },
    /// Entire buffer content was replaced (e.g., file reload).
    FullReplace,
}

/// Active buffer changed.
///
/// Emitted when the kernel switches focus to a different buffer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BufferSwitched {
    /// Previous buffer ID (if any)
    pub from: Option<u64>,
    /// New active buffer ID
    pub to: u64,
}

impl Event for BufferSwitched {}

/// A buffer was saved to disk.
///
/// Emitted after buffer content is persisted to the filesystem.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferSaved {
    /// ID of the saved buffer
    pub buffer_id: u64,
    /// Path the buffer was saved to
    pub path: String,
}

impl Event for BufferSaved {}

// =============================================================================
// Cursor Events
// =============================================================================

/// Cursor position changed in a buffer.
///
/// Emitted after the cursor moves to a new position.
/// Note: This is a notification only - it doesn't request movement.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorMoved {
    /// Buffer ID where cursor moved
    pub buffer_id: u64,
    /// Previous position (line, column) - 0-indexed
    pub from: (u32, u32),
    /// New position (line, column) - 0-indexed
    pub to: (u32, u32),
}

impl Event for CursorMoved {}

// =============================================================================
// Mode Events
// =============================================================================

/// Editor mode changed.
///
/// Emitted after a mode transition occurs (e.g., Normal → Insert).
/// The mode strings are intentionally generic - policy (specific modes)
/// is defined by the runtime and modules.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeChanged {
    /// Previous mode description
    pub from: String,
    /// New mode description
    pub to: String,
}

impl Event for ModeChanged {}

// =============================================================================
// Window Events
// =============================================================================

/// A window was created.
///
/// Emitted when a new window is added to the layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowCreated {
    /// ID of the created window
    pub window_id: u64,
}

impl Event for WindowCreated {}

/// A window was closed.
///
/// Emitted when a window is removed from the layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowClosed {
    /// ID of the closed window
    pub window_id: u64,
}

impl Event for WindowClosed {}

/// Active window changed.
///
/// Emitted when focus moves to a different window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WindowFocused {
    /// Previous window ID (if any)
    pub from: Option<u64>,
    /// New active window ID
    pub to: u64,
}

impl Event for WindowFocused {}

/// Viewport scrolled within a window.
///
/// Emitted when the visible portion of a buffer changes due to scrolling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewportScrolled {
    /// ID of the window that scrolled
    pub window_id: u64,
    /// ID of the buffer being viewed
    pub buffer_id: u64,
    /// First visible line (0-indexed)
    pub top_line: u32,
    /// Last visible line (0-indexed)
    pub bottom_line: u32,
}

impl Event for ViewportScrolled {}

// =============================================================================
// File Events
// =============================================================================

/// A file was opened into a buffer.
///
/// Emitted after file content is loaded into a buffer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileOpened {
    /// Buffer ID the file was loaded into
    pub buffer_id: u64,
    /// Path of the opened file
    pub path: String,
}

impl Event for FileOpened {}

/// File type was detected or changed.
///
/// Emitted when the file type (language) is determined or updated.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileTypeChanged {
    /// Buffer ID
    pub buffer_id: u64,
    /// Detected file type (e.g., "rust", "python", "markdown")
    pub file_type: String,
}

impl Event for FileTypeChanged {}

// =============================================================================
// Option Events
// =============================================================================

/// Source of an option value change.
///
/// Indicates how an option was modified, useful for handlers
/// that need to distinguish user commands from programmatic changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ChangeSource {
    /// Changed via `:set` command by the user.
    #[default]
    UserCommand,
    /// Changed programmatically by a plugin/module.
    Plugin,
    /// Loaded from configuration file.
    Config,
    /// Set to default during initialization.
    Default,
    /// Changed via settings menu/UI.
    SettingsMenu,
    /// Changed via RPC (server mode).
    Rpc,
}

/// An option value changed.
///
/// Emitted after any option value is modified via `OptionRegistry::set()`.
/// Handlers can react to option changes (e.g., re-render, update state).
///
/// # Example
///
/// ```ignore
/// use reovim_kernel::api::v1::{EventBus, EventResult, events::kernel::OptionChanged};
///
/// let bus = EventBus::new();
/// bus.subscribe::<OptionChanged, _>(100, |event| {
///     println!("Option '{}' changed from {} to {}",
///         event.name, event.old_value, event.new_value);
///     EventResult::Handled
/// });
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionChanged {
    /// Full option name that changed.
    pub name: String,
    /// Previous value.
    pub old_value: String,
    /// New value.
    pub new_value: String,
    /// Source of the change.
    pub source: ChangeSource,
}

impl Event for OptionChanged {}

/// An option was reset to its default value.
///
/// Emitted when an option is reset via `:set option&` or `OptionRegistry::reset()`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OptionReset {
    /// Option name that was reset.
    pub name: String,
    /// Previous value (before reset).
    pub old_value: String,
    /// Default value (after reset).
    pub default_value: String,
}

impl Event for OptionReset {}

// =============================================================================
// Lifecycle Events
// =============================================================================

/// Kernel is shutting down.
///
/// Emitted before the kernel terminates. Handlers should perform cleanup.
/// This is the last event handlers will receive before termination.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Shutdown;

impl Event for Shutdown {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_buffer_created() {
        let event = BufferCreated { buffer_id: 42 };
        assert_eq!(event.buffer_id, 42);
    }

    #[test]
    fn test_buffer_modified_insert() {
        let event = BufferModified {
            buffer_id: 1,
            modification: Modification::Insert {
                start: (0, 0),
                text: "hello".to_string(),
            },
        };
        assert_eq!(event.buffer_id, 1);
        if let Modification::Insert { start, text } = event.modification {
            assert_eq!(start, (0, 0));
            assert_eq!(text, "hello");
        } else {
            panic!("Expected Insert modification");
        }
    }

    #[test]
    fn test_mode_changed() {
        let event = ModeChanged {
            from: "Normal".to_string(),
            to: "Insert".to_string(),
        };
        assert_eq!(event.from, "Normal");
        assert_eq!(event.to, "Insert");
    }

    #[test]
    fn test_cursor_moved() {
        let event = CursorMoved {
            buffer_id: 1,
            from: (0, 0),
            to: (10, 5),
        };
        assert_eq!(event.from, (0, 0));
        assert_eq!(event.to, (10, 5));
    }

    #[test]
    fn test_shutdown_default() {
        let event = Shutdown;
        assert_eq!(event, Shutdown);
    }

    #[test]
    fn test_priority_constants() {
        const {
            assert!(priority::CRITICAL < priority::CORE);
            assert!(priority::CORE < priority::NORMAL);
            assert!(priority::NORMAL < priority::PLUGIN);
            assert!(priority::PLUGIN < priority::LOW);
        }
    }
}
