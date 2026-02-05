//! Session types for driver-layer state.
//!
//! This module provides types for managing session state at the driver layer.
//!
//! # Design (#471)
//!
//! - **`SessionShared`**: Shared session infrastructure (compositor, `terminal_size`)
//! - **`Session`**: DEPRECATED - legacy type being migrated to `SessionShared`
//! - **`ClientId`**: Unique client connection identifier
//! - **Viewport**: Client viewport dimensions and scroll
//! - **Window**: Single window with buffer reference
//! - **`WindowLayout`**: Window arrangement for a session
//! - **`TextObjRange`**: Range computed by text object commands
//!
//! # Architecture (#471)
//!
//! Per-client state (mode, cursor, selection) lives in `server::EditingState`.
//! The driver layer provides ONLY shared infrastructure via `SessionShared`.
//! `DriverRuntime` (formerly `SessionRuntime`) borrows both to operate on
//! the correct client's state.

use {
    reovim_driver_display::layout::RootCompositor,
    reovim_kernel::api::v1::{BufferId, ModeId, ModeStack, Position, WindowId},
};

use crate::{api::Selection as ApiSelection, extension::ExtensionMap};

// ============================================================================
// SessionShared - Shared Session Infrastructure (#471)
// ============================================================================

/// Shared session infrastructure.
///
/// Contains state that is shared across all clients in a session:
/// - Compositor for window layout management
/// - Default terminal size
/// - Active buffer ID (session-level)
///
/// # Architecture (#471)
///
/// Per-client state lives in `server::EditingState`, NOT here.
/// This type provides only the truly shared infrastructure.
///
/// ```text
/// ┌─────────────────────────────────────────────────────────────────┐
/// │ SERVER LAYER                                                    │
/// │   Session                                                       │
/// │   └── clients: HashMap<ClientId, Client>                        │
/// │       └── Client                                                │
/// │           └── state: EditingState  ◄─── OWNS per-client state   │
/// │               ├── mode_stack                                    │
/// │               ├── windows (with cursors!)                       │
/// │               └── extensions                                    │
/// └─────────────────────────────────────────────────────────────────┘
/// ┌─────────────────────────────────────────────────────────────────┐
/// │ DRIVER LAYER                                                    │
/// │   SessionShared  ◄─── Truly shared infrastructure               │
/// │   ├── compositor                                                │
/// │   ├── terminal_size                                             │
/// │   └── active_buffer                                             │
/// └─────────────────────────────────────────────────────────────────┘
/// ```
///
/// # Migration from `Session`
///
/// `Session` is being replaced by `SessionShared`. The following fields:
/// - `mode_stack`, `pending_keys`, `extensions` → now in `EditingState`
/// - `windows` → now in `EditingState` (per-client cursors)
/// - `id: ClientId` → now in `server::Client`
pub struct SessionShared {
    /// Window compositor for layout management.
    ///
    /// The compositor manages window geometry, splits, and navigation.
    /// This is set by the layout module during session initialization.
    pub compositor: Option<Box<dyn RootCompositor>>,

    /// Currently active buffer ID.
    ///
    /// This is a session-level concern - all clients attached to this
    /// session see the same active buffer.
    active_buffer: Option<BufferId>,

    /// Terminal dimensions (width, height) as session-level default.
    ///
    /// Per-client dimensions may override this via `ClientViewport`.
    /// Default: (80, 24) - standard VT100 size.
    terminal_size: (u16, u16),
}

impl std::fmt::Debug for SessionShared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionShared")
            .field("compositor", &self.compositor.as_ref().map(|_| "..."))
            .field("active_buffer", &self.active_buffer)
            .field("terminal_size", &self.terminal_size)
            .finish()
    }
}

