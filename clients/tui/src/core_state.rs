//! Shared TUI core state for interactive and headless modes.
//!
//! This module provides `TuiCoreState`, the unified state type that
//! `TuiApp<O: TuiOutput>` uses for multi-client awareness and proper
//! cursor/selection tracking.
//!
//! # Architecture (Issue #493)
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │  TuiCoreState (this module)                                 │
//! │    - Identity: my_client_id, my_role                        │
//! │    - Mode: mode_name, mode_display, flags (insert mode)      │
//! │    - Cursor: global + per-window (window_cursors)           │
//! │    - Selection: per-window (window_selections)              │
//! │    - Layout: focused_window_id, windows                     │
//! │    - Content: buffer_cache                                  │
//! │    - Multi-client: other_clients (remote awareness)         │
//! └─────────────────────────────────────────────────────────────┘
//! ```
//!
//! Both interactive and headless TUI embed this struct to share
//! notification handling and rendering logic.

use std::collections::HashMap;

use reovim_protocol::v2::WindowInfo;

/// Line number display mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LineNumberMode {
    /// No line numbers.
    #[default]
    None,
    /// Absolute line numbers (1, 2, 3...).
    Absolute,
    /// Relative line numbers (distance from cursor).
    Relative,
    /// Hybrid: absolute for cursor line, relative for others.
    Hybrid,
}

/// Client role in a multi-client session.
///
/// Determines how this client's input is routed:
/// - `Owner`: Has own independent state (cursor, mode, etc.)
/// - `Follow`: Read-only spectator of another client
/// - `Share`: Bidirectional editing with another client's state
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum ClientRole {
    /// Owns editing state (default for new clients).
    #[default]
    Owner,
    /// Read-only spectator of target client.
    #[allow(dead_code)]
    Follow,
    /// Shares state with owner (pair programming).
    #[allow(dead_code)]
    Share,
}

impl ClientRole {
    /// Returns the display string for the statusline.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "Owner",
            Self::Follow => "Follow",
            Self::Share => "Share",
        }
    }
}

/// Cursor position for per-window tracking.
///
/// Uses u64 to match protobuf `CursorMovedPayload` types.
#[derive(Debug, Clone, Copy, Default)]
pub struct CursorPosition {
    /// Line number (0-indexed).
    pub line: u64,
    /// Column number (0-indexed).
    pub column: u64,
}

/// Selection state for per-window tracking.
///
/// Tracks the visual selection range for highlighting in the TUI.
#[derive(Debug, Clone, Default)]
pub struct SelectionState {
    /// Start position of selection.
    pub start: CursorPosition,
    /// End position of selection (exclusive).
    pub end: CursorPosition,
    /// Visual mode type (char, line, block).
    pub mode: String,
}

/// Presence information for a remote client.
///
/// Tracks other clients' cursor positions for awareness rendering.
#[derive(Debug, Clone)]
pub struct RemoteClient {
    /// Client's unique ID.
    pub client_id: u64,
    /// User-friendly display name (e.g., "TUI@laptop").
    pub display_name: String,
    /// Cursor line (0-indexed).
    pub cursor_line: u64,
    /// Cursor column (0-indexed).
    pub cursor_col: u64,
    /// Buffer ID the client is viewing (None if no buffer assigned).
    pub buffer_id: Option<u64>,
    /// Current mode name.
    pub mode: String,
    /// Selection state for visual mode.
    pub selection: Option<SelectionState>,
}

/// Boolean state flags packed into a `u8` bitfield.
///
/// Replaces individual `bool` fields on `TuiCoreState` to satisfy
/// `clippy::struct_excessive_bools` without external dependencies.
#[derive(Debug, Clone, Copy, Default)]
struct TuiStateFlags(u8);

impl TuiStateFlags {
    const INSERT_MODE: u8 = 1 << 0;
    const NEEDS_DEFAULT_WINDOW: u8 = 1 << 1;
    const NEEDS_REDRAW: u8 = 1 << 2;

    const fn get(self, bit: u8) -> bool {
        self.0 & bit != 0
    }

    const fn set(&mut self, bit: u8, value: bool) {
        if value {
            self.0 |= bit;
        } else {
            self.0 &= !bit;
        }
    }
}

/// Shared TUI state tracked from server notifications.
///
/// This struct contains all state needed by both interactive and headless
/// TUI modes for proper multi-client awareness and rendering.
#[derive(Debug, Default)]
pub struct TuiCoreState {
    // =========================================================================
    // Identity
    // =========================================================================
    /// This client's unique ID.
    ///
    /// Assigned by `presence_join()` on connect. All `SendKeys` requests
    /// must include this ID for per-client state isolation.
    pub my_client_id: u64,

