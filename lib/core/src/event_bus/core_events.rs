//! Core events that are available to all plugins
//!
//! These events represent fundamental editor operations that plugins commonly
//! need to react to. They use low priority numbers (0-50) to ensure core
//! handlers run before plugin handlers.

use super::Event;

/// Priority constants for core events
pub mod priority {
    /// Highest priority - for critical core handlers
    pub const CRITICAL: u32 = 0;
    /// High priority - for core system handlers
    pub const CORE: u32 = 10;
    /// Normal priority - for standard core handlers
    pub const NORMAL: u32 = 50;
    /// Default priority for plugins
    pub const PLUGIN: u32 = 100;
    /// Low priority - for cleanup/finalization handlers
    pub const LOW: u32 = 200;
}

// =============================================================================
// Buffer Events
// =============================================================================

/// A buffer was created
#[derive(Debug, Clone)]
pub struct BufferCreated {
    /// ID of the created buffer
    pub buffer_id: usize,
}

impl Event for BufferCreated {
    fn priority(&self) -> u32 {
        priority::CORE
    }
}

/// A buffer was closed
#[derive(Debug, Clone)]
pub struct BufferClosed {
    /// ID of the closed buffer
    pub buffer_id: usize,
}

impl Event for BufferClosed {
    fn priority(&self) -> u32 {
        priority::CORE
    }
}

/// A buffer's content was modified
#[derive(Debug, Clone)]
pub struct BufferModified {
    /// ID of the modified buffer
    pub buffer_id: usize,
    /// Type of modification
    pub modification: BufferModification,
}

impl Event for BufferModified {
    fn priority(&self) -> u32 {
        priority::NORMAL
    }
}

/// Type of buffer modification
#[derive(Debug, Clone)]
pub enum BufferModification {
    /// Text was inserted
    Insert {
        /// Start position (line, column)
        start: (usize, usize),
        /// Inserted text
        text: String,
    },
    /// Text was deleted
    Delete {
        /// Start position (line, column)
        start: (usize, usize),
        /// End position (line, column)
        end: (usize, usize),
        /// Deleted text
        text: String,
    },
    /// Text was replaced
    Replace {
        /// Start position (line, column)
        start: (usize, usize),
        /// End position (line, column)
        end: (usize, usize),
        /// Old text that was replaced
        old_text: String,
        /// New text
        new_text: String,
    },
    /// Buffer content was fully replaced (e.g., file load)
    FullReplace,
}

/// Active buffer changed
#[derive(Debug, Clone)]
pub struct BufferSwitched {
    /// Previous buffer ID (if any)
    pub from: Option<usize>,
    /// New active buffer ID
    pub to: usize,
}

impl Event for BufferSwitched {
    fn priority(&self) -> u32 {
        priority::CORE
    }
}

/// Buffer was saved to disk
#[derive(Debug, Clone)]
pub struct BufferSaved {
    /// ID of the saved buffer
    pub buffer_id: usize,
    /// Path the buffer was saved to
    pub path: String,
}

impl Event for BufferSaved {
    fn priority(&self) -> u32 {
        priority::NORMAL
    }
}

// =============================================================================
// Mode Events
// =============================================================================

/// Editor mode changed
#[derive(Debug, Clone)]
pub struct ModeChanged {
    /// Previous mode description
    pub from: String,
    /// New mode description
    pub to: String,
}

impl Event for ModeChanged {
    fn priority(&self) -> u32 {
        priority::CORE
    }
}

// =============================================================================
// Cursor Events
// =============================================================================

/// Cursor position changed
#[derive(Debug, Clone)]
pub struct CursorMoved {
    /// Buffer ID where cursor moved
    pub buffer_id: usize,
    /// Previous position (line, column)
    pub from: (usize, usize),
    /// New position (line, column)
    pub to: (usize, usize),
}

impl Event for CursorMoved {
    fn priority(&self) -> u32 {
        priority::NORMAL
    }
}

// =============================================================================
// Window Events
// =============================================================================