impl Default for SessionShared {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionShared {
    /// Create a new shared session infrastructure.
    ///
    /// Terminal size defaults to VT100 standard (80x24).
    /// Compositor is initialized as `None` and should be set by the layout module.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            compositor: None,
            active_buffer: None,
            terminal_size: (80, 24), // VT100 default
        }
    }

    /// Set the compositor for this session.
    ///
    /// Called by the layout module during session initialization.
    pub fn set_compositor(&mut self, compositor: Box<dyn RootCompositor>) {
        self.compositor = Some(compositor);
    }

    /// Get a reference to the compositor.
    #[must_use]
    pub fn compositor(&self) -> Option<&dyn RootCompositor> {
        self.compositor.as_deref()
    }

    /// Get a mutable reference to the compositor.
    pub fn compositor_mut(&mut self) -> Option<&mut (dyn RootCompositor + 'static)> {
        self.compositor.as_deref_mut()
    }

    /// Get the active buffer ID.
    #[must_use]
    pub const fn active_buffer(&self) -> Option<BufferId> {
        self.active_buffer
    }

    /// Set the active buffer ID.
    pub const fn set_active_buffer(&mut self, id: Option<BufferId>) {
        self.active_buffer = id;
    }

    /// Get terminal dimensions (width, height).
    ///
    /// This is the session-level default. Per-client dimensions
    /// may differ and are stored in `ClientViewport`.
    #[must_use]
    pub const fn terminal_size(&self) -> (u16, u16) {
        self.terminal_size
    }

    /// Set terminal dimensions.
    pub const fn set_terminal_size(&mut self, width: u16, height: u16) {
        self.terminal_size = (width, height);
    }
}

// ============================================================================
// TextObjRange
// ============================================================================

/// Range computed by a text object command.
///
/// Text objects like `iw`, `aw`, `i"` compute a range directly instead of
/// moving the cursor. When executed during operator-pending mode, they store
/// the range for the operator resolver to consume.
///
/// # Coordinate System
///
/// - `start`: First position in the range (inclusive)
/// - `end`: First position AFTER the range (exclusive)
///
/// This matches Rust's standard `Range<Position>` semantics.
///
/// # Example Flow
///
/// 1. User presses `d` → enters operator-pending mode
/// 2. User presses `iw` → `InnerWord` command executes
/// 3. Command calculates word boundaries and creates `TextObjRange`
/// 4. Command stores range in session extension state
/// 5. Operator resolver's `on_command_complete` consumes the range
/// 6. Delete operation uses the range
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextObjRange {
    /// Start position (inclusive).
    pub start: Position,
    /// End position (exclusive - points to first char NOT in range).
    pub end: Position,
    /// Whether this is a linewise text object (e.g., `ip`, `ap`).
    ///
    /// Linewise text objects affect entire lines, similar to linewise
    /// motions like `j`, `k`, `gg`, `G`.
    pub is_linewise: bool,
}

impl TextObjRange {
    /// Create a new characterwise text object range.
    ///
    /// Characterwise ranges operate on a specific range of characters,
    /// like `iw` (inner word) or `i"` (inside quotes).
    #[must_use]
    pub const fn characterwise(start: Position, end: Position) -> Self {
        Self {
            start,
            end,
            is_linewise: false,
        }
    }

    /// Create a new linewise text object range.
    ///
    /// Linewise ranges affect entire lines, like `ip` (inner paragraph)
    /// or `ap` (a paragraph).
    #[must_use]
    pub const fn linewise(start: Position, end: Position) -> Self {
        Self {
            start,
            end,
            is_linewise: true,
        }
    }

    /// Check if this range is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.start.line == self.end.line && self.start.column == self.end.column
    }
}

/// Unique client connection identifier.
///
/// Each terminal/TUI that connects to the server gets a unique `ClientId`.
/// IDs are monotonically increasing and not reused after disconnect.
///
/// # Semantics
///
/// - **Client**: Individual connection to the server (like tmux clients)
/// - **Session**: Named editing context (defined in runner layer)
/// - Multiple clients can attach to the same session
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct ClientId(pub usize);

impl ClientId {
    /// Create a new client ID.
    #[must_use]
    pub const fn new(id: usize) -> Self {
        Self(id)
    }

    /// Get the raw ID value.
    #[must_use]
    pub const fn as_usize(&self) -> usize {
        self.0
    }
}