    /// This client's role in the session.
    pub my_role: ClientRole,

    // =========================================================================
    // Mode
    // =========================================================================
    /// Current mode name (internal).
    pub mode_name: String,

    /// Current mode display string.
    pub mode_display: String,

    /// Boolean state flags (`is_insert_mode`, `needs_default_window`, `needs_redraw`).
    flags: TuiStateFlags,

    // =========================================================================
    // Cursor (per-window only)
    // =========================================================================
    /// Per-window cursor positions.
    ///
    /// Maps `window_id` -> cursor position. Updated from `CursorMoved`
    /// notifications that include `window_id`.
    pub window_cursors: HashMap<u64, CursorPosition>,

    /// Per-window selection state.
    ///
    /// Maps `window_id` -> selection. Updated from `SelectionChanged`
    /// notifications. Used to render visual selection highlighting.
    pub window_selections: HashMap<u64, SelectionState>,

    // =========================================================================
    // Layout
    // =========================================================================
    /// Focused window ID.
    pub focused_window_id: u64,

    /// Window layout info.
    pub windows: Vec<WindowInfo>,

    /// Active tab page ID (#401).
    pub active_tab_id: Option<u64>,

    /// Tab page info (#401).
    pub tabs: Vec<reovim_protocol::v2::TabPageInfo>,

    // =========================================================================
    // Content
    // =========================================================================
    /// Buffer content cache (`buffer_id` -> lines).
    pub buffer_cache: HashMap<u64, Vec<String>>,

    /// Per-window scroll offsets (vertical).
    ///
    /// Maps `window_id` -> `scroll_top` (first visible line, 0-indexed).
    /// Computed before each render frame from cursor position and viewport height.
    pub scroll_tops: HashMap<u64, usize>,

    // =========================================================================
    // Multi-client awareness (#474)
    // =========================================================================
    /// Other connected clients for awareness rendering.
    ///
    /// Maps `client_id` -> `RemoteClient`. Used to render other clients'
    /// cursors and selections.
    pub other_clients: HashMap<u64, RemoteClient>,

    // =========================================================================
    // Display
    // =========================================================================
    /// Line number display mode.
    pub line_number_mode: LineNumberMode,

    /// Last error message for statusline.
    pub last_error: Option<String>,

    // =========================================================================
    // Viewport (headless-specific, but kept for unification)
    // =========================================================================
    /// Viewport width.
    pub width: u16,

    /// Viewport height.
    pub height: u16,
}

impl TuiCoreState {
    // =========================================================================
    // Flag accessors (delegating to TuiStateFlags bitfield)
    // =========================================================================

    /// Whether mode accepts text input.
    #[must_use]
    pub const fn is_insert_mode(&self) -> bool {
        self.flags.get(TuiStateFlags::INSERT_MODE)
    }

    /// Set whether mode accepts text input.
    pub const fn set_insert_mode(&mut self, value: bool) {
        self.flags.set(TuiStateFlags::INSERT_MODE, value);
    }

    /// Whether client needs to create a default window (empty server layout).
    #[must_use]
    pub const fn needs_default_window(&self) -> bool {
        self.flags.get(TuiStateFlags::NEEDS_DEFAULT_WINDOW)
    }

    /// Set whether client needs to create a default window.
    pub const fn set_needs_default_window(&mut self, value: bool) {
        self.flags.set(TuiStateFlags::NEEDS_DEFAULT_WINDOW, value);
    }

    /// Whether screen needs redraw.
    #[must_use]
    pub const fn needs_redraw(&self) -> bool {
        self.flags.get(TuiStateFlags::NEEDS_REDRAW)
    }

    /// Set whether screen needs redraw.
    pub const fn set_needs_redraw(&mut self, value: bool) {
        self.flags.set(TuiStateFlags::NEEDS_REDRAW, value);
    }

    // =========================================================================
    // Constructors
    // =========================================================================

    /// Create a new core state with the given client ID.
    #[must_use]
    pub fn new(client_id: u64) -> Self {
        Self {
            my_client_id: client_id,
            ..Self::default()
        }
    }

    /// Create a new core state with client ID and viewport size.
    #[must_use]
    pub fn new_with_size(client_id: u64, width: u16, height: u16) -> Self {
        Self {
            my_client_id: client_id,
            width,
            height,
            ..Self::default()
        }
    }

