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
    reovim_driver_layout::RootCompositor,
    reovim_kernel::api::v1::{BufferId, MarkBank, ModeId, ModeStack, WindowId},
    reovim_types_text::{HistoryRing, Position, RegisterBank},
};

use crate::{Jumplist, api::Selection as ApiSelection, extension::ExtensionMap};

// ============================================================================
// SessionShared - Shared Session Infrastructure (#471)
// ============================================================================

/// Shared session infrastructure.
///
/// Contains state that is shared across all clients in a session:
/// - Compositor for window layout management
/// - Default terminal size
/// - Active buffer ID (session-level)
/// - Home mode for initializing new clients
///
/// # Architecture (#471, #491)
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
/// │   ├── compositor  (template for layout)                         │
/// │   └── home_mode   (bootstrap template for new clients, #491)   │
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

    /// Home mode for initializing new clients (#491).
    ///
    /// Bootstrap template for initializing new clients -- each client's
    /// mode evolves independently after init.
    home_mode: ModeId,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl std::fmt::Debug for SessionShared {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SessionShared")
            .field("compositor", &self.compositor.as_ref().map(|_| "..."))
            .field("home_mode", &self.home_mode)
            .finish()
    }
}

// NOTE: Default impl removed in #491 - SessionShared now requires home_mode parameter