impl std::fmt::Display for ClientId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "client-{}", self.0)
    }
}

/// Viewport for a client session.
///
/// Represents the visible area of a buffer in a window.
/// Tracks both vertical and horizontal scroll positions.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Viewport {
    /// Width in columns.
    pub width: u16,
    /// Height in rows.
    pub height: u16,
    /// Vertical scroll offset (first visible line, 0-indexed).
    pub scroll_top: usize,
    /// Horizontal scroll offset (first visible column, 0-indexed).
    pub scroll_left: usize,
}

impl Viewport {
    /// Create a new viewport.
    #[must_use]
    pub const fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            scroll_top: 0,
            scroll_left: 0,
        }
    }

    /// Create a default viewport (80x24).
    #[must_use]
    pub const fn default_size() -> Self {
        Self::new(80, 24)
    }

    /// Get the last visible line index.
    #[must_use]
    pub const fn last_visible_line(&self) -> usize {
        self.scroll_top + self.height as usize - 1
    }

    /// Get the last visible column index.
    #[must_use]
    pub const fn last_visible_column(&self) -> usize {
        self.scroll_left + self.width as usize - 1
    }

    /// Check if a line is visible.
    #[must_use]
    pub const fn is_line_visible(&self, line: usize) -> bool {
        line >= self.scroll_top && line <= self.last_visible_line()
    }

    /// Check if a column is visible.
    #[must_use]
    pub const fn is_column_visible(&self, column: usize) -> bool {
        column >= self.scroll_left && column <= self.last_visible_column()
    }

    /// Check if a position (line, column) is visible.
    #[must_use]
    pub const fn is_position_visible(&self, line: usize, column: usize) -> bool {
        self.is_line_visible(line) && self.is_column_visible(column)
    }
}

impl Default for Viewport {
    fn default() -> Self {
        Self::default_size()
    }
}

/// Cursor position within a window.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct CursorPosition {
    /// Line number (0-indexed).
    pub line: usize,
    /// Column number (0-indexed).
    pub column: usize,
}

impl CursorPosition {
    /// Create a new cursor position.
    #[must_use]
    pub const fn new(line: usize, column: usize) -> Self {
        Self { line, column }
    }

    /// Create cursor at origin (0, 0).
    #[must_use]
    pub const fn origin() -> Self {
        Self::new(0, 0)
    }
}

impl From<Position> for CursorPosition {
    fn from(pos: Position) -> Self {
        Self::new(pos.line, pos.column)
    }
}

impl From<CursorPosition> for Position {
    fn from(cursor: CursorPosition) -> Self {
        Self::new(cursor.line, cursor.column)
    }
}

/// Window within a session.
///
/// A window displays a portion of a buffer. Sessions can have multiple
/// windows (splits), each with its own viewport and cursor.
///
/// # Per-Window Selection (Phase 8 #465)
///
/// Selection is stored per-window, not per-buffer. This enables:
/// - Multi-window same-buffer independence (different selections in each window)
/// - Multi-client selection isolation (each client's visual mode is independent)
///
/// Uses API's `Selection` type with explicit start/end, NOT kernel's Selection.
#[derive(Debug, Clone)]
pub struct Window {
    /// Unique window identifier.
    pub id: WindowId,
    /// Buffer displayed in this window (if any).
    pub buffer_id: Option<BufferId>,
    /// Cursor position within the buffer.
    pub cursor: CursorPosition,
    /// Visible viewport.
    pub viewport: Viewport,
    /// Selection state for visual mode (Phase 8 #465).
    ///
    /// Uses API's Selection type with explicit start/end positions.
    /// `None` means no active selection.
    pub selection: Option<ApiSelection>,
}

impl Window {
    /// Create a new empty window.
    #[must_use]
    pub fn new() -> Self {
        Self {
            id: WindowId::new(),
            buffer_id: None,
            cursor: CursorPosition::origin(),
            viewport: Viewport::default(),
            selection: None,
        }
    }