    /// Update cursor position for local client.
    ///
    /// Stores cursor position per-window in `window_cursors`.
    /// Use `get_focused_cursor()` to retrieve the cursor for the focused window.
    pub fn update_local_cursor(&mut self, window_id: u64, line: u64, column: u64) {
        self.window_cursors
            .insert(window_id, CursorPosition { line, column });
    }

    /// Update cursor position for a remote client.
    pub fn update_remote_cursor(&mut self, client_id: u64, line: u64, col: u64) {
        if let Some(remote) = self.other_clients.get_mut(&client_id) {
            remote.cursor_line = line;
            remote.cursor_col = col;
        }
    }

    /// Update selection for local client.
    pub fn update_local_selection(&mut self, window_id: u64, selection: Option<SelectionState>) {
        match selection {
            Some(sel) => {
                self.window_selections.insert(window_id, sel);
            }
            None => {
                self.window_selections.remove(&window_id);
            }
        }
    }

    /// Update selection for a remote client.
    pub fn update_remote_selection(&mut self, client_id: u64, selection: Option<SelectionState>) {
        if let Some(remote) = self.other_clients.get_mut(&client_id) {
            remote.selection = selection;
        }
    }

    /// Add a remote client from presence notification.
    pub fn add_remote_client(&mut self, client: RemoteClient) {
        // Don't track ourselves
        if client.client_id != self.my_client_id {
            self.other_clients.insert(client.client_id, client);
        }
    }

    /// Remove a remote client when they leave.
    pub fn remove_remote_client(&mut self, client_id: u64) {
        self.other_clients.remove(&client_id);
    }

    /// Clean up stale window cursors after layout change.
    pub fn cleanup_stale_cursors(&mut self) {
        let current_window_ids: std::collections::HashSet<u64> =
            self.windows.iter().map(|w| w.window_id).collect();
        self.window_cursors
            .retain(|id, _| current_window_ids.contains(id));
        self.window_selections
            .retain(|id, _| current_window_ids.contains(id));
        self.scroll_tops
            .retain(|id, _| current_window_ids.contains(id));
    }

    /// Get cursor position for the focused window.
    ///
    /// This is the single source of truth for cursor position, used by both
    /// the statusline and cursor positioning. Returns `None` if no cursor
    /// is tracked for the focused window (e.g., before the first `CursorMoved`
    /// notification arrives after a layout change).
    #[must_use]
    pub fn get_focused_cursor(&self) -> Option<CursorPosition> {
        self.window_cursors.get(&self.focused_window_id).copied()
    }

    // =========================================================================
    // Scroll tracking (#494)
    // =========================================================================

    /// Compute and store `scroll_top` for a window to keep its cursor visible.
    ///
    /// Implements "ensure cursor in viewport" logic:
    /// - If cursor is above viewport: scroll up to cursor line
    /// - If cursor is below viewport: scroll down so cursor is on last line
    /// - Otherwise: keep current scroll position
    ///
    /// Returns the computed `scroll_top`.
    #[allow(clippy::cast_possible_truncation)]
    pub fn compute_scroll_top(&mut self, window_id: u64, content_height: u16) -> usize {
        let cursor_line = self
            .window_cursors
            .get(&window_id)
            .map_or(0, |c| c.line as usize);

        let current = self.scroll_tops.get(&window_id).copied().unwrap_or(0);
        let height = content_height as usize;

        let new_scroll_top = if height == 0 {
            0
        } else if cursor_line < current {
            cursor_line
        } else if cursor_line >= current + height {
            cursor_line - height + 1
        } else {
            current
        };

        self.scroll_tops.insert(window_id, new_scroll_top);
        new_scroll_top
    }

    /// Get the stored `scroll_top` for a window.
    ///
    /// Returns 0 if no `scroll_top` has been computed for this window.
    #[must_use]
    pub fn get_scroll_top(&self, window_id: u64) -> usize {
        self.scroll_tops.get(&window_id).copied().unwrap_or(0)
    }

    /// Get `scroll_top` for the focused window.
    #[must_use]
    pub fn get_focused_scroll_top(&self) -> usize {
        self.get_scroll_top(self.focused_window_id)
    }

    /// Get the buffer ID for the focused window.
    ///
    /// Searches `windows` for the entry matching `focused_window_id`.
    /// Returns `None` if the window list is empty, no match is found,
    /// or the matching window has no buffer assigned.
    #[must_use]
    pub fn get_focused_buffer_id(&self) -> Option<u64> {
        self.windows
            .iter()
            .find(|w| w.window_id == self.focused_window_id)
            .and_then(|w| w.buffer_id)
    }
}

#[cfg(test)]
#[path = "core_state_tests.rs"]
mod tests;
