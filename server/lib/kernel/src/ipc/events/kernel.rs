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
//! - Kernel events = domain-free substrate notifications (something happened)
//! - Text-domain events = semantic notifications (in `reovim-domain-text-events`)
//! - Request events = policy (stay in modules)
//!
//! # Priority Constants
//!
//! ```ignore
//! CRITICAL (0): Lifecycle events (Shutdown)
//! CORE (10): System state changes (Buffer/Window lifecycle, BufferBytesEdited)
//! NORMAL (50): Content changes
//! PLUGIN (100): Default for plugin handlers
//! LOW (200): Cleanup/finalization handlers
//! ```
//!
//! # Example
//!
//! ```
//! use reovim_kernel::api::v1::{Event, EventBus, EventResult};
//! use reovim_kernel::api::v1::events::kernel::BufferCreated;
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

use crate::{
    block::ByteEdit,
    core::{ModeId, OptionScopeId},
    ipc::Event,
    mm::BufferId,
};

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

/// Byte-layer mutation notification.
///
/// Emitted once per [`ByteEdit`] applied to storage, regardless of origin
/// (local edit, file reload, codec reverse translation, byte undo replay).
///
/// Subscribers that only care about byte changes (byte undo log, network
/// sync, file watcher debouncer) use this.  Tree-sitter and other
/// incremental parsers that need atomic byte-range + point-range
/// correlation should subscribe to `TextBufferModified` (in
/// `reovim-domain-text-events`) instead.
#[derive(Debug, Clone)]
pub struct BufferBytesEdited {
    /// Buffer that was edited.
    pub buffer_id: BufferId,
    /// The byte-level edit that was applied.
    pub edit: ByteEdit,
}

impl Event for BufferBytesEdited {
    fn priority(&self) -> u32 {
        priority::CORE
    }
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

/// A buffer is about to be saved to disk.
///
/// Emitted before buffer content is persisted. Subscribers can modify
/// the buffer content (e.g., format-on-save) and the write command will
/// pick up the formatted content. Dispatch is synchronous.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BufferWillSave {
    /// ID of the buffer about to be saved
    pub buffer_id: u64,
    /// Path the buffer will be saved to
    pub path: String,
}

impl Event for BufferWillSave {}

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
// Mode Events
// =============================================================================

/// Editor mode changed.
///
/// Emitted after a mode transition occurs (e.g., Normal → Insert).
/// The mode strings are intentionally generic - policy (specific modes)
/// is defined by the runtime and modules.
///
/// # Type-Safe Mode Transitions
///
/// When `target_mode` is set, it provides the exact `ModeId` to transition to.
/// This enables event-driven mode transitions without hardcoding command names
/// in the runner layer.
///
/// # Example
///
/// ```ignore
/// // Commands emit with target_mode for type-safe transitions
/// ctx.event_bus.emit(ModeChanged::with_mode_id("normal", editor_visual_mode));
///
/// // Runner subscribes and updates mode_stack from the event
/// bus.subscribe::<ModeChanged, _>(priority::CORE, |event| {
///     if let Some(mode_id) = event.target_mode() {
///         app.mode_stack.set(mode_id.clone());
///     }
///     EventResult::Handled
/// });
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModeChanged {
    /// Previous mode description (for display/logging).
    pub from: String,
    /// New mode description (for display/logging).
    pub to: String,
    /// Target mode ID for type-safe transitions.
    ///
    /// When set, the runner uses this to update the mode stack instead of
    /// predicting mode transitions based on command names.
    target_mode: Option<ModeId>,
}

impl ModeChanged {
    /// Create a new mode change event with string descriptions only.
    ///
    /// Use this for backward compatibility or when the target mode is
    /// implicit (e.g., commands that emit events for logging only).
    #[must_use]
    pub fn new(from: impl Into<String>, to: impl Into<String>) -> Self {
        Self {
            from: from.into(),
            to: to.into(),
            target_mode: None,
        }
    }

    /// Create a mode change event with a specific target mode ID.
    ///
    /// The runner subscribes to these events and updates the mode stack
    /// using the provided `ModeId`, eliminating the need for hardcoded
    /// command name mappings.
    #[must_use]
    pub fn with_mode_id(from: impl Into<String>, target: ModeId) -> Self {
        let to = target.name().to_string();
        Self {
            from: from.into(),
            to,
            target_mode: Some(target),
        }
    }

    /// Get the target mode ID, if set.
    ///
    /// Returns `Some(&ModeId)` when the event was created with `with_mode_id()`,
    /// `None` for legacy events created with just string descriptions.
    #[must_use]
    pub const fn target_mode(&self) -> Option<&ModeId> {
        self.target_mode.as_ref()
    }

    /// Check if this event has a type-safe target mode.
    #[must_use]
    pub const fn has_target_mode(&self) -> bool {
        self.target_mode.is_some()
    }
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

// =============================================================================
// Layout Events
// =============================================================================

/// Direction of a window split.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    /// Horizontal split (windows stacked top/bottom).
    Horizontal,
    /// Vertical split (windows side by side).
    Vertical,
}

/// Type of layout change that occurred.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LayoutChangeKind {
    /// Window was split.
    Split {
        /// ID of the new window created by the split.
        new_window: u64,
        /// Direction of the split.
        direction: SplitDirection,
    },
    /// Window was closed.
    Close {
        /// ID of the closed window.
        closed_window: u64,
        /// ID of the window that received focus (if any).
        new_focus: Option<u64>,
    },
    /// Focus changed to a different window.
    Focus {
        /// Previous focused window (if any).
        from: Option<u64>,
        /// New focused window.
        to: u64,
    },
    /// Window was resized.
    Resize {
        /// ID of the resized window.
        window: u64,
    },
    /// All windows were equalized in size.
    Equalize,
}

/// Layout changed event.
///
/// Emitted after any layout operation (split, close, focus, resize, equalize).
/// This is the primary event for notifying clients of layout changes.
///
/// The runner subscribes to this event and converts it to an RPC notification
/// for connected clients.
///
/// # Example
///
/// ```ignore
/// use reovim_kernel::api::v1::{EventBus, EventResult, events::kernel::LayoutChanged};
///
/// let bus = EventBus::new();
/// let _sub = bus.subscribe::<LayoutChanged, _>(100, |event| {
///     println!("Layout changed: {:?}", event.kind);
///     EventResult::Handled
/// });
/// ```
#[derive(Debug, Clone)]
pub struct LayoutChanged {
    /// Type of layout change that occurred.
    pub kind: LayoutChangeKind,
    /// Total window count after the change.
    pub window_count: usize,
    /// Currently focused window (if any).
    pub focused_window: Option<u64>,
}

impl Event for LayoutChanged {}

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
    /// Runtime scope where the change was applied (which specific buffer or window).
    ///
    /// This is the runtime context, NOT the option's static scope declaration
    /// (see `OptionSpec::scope` for the capability declaration).
    pub scope: OptionScopeId,
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
    /// Runtime scope where the reset was applied (which specific buffer or window).
    ///
    /// This is the runtime context, NOT the option's static scope declaration
    /// (see `OptionSpec::scope` for the capability declaration).
    pub scope: OptionScopeId,
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