    /// Create a window displaying a buffer.
    #[must_use]
    pub fn with_buffer(buffer_id: BufferId) -> Self {
        Self {
            id: WindowId::new(),
            buffer_id: Some(buffer_id),
            cursor: CursorPosition::origin(),
            viewport: Viewport::default(),
            selection: None,
        }
    }
}

impl Default for Window {
    fn default() -> Self {
        Self::new()
    }
}

/// Window layout for a session.
///
/// Manages the windows in a session, including which window is active.
#[derive(Debug, Default, Clone)]
pub struct WindowLayout {
    /// All windows in this session.
    pub windows: Vec<Window>,
    /// Currently active window index.
    active_index: Option<usize>,
}

impl WindowLayout {
    /// Create an empty layout.
    #[must_use]
    pub const fn empty() -> Self {
        Self {
            windows: Vec::new(),
            active_index: None,
        }
    }

    /// Create a layout with one window.
    #[must_use]
    pub fn single(window: Window) -> Self {
        Self {
            windows: vec![window],
            active_index: Some(0),
        }
    }

    /// Add a window to the layout.
    ///
    /// If this is the first window, it becomes active.
    pub fn add(&mut self, window: Window) {
        self.windows.push(window);
        if self.active_index.is_none() {
            self.active_index = Some(0);
        }
    }

    /// Get the active window.
    ///
    /// If no window is explicitly active but windows exist, returns the first window.
    /// This ensures a valid window is always available when the layout is non-empty.
    #[must_use]
    pub fn active(&self) -> Option<&Window> {
        // Try explicit active index first, fallback to first window
        if let Some(idx) = self.active_index
            && let Some(window) = self.windows.get(idx)
        {
            return Some(window);
        }
        // Fallback: return first window if layout is non-empty
        self.windows.first()
    }

    /// Get the active window mutably.
    ///
    /// If no window is explicitly active but windows exist, returns the first window.
    /// This ensures a valid window is always available when the layout is non-empty.
    pub fn active_mut(&mut self) -> Option<&mut Window> {
        // Try explicit active index first, fallback to first window
        if let Some(idx) = self.active_index
            && idx < self.windows.len()
        {
            return self.windows.get_mut(idx);
        }
        // Fallback: return first window if layout is non-empty
        self.windows.first_mut()
    }

    /// Get the active window ID.
    #[must_use]
    pub fn active_id(&self) -> Option<WindowId> {
        self.active().map(|w| w.id)
    }

    /// Set the active window by ID.
    ///
    /// Returns `true` if the window was found and made active.
    pub fn set_active(&mut self, id: WindowId) -> bool {
        if let Some(idx) = self.windows.iter().position(|w| w.id == id) {
            self.active_index = Some(idx);
            true
        } else {
            false
        }
    }

    /// Check if the layout is empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }

    /// Get the number of windows.
    #[must_use]
    pub const fn len(&self) -> usize {
        self.windows.len()
    }

    /// Clear all windows from the layout.
    pub fn clear(&mut self) {
        self.windows.clear();
        self.active_index = None;
    }

    /// Get window by ID.
    #[must_use]
    pub fn get(&self, id: WindowId) -> Option<&Window> {
        self.windows.iter().find(|w| w.id == id)
    }

    /// Get window by ID mutably.
    pub fn get_mut(&mut self, id: WindowId) -> Option<&mut Window> {
        self.windows.iter_mut().find(|w| w.id == id)
    }
}

/// Pending key sequence.
///
/// Accumulates keys that haven't been resolved yet (e.g., `d` waiting for motion).
#[derive(Debug, Clone, Default)]
pub struct KeySequence {
    /// Accumulated key representations.
    keys: Vec<String>,
}

impl KeySequence {
    /// Create an empty sequence.
    #[must_use]
    pub const fn new() -> Self {
        Self { keys: Vec::new() }
    }

    /// Add a key to the sequence.
    pub fn push(&mut self, key: String) {
        self.keys.push(key);
    }

    /// Clear the sequence.
    pub fn clear(&mut self) {
        self.keys.clear();
    }