impl SessionShared {
    /// Create a new shared session infrastructure with the specified home mode.
    ///
    /// # Parameters
    ///
    /// - `home_mode`: The mode used to initialize new clients' mode stacks
    ///
    /// Compositor is initialized as `None` and should be set by the layout module.
    #[must_use]
    pub fn new(home_mode: ModeId) -> Self {
        Self {
            compositor: None,
            home_mode,
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

    /// Get the home mode for initializing new clients (#491).
    ///
    /// When a new client connects, their mode stack is initialized
    /// with this mode at the bottom.
    #[must_use]
    pub const fn home_mode(&self) -> &ModeId {
        &self.home_mode
    }
}

// ============================================================================
// BootstrapState - Initial Per-Client State for Session Initialization (#488)
// ============================================================================

/// Bootstrap state for creating a new session with initial per-client data.
///
/// This struct provides the initial per-client state needed when initializing
/// a session or adding the first client. It contains fields that will become
/// part of `EditingState` in the server layer.
///
/// # Usage
///
/// ```rust,ignore
/// // Create session with bootstrap state for tests
/// let (session, bootstrap) = Session::bootstrap(ClientId::new(1), home_mode);
///
/// // Use bootstrap fields for per-client state
/// let mode_stack = bootstrap.mode_stack;
/// let windows = bootstrap.windows;
/// ```
///
/// # Architecture (#488)
///
/// This type exists to provide a clean separation between:
/// - **Session**: Shared infrastructure only (`SessionShared`)
/// - **`BootstrapState`**: Initial per-client state (mode, windows, extensions)
///
/// At runtime, per-client state lives in `EditingState` in the server layer.
#[derive(Debug)]
pub struct BootstrapState {
    /// Initial mode stack with home mode at the bottom.
    pub mode_stack: ModeStack,
    /// Initial window layout (empty by default).
    pub windows: WindowLayout,
    /// Initial pending keys (empty by default).
    pub pending_keys: KeySequence,
    /// Initial extensions map (empty by default).
    pub extensions: ExtensionMap,
}

impl BootstrapState {
    /// Create bootstrap state with the given home mode.
    ///
    /// The home mode becomes the bottom of the mode stack.
    /// Windows, pending keys, and extensions start empty.
    #[must_use]
    pub fn new(home_mode: ModeId) -> Self {
        Self {
            mode_stack: ModeStack::new(home_mode),
            windows: WindowLayout::empty(),
            pending_keys: KeySequence::new(),
            extensions: ExtensionMap::new(),
        }
    }

    /// Create bootstrap state with a window for the given buffer.
    ///
    /// This is useful when the session already has an active buffer
    /// and the new client should have a window showing that buffer.
    #[must_use]
    pub fn with_buffer(home_mode: ModeId, buffer_id: BufferId) -> Self {
        let mut state = Self::new(home_mode);
        state.windows.add(Window::with_buffer(buffer_id));
        state
    }
}

// ============================================================================
// ClientContext - Borrowed Per-Client State Bundle (#515)
// ============================================================================

/// Borrowed per-client state bundle for session operations.
///
/// Groups the 7 mutable references to per-client state that [`SessionRuntime`]
/// and command execution require. This replaces passing 7 individual `&mut`
/// parameters through function signatures.
///
/// # Ownership
///
/// `ClientContext` does NOT own the data -- it borrows from `server::EditingState`
/// (or from test fixtures). The owned counterpart is `server::EditingState`.
///
/// # Convention
///
/// Follows the `*Context<'a>` naming convention used throughout the codebase
/// (`OperatorContext`, `HandlerContext`, `CommandContext`, etc.).
///
/// [`SessionRuntime`]: crate::SessionRuntime
pub struct ClientContext<'a> {
    /// Per-client mode stack (current mode on top).
    pub mode_stack: &'a mut ModeStack,
    /// Per-client window layout with independent cursors.
    pub windows: &'a mut WindowLayout,
    /// Per-client module extensions (type-erased state).
    pub extensions: &'a mut ExtensionMap,
    /// Per-client compositor for window layout geometry.
    pub compositor: &'a mut Option<Box<dyn RootCompositor>>,
    /// Per-client tab pages (#401).
    pub tabs: &'a mut crate::TabPageSet,
    /// Per-client register storage (unnamed, named a-z/A-Z).
    pub registers: &'a mut RegisterBank,
    /// Per-client clipboard history ring (numbered registers 0-9).
    pub clipboard_history: &'a mut HistoryRing,
    /// Per-client local marks (a-z).
    pub local_marks: &'a mut MarkBank,
    /// Per-client jump list for Ctrl-O / Ctrl-I navigation (#654).
    pub jumplist: &'a mut Jumplist,
    /// Per-client active buffer (#471).
    ///
    /// Each client tracks which buffer they are viewing independently.
    /// Previously session-level in `SessionShared`, migrated to per-client
    /// so that multi-client sessions don't share active buffer state.
    pub active_buffer: &'a mut Option<BufferId>,
    /// Per-client terminal dimensions (width, height) (#471).
    ///
    /// Each client has independent terminal size for compositor calculations.
    /// Previously hardcoded (80, 24) in `SessionShared`, migrated to per-client
    /// so that clients with different screen sizes get correct layouts.
    pub terminal_size: &'a mut (u16, u16),
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

    /// Adjust `scroll_top` so the cursor line is visible.
    ///
    /// Returns `true` if `scroll_top` was changed.
    pub const fn ensure_cursor_visible(&mut self, cursor_line: usize) -> bool {
        if self.height == 0 {
            return false;
        }
        let old = self.scroll_top;
        let h = self.height as usize;
        if cursor_line < self.scroll_top {
            self.scroll_top = cursor_line;
        } else if cursor_line >= self.scroll_top + h {
            self.scroll_top = cursor_line - h + 1;
        }
        self.scroll_top != old
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

    /// Create a window with a specific ID and buffer (#474).
    ///
    /// Used when creating per-client windows that must match compositor window IDs.
    /// Unlike `with_buffer()`, this does NOT generate a new `WindowId`.
    #[must_use]
    pub fn with_id_and_buffer(id: WindowId, buffer_id: BufferId) -> Self {
        Self {
            id,
            buffer_id: Some(buffer_id),
            cursor: CursorPosition::origin(),
            viewport: Viewport::default(),
            selection: None,
        }
    }

    /// Create a window that inherits state from a source window (#692).
    ///
    /// Used when splitting: the new pane gets the same buffer, cursor position,
    /// and viewport scroll offset as the source. Selection is NOT copied —
    /// splitting does not clone visual mode state.
    #[must_use]
    pub const fn split_from(id: WindowId, source: &Self) -> Self {
        Self {
            id,
            buffer_id: source.buffer_id,
            cursor: source.cursor,
            viewport: source.viewport,
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
    #[cfg_attr(coverage_nightly, coverage(off))]
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

    /// Remove a window by ID (#474).
    ///
    /// Returns `true` if the window was found and removed. If the removed
    /// window was active, the active index is adjusted.
    pub fn remove(&mut self, id: WindowId) -> bool {
        if let Some(idx) = self.windows.iter().position(|w| w.id == id) {
            self.windows.remove(idx);
            // Adjust active index after removal
            match self.active_index {
                Some(active) if active == idx => {
                    // Active window was removed - reset to first window (if any)
                    self.active_index = if self.windows.is_empty() {
                        None
                    } else {
                        Some(0)
                    };
                }
                Some(active) if active > idx => {
                    // Active was after removed - shift down
                    self.active_index = Some(active - 1);
                }
                _ => {}
            }
            true
        } else {
            false
        }
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

/// Driver-layer session containing shared infrastructure.
///
/// # Architecture (#471, #491)
///
/// After the per-client state migration, `Session` contains only:
/// - `id`: Client identifier (placeholder for session-level operations)
/// - `shared`: Truly shared infrastructure (`SessionShared`)
///
/// Per-client state (mode, cursor, selection) now lives in `EditingState`
/// at the server layer. Commands use `SessionRuntime::new()` which borrows
/// both shared infrastructure and per-client state.
///
/// ```text
/// ┌─────────────────────────────────────────────────────────────────┐
/// │ SERVER LAYER                                                    │
/// │   Session (server/lib/server/)                                  │
/// │   └── clients: HashMap<ClientId, Client>                        │
/// │       └── Client                                                │
/// │           └── state: EditingState  ◄─── OWNS per-client state   │
/// │               ├── mode_stack                                    │
/// │               ├── windows (with cursors!)                       │
/// │               ├── pending_keys                                  │
/// │               └── extensions                                    │
/// └─────────────────────────────────────────────────────────────────┘
/// ┌─────────────────────────────────────────────────────────────────┐
/// │ DRIVER LAYER                                                    │
/// │   Session (this struct)                                         │
/// │   ├── id: ClientId (deprecated, not used at runtime #471)       │
/// │   └── shared: SessionShared  ◄─── Truly shared infrastructure   │
/// │       ├── compositor  (template for layout)                     │
/// │       └── home_mode   (bootstrap template for new clients)      │
/// └─────────────────────────────────────────────────────────────────┘
/// ```
pub struct Session {
    /// Client identifier (deprecated -- not used at runtime #471).
    pub id: ClientId,
    /// Shared session infrastructure (compositor, home mode).
    ///
    /// This field contains truly shared state that is accessed by all clients.
    /// `SessionRuntime` accesses shared state via `session.shared`.
    pub shared: SessionShared,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl std::fmt::Debug for Session {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Session")
            .field("id", &self.id)
            .field("shared", &self.shared)
            .finish()
    }
}

impl Session {
    /// Create a new session with shared infrastructure.
    ///
    /// # Parameters
    ///
    /// - `id`: Unique client identifier
    /// - `home_mode`: The mode used to initialize new clients' mode stacks
    ///
    /// # Architecture (#488, #491)
    ///
    /// This creates a session with shared state (`SessionShared`) containing
    /// the `home_mode` for initializing new clients.
    ///
    /// The deprecated fields (`mode_stack`, `windows`, `extensions`) are
    /// initialized with placeholder values and should NOT be used at runtime.
    ///
    /// For per-client state, use [`BootstrapState`] or create `EditingState` directly.
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// // Create session with home mode
    /// let session = Session::new(ClientId::new(1), home_mode.clone());
    ///
    /// // Get home_mode for initializing new clients
    /// let mode_for_new_client = session.shared.home_mode().clone();
    ///
    /// // Create per-client state separately
    /// let mode_stack = ModeStack::new(mode_for_new_client);
    /// let windows = WindowLayout::empty();
    /// let extensions = ExtensionMap::new();
    /// ```
    #[must_use]
    pub fn new(id: ClientId, home_mode: ModeId) -> Self {
        Self {
            id,
            shared: SessionShared::new(home_mode),
        }
    }

    /// Create session with bootstrap state for a single client.
    ///
    /// This is a convenience method for tests and initialization that need
    /// both shared session state and initial per-client state.
    ///
    /// # Returns
    ///
    /// Tuple of `(Session, BootstrapState)` where:
    /// - `Session` contains shared infrastructure (`SessionShared`) with `home_mode`
    /// - `BootstrapState` contains initial per-client data that should be
    ///   stored in `EditingState` when adding a client
    ///
    /// # Example
    ///
    /// ```rust,ignore
    /// let (mut session, bootstrap) = Session::bootstrap(ClientId::new(1), home_mode);
    ///
    /// // Use bootstrap for per-client state
    /// let mut mode_stack = bootstrap.mode_stack;
    /// let mut windows = bootstrap.windows;
    /// let mut extensions = bootstrap.extensions;
    ///
    /// // Create runtime with per-client state
    /// let runtime = SessionRuntime::new(
    ///     &mut session, &mut mode_stack, &mut windows, &mut extensions,
    ///     &kernel, &executor,
    /// );
    /// ```
    #[must_use]
    pub fn bootstrap(id: ClientId, home_mode: ModeId) -> (Self, BootstrapState) {
        (Self::new(id, home_mode.clone()), BootstrapState::new(home_mode))
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
}

/// Per-client cursor position snapshot for bridge tick consumption.
///
/// Updated by the runner after each key event. Bridges read this in
/// `tick()` to detect cursor movement without direct access to the
/// window layout.
///
/// This is a mechanism-level type: any bridge can read it, the runner
/// writes it. No module-specific coupling.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CursorSnapshot {
    /// Cursor line (0-indexed).
    pub line: u32,
    /// Cursor column (0-indexed).
    pub col: u32,
    /// Raw buffer ID for the active buffer.
    pub buffer_id: u64,
}

impl CursorSnapshot {
    /// Create a new cursor snapshot.
    #[must_use]
    pub const fn new(line: u32, col: u32, buffer_id: u64) -> Self {
        Self {
            line,
            col,
            buffer_id,
        }
    }
}

impl crate::SessionExtension for CursorSnapshot {
    fn create() -> Self {
        Self {
            line: u32::MAX,
            col: u32::MAX,
            buffer_id: 0,
        }
    }
}

#[cfg(test)]
#[path = "types_tests.rs"]
mod tests;