/// Window was created
#[derive(Debug, Clone)]
pub struct WindowCreated {
    /// ID of the created window
    pub window_id: usize,
}

impl Event for WindowCreated {
    fn priority(&self) -> u32 {
        priority::CORE
    }
}

/// Window was closed
#[derive(Debug, Clone)]
pub struct WindowClosed {
    /// ID of the closed window
    pub window_id: usize,
}

impl Event for WindowClosed {
    fn priority(&self) -> u32 {
        priority::CORE
    }
}

/// Active window changed
#[derive(Debug, Clone)]
pub struct WindowFocused {
    /// Previous window ID (if any)
    pub from: Option<usize>,
    /// New active window ID
    pub to: usize,
}

impl Event for WindowFocused {
    fn priority(&self) -> u32 {
        priority::CORE
    }
}

/// Screen was resized
#[derive(Debug, Clone, Copy)]
pub struct ScreenResized {
    /// New width in columns
    pub width: u16,
    /// New height in rows
    pub height: u16,
}

impl Event for ScreenResized {
    fn priority(&self) -> u32 {
        priority::CRITICAL
    }
}

/// Request to change the focused component
///
/// Plugins emit this event to request focus change. The runtime subscribes
/// to this event and updates `ModeState.interactor_id` to route keybindings
/// to the newly focused component.
#[derive(Debug, Clone)]
pub struct RequestFocusChange {
    /// The component ID to focus
    pub target: crate::ui_component::ComponentId,
}

impl Event for RequestFocusChange {
    fn priority(&self) -> u32 {
        priority::CORE
    }
}

/// Request to open a file in the editor
///
/// Plugins emit this event to request opening a file. The runtime subscribes
/// to this event and creates a new buffer with the file content.
#[derive(Debug, Clone)]
pub struct RequestOpenFile {
    /// Path to the file to open
    pub path: std::path::PathBuf,
}

impl Event for RequestOpenFile {
    fn priority(&self) -> u32 {
        priority::CORE
    }
}

// =============================================================================
// Lifecycle Events
// =============================================================================

/// Request a render of the UI
#[derive(Debug, Clone, Copy)]
pub struct RenderRequest;

impl Event for RenderRequest {
    fn priority(&self) -> u32 {
        priority::LOW
    }
}

/// Request the editor to quit
#[derive(Debug, Clone)]
pub struct QuitRequest {
    /// Whether to force quit (ignore unsaved changes)
    pub force: bool,
}

impl Event for QuitRequest {
    fn priority(&self) -> u32 {
        priority::CRITICAL
    }
}

/// Editor is about to shut down
///
/// Plugins can use this to perform cleanup.
#[derive(Debug, Clone, Copy)]
pub struct ShutdownEvent;

impl Event for ShutdownEvent {
    fn priority(&self) -> u32 {
        priority::CRITICAL
    }
}

// =============================================================================
// File Events
// =============================================================================

/// A file was opened
#[derive(Debug, Clone)]
pub struct FileOpened {
    /// Buffer ID the file was loaded into
    pub buffer_id: usize,
    /// Path of the opened file
    pub path: String,
}

impl Event for FileOpened {
    fn priority(&self) -> u32 {
        priority::NORMAL
    }
}

/// File type was detected/changed
#[derive(Debug, Clone)]
pub struct FileTypeChanged {
    /// Buffer ID
    pub buffer_id: usize,
    /// Detected file type (e.g., "rust", "python", "markdown")
    pub file_type: String,
}

impl Event for FileTypeChanged {
    fn priority(&self) -> u32 {
        priority::NORMAL
    }
}

// =============================================================================
// Fold Events
// =============================================================================

use crate::folding::FoldRange;

/// Fold ranges were computed for a buffer (e.g., by treesitter)
#[derive(Debug, Clone)]
pub struct FoldRangesComputed {
    /// Buffer ID where fold ranges were computed
    pub buffer_id: usize,
    /// The computed fold ranges
    pub ranges: Vec<FoldRange>,
}

impl Event for FoldRangesComputed {
    fn priority(&self) -> u32 {
        priority::NORMAL
    }
}