    /// Check if empty.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.keys.is_empty()
    }

    /// Get the keys.
    #[must_use]
    pub fn keys(&self) -> &[String] {
        &self.keys
    }

    /// Get the sequence as a single string.
    #[must_use]
    pub fn as_string(&self) -> String {
        self.keys.join("")
    }
}

/// Legacy session type - **DEPRECATED, use [`SessionShared`] instead**.
///
/// # Migration Guide (#471)
///
/// This type mixes shared infrastructure with per-client state, causing confusion.
/// It is being replaced by a cleaner separation:
///
/// | Old Field | New Location |
/// |-----------|--------------|
/// | `shared.compositor` | [`SessionShared::compositor`] |
/// | `shared.terminal_size` | [`SessionShared::terminal_size()`] |
/// | `shared.active_buffer` | [`SessionShared::active_buffer()`] |
/// | `mode_stack` | `server::EditingState::mode_stack` (per-client) |
/// | `windows` | `server::EditingState::windows` (per-client) |
/// | `pending_keys` | `server::EditingState::pending_keys` (per-client) |
/// | `extensions` | `server::EditingState::extensions` (per-client) |
/// | `id: ClientId` | `server::Client` owns the ID |
///
/// # Deprecation Note (#471)
///
/// **BOOTSTRAP ONLY** - This type exists for backward compatibility during migration.
/// At runtime, per-client state lives in `EditingState` in the server layer.
///
/// Commands should use `SessionRuntime::new()` (or the upcoming
/// `DriverRuntime`) to operate on per-client state.
///
/// # Architecture
///
/// ```text
/// OLD (confusing):
///   Session { compositor, mode_stack, windows, ... }  // Mixed!
///
/// NEW (clear separation):
///   Session.shared: SessionShared { compositor, terminal_size, active_buffer }
///   EditingState { mode_stack, windows, extensions, ... }  // Per-client
/// ```
pub struct Session {
    /// Unique client identifier.
    pub id: ClientId,
    /// Shared session infrastructure (compositor, `terminal_size`, `active_buffer`).
    ///
    /// # Phase 0.3 (#471)
    ///
    /// This field contains truly shared state that is accessed by all clients.
    /// `SessionRuntime` accesses shared state via `session.shared`.
    pub shared: SessionShared,
    /// Window layout (per-session windows).
    ///
    /// # Deprecation Note (#471)
    ///
    /// **BOOTSTRAP ONLY** - This field exists for session initialization.
    /// At runtime, use `EditingState.windows` for per-client cursor/selection state.
    /// Commands should use `SessionRuntime::new()`.
    pub windows: WindowLayout,
    /// Mode stack (current mode on top).
    ///
    /// # Deprecation Note (#471)
    ///
    /// **BOOTSTRAP ONLY** - This field exists for session initialization.
    /// At runtime, use `EditingState.mode_stack` for per-client mode state.
    /// Commands should use `SessionRuntime::new()`.
    pub mode_stack: ModeStack,
    /// Keys accumulated but not yet processed.
    pub pending_keys: KeySequence,
    /// Module-provided per-session state.
    pub extensions: ExtensionMap,
}

impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("id", &self.id)
            .field("shared", &self.shared)
            .field("windows", &self.windows)
            .field("mode_stack", &self.mode_stack)
            .field("pending_keys", &self.pending_keys)
            .field("extensions", &self.extensions)
            .finish()
    }
}

impl Session {
    /// Create a new session with a home mode.
    ///
    /// The home mode is the bottom of the mode stack and cannot be popped.
    /// Terminal size defaults to VT100 standard (80x24).
    /// Compositor is initialized as `None` and should be set by the layout module.
    #[must_use]
    pub fn new(id: ClientId, home_mode: ModeId) -> Self {
        Self {
            id,
            shared: SessionShared::new(),
            windows: WindowLayout::empty(),
            mode_stack: ModeStack::new(home_mode),
            pending_keys: KeySequence::new(),
            extensions: ExtensionMap::new(),
        }
    }

    /// Set the compositor for this session.
    ///
    /// Called by the layout module during session initialization.
    /// Delegates to `self.shared.set_compositor()`.
    pub fn set_compositor(&mut self, compositor: Box<dyn RootCompositor>) {
        self.shared.set_compositor(compositor);
    }

    /// Get a reference to the compositor.
    ///
    /// Delegates to `self.shared.compositor()`.
    #[must_use]
    pub fn compositor(&self) -> Option<&dyn RootCompositor> {
        self.shared.compositor()
    }

    /// Get a mutable reference to the compositor.
    ///
    /// Delegates to `self.shared.compositor_mut()`.
    pub fn compositor_mut(&mut self) -> Option<&mut (dyn RootCompositor + 'static)> {
        self.shared.compositor_mut()
    }

    /// Get the current mode.
    #[must_use]
    pub fn current_mode(&self) -> &ModeId {
        self.mode_stack.current()
    }

    /// Get the active buffer ID.
    ///
    /// Delegates to `self.shared.active_buffer()`.
    #[must_use]
    pub const fn active_buffer(&self) -> Option<BufferId> {
        self.shared.active_buffer()
    }

    /// Set the active buffer ID.
    ///
    /// Delegates to `self.shared.set_active_buffer()`.
    pub const fn set_active_buffer(&mut self, id: Option<BufferId>) {
        self.shared.set_active_buffer(id);
    }

    /// Get terminal dimensions (width, height).
    ///
    /// This is the session-level default. Per-client dimensions
    /// may differ and are stored in `ClientViewport`.
    /// Delegates to `self.shared.terminal_size()`.
    #[must_use]
    pub const fn terminal_size(&self) -> (u16, u16) {
        self.shared.terminal_size()
    }

    /// Set terminal dimensions.
    ///
    /// Delegates to `self.shared.set_terminal_size()`.
    pub const fn set_terminal_size(&mut self, width: u16, height: u16) {
        self.shared.set_terminal_size(width, height);
    }
}

#[cfg(test)]
mod tests {
    use {super::*, reovim_kernel::api::v1::ModuleId};

    fn test_mode() -> ModeId {
        ModeId::new(ModuleId::new("test"), "normal")
    }

    #[test]
    fn test_client_id() {
        let id = ClientId::new(42);
        assert_eq!(id.as_usize(), 42);
        assert_eq!(id.to_string(), "client-42");
    }

    #[test]
    fn test_viewport() {
        let vp = Viewport::new(100, 50);
        assert_eq!(vp.width, 100);
        assert_eq!(vp.height, 50);
        assert_eq!(vp.scroll_top, 0);
        assert_eq!(vp.scroll_left, 0);
        assert_eq!(vp.last_visible_line(), 49);
        assert_eq!(vp.last_visible_column(), 99);
        assert!(vp.is_line_visible(0));
        assert!(vp.is_line_visible(49));
        assert!(!vp.is_line_visible(50));
        assert!(vp.is_column_visible(0));
        assert!(vp.is_column_visible(99));
        assert!(!vp.is_column_visible(100));
        assert!(vp.is_position_visible(25, 50));
        assert!(!vp.is_position_visible(50, 50));
    }

    #[test]
    fn test_viewport_default() {
        let vp = Viewport::default();
        assert_eq!(vp.width, 80);
        assert_eq!(vp.height, 24);
    }

    #[test]
    fn test_cursor_position() {
        let cursor = CursorPosition::new(5, 10);
        assert_eq!(cursor.line, 5);
        assert_eq!(cursor.column, 10);

        let origin = CursorPosition::origin();
        assert_eq!(origin.line, 0);
        assert_eq!(origin.column, 0);
    }

    #[test]
    fn test_cursor_position_conversion() {
        let pos = Position::new(10, 20);
        let cursor: CursorPosition = pos.into();
        assert_eq!(cursor.line, 10);
        assert_eq!(cursor.column, 20);

        let back: Position = cursor.into();
        assert_eq!(back, pos);
    }

    #[test]
    fn test_window() {
        let window = Window::new();
        assert!(window.buffer_id.is_none());

        let buf_id = BufferId::new();
        let window = Window::with_buffer(buf_id);
        assert_eq!(window.buffer_id, Some(buf_id));
    }

    #[test]
    fn test_window_layout_empty() {
        let layout = WindowLayout::empty();
        assert!(layout.is_empty());
        assert_eq!(layout.len(), 0);
        assert!(layout.active().is_none());
    }

    #[test]
    fn test_window_layout_single() {
        let window = Window::new();
        let id = window.id;
        let layout = WindowLayout::single(window);

        assert!(!layout.is_empty());
        assert_eq!(layout.len(), 1);
        assert_eq!(layout.active_id(), Some(id));
    }

    #[test]
    fn test_window_layout_add() {
        let mut layout = WindowLayout::empty();
        let w1 = Window::new();
        let id1 = w1.id;
        layout.add(w1);

        assert_eq!(layout.active_id(), Some(id1)); // First becomes active

        let w2 = Window::new();
        let id2 = w2.id;
        layout.add(w2);

        assert_eq!(layout.len(), 2);
        assert_eq!(layout.active_id(), Some(id1)); // Still first

        assert!(layout.set_active(id2));
        assert_eq!(layout.active_id(), Some(id2));
    }

    #[test]
    fn test_key_sequence() {
        let mut seq = KeySequence::new();
        assert!(seq.is_empty());

        seq.push("d".to_string());
        seq.push("w".to_string());
        assert!(!seq.is_empty());
        assert_eq!(seq.as_string(), "dw");

        seq.clear();
        assert!(seq.is_empty());
    }

    #[test]
    fn test_session_new() {
        let mode = test_mode();
        let session = Session::new(ClientId::new(1), mode.clone());

        assert_eq!(session.id.as_usize(), 1);
        assert_eq!(session.current_mode(), &mode);
        assert!(session.windows.is_empty());
        assert!(session.pending_keys.is_empty());
        assert!(session.extensions.is_empty());
        // New fields initialized correctly
        assert!(session.active_buffer().is_none());
        assert_eq!(session.terminal_size(), (80, 24)); // VT100 default
    }

    #[test]
    fn test_session_active_buffer() {
        let mode = test_mode();
        let mut session = Session::new(ClientId::new(1), mode);

        // Initially None
        assert!(session.active_buffer().is_none());

        // Set active buffer
        let buf_id = BufferId::new();
        session.set_active_buffer(Some(buf_id));
        assert_eq!(session.active_buffer(), Some(buf_id));

        // Clear active buffer
        session.set_active_buffer(None);
        assert!(session.active_buffer().is_none());
    }

    #[test]
    fn test_session_terminal_size() {
        let mode = test_mode();
        let mut session = Session::new(ClientId::new(1), mode);

        // Default VT100 size
        assert_eq!(session.terminal_size(), (80, 24));

        // Update terminal size
        session.set_terminal_size(120, 40);
        assert_eq!(session.terminal_size(), (120, 40));
    }

    #[test]
    fn test_session_terminal_size_boundaries() {
        let mode = test_mode();
        let mut session = Session::new(ClientId::new(1), mode);

        // Min values
        session.set_terminal_size(0, 0);
        assert_eq!(session.terminal_size(), (0, 0));

        // Near-min
        session.set_terminal_size(1, 1);
        assert_eq!(session.terminal_size(), (1, 1));

        // Max values
        session.set_terminal_size(u16::MAX, u16::MAX);
        assert_eq!(session.terminal_size(), (u16::MAX, u16::MAX));
    }

    // =========================================================================
    // TextObjRange tests
    // =========================================================================

    #[test]
    fn test_window_layout_active_fallback() {
        // Regression test for cursor visibility bug (#465):
        // active() should return first window when active_index is None
        let mut layout = WindowLayout::empty();

        // Empty layout returns None
        assert!(layout.active().is_none());
        assert!(layout.active_id().is_none());

        // Add window but don't set active_index explicitly
        let w1 = Window::new();
        let id1 = w1.id;
        layout.windows.push(w1); // Direct push bypasses active_index setting

        // Fallback should return first window
        assert!(layout.active().is_some());
        assert_eq!(layout.active_id(), Some(id1));

        // active_mut should also work
        assert!(layout.active_mut().is_some());
    }

    #[test]
    fn test_window_layout_active_stale_index() {
        // Test fallback when active_index points to invalid index
        let mut layout = WindowLayout::empty();
        let w1 = Window::new();
        let id1 = w1.id;
        layout.add(w1);

        // Manually set invalid index
        layout.active_index = Some(999);

        // Should fallback to first window
        assert!(layout.active().is_some());
        assert_eq!(layout.active_id(), Some(id1));
    }

    #[test]
    fn test_textobj_range_characterwise() {
        let range = TextObjRange::characterwise(Position::new(0, 0), Position::new(0, 5));
        assert_eq!(range.start, Position::new(0, 0));
        assert_eq!(range.end, Position::new(0, 5));
        assert!(!range.is_linewise);
    }

    #[test]
    fn test_textobj_range_linewise() {
        let range = TextObjRange::linewise(Position::new(1, 0), Position::new(3, 0));
        assert_eq!(range.start, Position::new(1, 0));
        assert_eq!(range.end, Position::new(3, 0));
        assert!(range.is_linewise);
    }

    #[test]
    fn test_textobj_range_is_empty() {
        let empty = TextObjRange::characterwise(Position::new(0, 0), Position::new(0, 0));
        assert!(empty.is_empty());

        let not_empty = TextObjRange::characterwise(Position::new(0, 0), Position::new(0, 1));
        assert!(!not_empty.is_empty());

        let multiline_empty = TextObjRange::linewise(Position::new(1, 0), Position::new(2, 0));
        assert!(!multiline_empty.is_empty()); // Different lines
    }

    // =========================================================================
    // SessionShared tests (#471)
    // =========================================================================

    #[test]
    fn test_session_shared_new() {
        let shared = SessionShared::new();

        assert!(shared.compositor.is_none());
        assert!(shared.active_buffer().is_none());
        assert_eq!(shared.terminal_size(), (80, 24)); // VT100 default
    }

    #[test]
    fn test_session_shared_default() {
        let shared = SessionShared::default();

        assert!(shared.compositor.is_none());
        assert!(shared.active_buffer().is_none());
        assert_eq!(shared.terminal_size(), (80, 24));
    }

    #[test]
    fn test_session_shared_active_buffer() {
        let mut shared = SessionShared::new();

        // Initially None
        assert!(shared.active_buffer().is_none());

        // Set active buffer
        let buf_id = BufferId::new();
        shared.set_active_buffer(Some(buf_id));
        assert_eq!(shared.active_buffer(), Some(buf_id));

        // Clear active buffer
        shared.set_active_buffer(None);
        assert!(shared.active_buffer().is_none());
    }

    #[test]
    fn test_session_shared_terminal_size() {
        let mut shared = SessionShared::new();

        // Default VT100 size
        assert_eq!(shared.terminal_size(), (80, 24));

        // Update terminal size
        shared.set_terminal_size(120, 40);
        assert_eq!(shared.terminal_size(), (120, 40));
    }

    #[test]
    fn test_session_shared_terminal_size_boundaries() {
        let mut shared = SessionShared::new();

        // Min values
        shared.set_terminal_size(0, 0);
        assert_eq!(shared.terminal_size(), (0, 0));

        // Near-min
        shared.set_terminal_size(1, 1);
        assert_eq!(shared.terminal_size(), (1, 1));

        // Max values
        shared.set_terminal_size(u16::MAX, u16::MAX);
        assert_eq!(shared.terminal_size(), (u16::MAX, u16::MAX));
    }

    #[test]
    fn test_session_shared_debug() {
        let shared = SessionShared::new();
        let debug_str = format!("{shared:?}");

        assert!(debug_str.contains("SessionShared"));
        assert!(debug_str.contains("compositor"));
        assert!(debug_str.contains("terminal_size"));
    }
}
