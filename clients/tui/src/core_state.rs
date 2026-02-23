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
//! │    - Mode: mode_name, mode_display, is_insert_mode          │
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

/// Shared TUI state tracked from server notifications.
///
/// This struct contains all state needed by both interactive and headless
/// TUI modes for proper multi-client awareness and rendering.
#[derive(Debug, Default)]
#[allow(clippy::struct_excessive_bools)]
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

    /// Whether mode accepts text input.
    pub is_insert_mode: bool,

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

    /// Whether client needs to create a default window (empty server layout).
    pub needs_default_window: bool,

    // =========================================================================
    // Content
    // =========================================================================
    /// Buffer content cache (`buffer_id` -> lines).
    pub buffer_cache: HashMap<u64, Vec<String>>,
    // TODO(#494): Add per-window scroll_top: HashMap<u64, usize> for scroll tracking

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

    /// Whether screen needs redraw.
    pub needs_redraw: bool,

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
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_core_state_default() {
        let state = TuiCoreState::default();
        assert_eq!(state.my_client_id, 0);
        assert!(state.mode_name.is_empty());
        assert!(state.other_clients.is_empty());
        assert_eq!(state.line_number_mode, LineNumberMode::None);
    }

    #[test]
    fn test_core_state_new_with_client_id() {
        let state = TuiCoreState::new(42);
        assert_eq!(state.my_client_id, 42);
    }

    #[test]
    fn test_core_state_new_with_size() {
        let state = TuiCoreState::new_with_size(1, 80, 24);
        assert_eq!(state.my_client_id, 1);
        assert_eq!(state.width, 80);
        assert_eq!(state.height, 24);
    }

    #[test]
    fn test_update_local_cursor() {
        let mut state = TuiCoreState::new(1);
        state.focused_window_id = 10;
        state.update_local_cursor(10, 5, 3);

        // Verify cursor stored in window_cursors
        assert!(state.window_cursors.contains_key(&10));

        // Verify get_focused_cursor() returns correct position
        let cursor = state.get_focused_cursor().expect("cursor should exist");
        assert_eq!(cursor.line, 5);
        assert_eq!(cursor.column, 3);
    }

    #[test]
    fn test_add_remove_remote_client() {
        let mut state = TuiCoreState::new(1);

        let remote = RemoteClient {
            client_id: 2,
            display_name: "Test".to_string(),
            cursor_line: 0,
            cursor_col: 0,
            buffer_id: Some(1),
            mode: "NORMAL".to_string(),
            selection: None,
        };

        state.add_remote_client(remote);
        assert!(state.other_clients.contains_key(&2));

        state.remove_remote_client(2);
        assert!(!state.other_clients.contains_key(&2));
    }

    #[test]
    fn test_add_remote_client_skips_self() {
        let mut state = TuiCoreState::new(1);

        let remote = RemoteClient {
            client_id: 1, // Same as my_client_id
            display_name: "Self".to_string(),
            cursor_line: 0,
            cursor_col: 0,
            buffer_id: Some(1),
            mode: "NORMAL".to_string(),
            selection: None,
        };

        state.add_remote_client(remote);
        assert!(!state.other_clients.contains_key(&1)); // Should not be added
    }

    #[test]
    fn test_client_role_display() {
        assert_eq!(ClientRole::Owner.as_str(), "Owner");
        assert_eq!(ClientRole::Follow.as_str(), "Follow");
        assert_eq!(ClientRole::Share.as_str(), "Share");
    }

    #[test]
    fn test_line_number_mode_default() {
        assert_eq!(LineNumberMode::default(), LineNumberMode::None);
    }

    #[test]
    fn test_update_remote_cursor() {
        let mut state = TuiCoreState::new(1);

        let remote = RemoteClient {
            client_id: 2,
            display_name: "Peer".to_string(),
            cursor_line: 0,
            cursor_col: 0,
            buffer_id: Some(1),
            mode: "NORMAL".to_string(),
            selection: None,
        };
        state.add_remote_client(remote);

        state.update_remote_cursor(2, 10, 5);

        let remote = state.other_clients.get(&2).unwrap();
        assert_eq!(remote.cursor_line, 10);
        assert_eq!(remote.cursor_col, 5);
    }

    #[test]
    fn test_update_remote_cursor_unknown_client() {
        let mut state = TuiCoreState::new(1);
        // Update for a non-existent client should be a no-op (no panic)
        state.update_remote_cursor(999, 10, 5);
    }

    #[test]
    fn test_update_local_selection() {
        let mut state = TuiCoreState::new(1);
        state.focused_window_id = 10;

        let sel = SelectionState {
            start: CursorPosition { line: 1, column: 0 },
            end: CursorPosition { line: 3, column: 5 },
            mode: "char".to_string(),
        };

        state.update_local_selection(10, Some(sel));
        assert!(state.window_selections.contains_key(&10));

        // Clear selection
        state.update_local_selection(10, None);
        assert!(!state.window_selections.contains_key(&10));
    }

    #[test]
    fn test_update_remote_selection() {
        let mut state = TuiCoreState::new(1);

        let remote = RemoteClient {
            client_id: 2,
            display_name: "Peer".to_string(),
            cursor_line: 0,
            cursor_col: 0,
            buffer_id: Some(1),
            mode: "NORMAL".to_string(),
            selection: None,
        };
        state.add_remote_client(remote);

        let sel = SelectionState {
            start: CursorPosition { line: 0, column: 0 },
            end: CursorPosition {
                line: 2,
                column: 10,
            },
            mode: "line".to_string(),
        };
        state.update_remote_selection(2, Some(sel));

        assert!(state.other_clients.get(&2).unwrap().selection.is_some());

        // Clear remote selection
        state.update_remote_selection(2, None);
        assert!(state.other_clients.get(&2).unwrap().selection.is_none());
    }

    #[test]
    fn test_update_remote_selection_unknown_client() {
        let mut state = TuiCoreState::new(1);
        // Should be a no-op for unknown client
        state.update_remote_selection(999, Some(SelectionState::default()));
    }

    #[test]
    fn test_cleanup_stale_cursors() {
        let mut state = TuiCoreState::new(1);

        // Set up cursors for windows 10 and 20
        state.update_local_cursor(10, 5, 3);
        state.update_local_cursor(20, 8, 1);
        state.update_local_selection(
            10,
            Some(SelectionState {
                start: CursorPosition { line: 0, column: 0 },
                end: CursorPosition { line: 1, column: 5 },
                mode: "char".to_string(),
            }),
        );
        state.update_local_selection(
            20,
            Some(SelectionState {
                start: CursorPosition { line: 2, column: 0 },
                end: CursorPosition { line: 3, column: 5 },
                mode: "line".to_string(),
            }),
        );

        // Only window 10 survives the layout change
        state.windows = vec![WindowInfo {
            window_id: 10,
            buffer_id: Some(1),
            rect: None,
            focused: true,
        }];

        state.cleanup_stale_cursors();

        assert!(state.window_cursors.contains_key(&10));
        assert!(!state.window_cursors.contains_key(&20));
        assert!(state.window_selections.contains_key(&10));
        assert!(!state.window_selections.contains_key(&20));
    }

    #[test]
    fn test_get_focused_cursor_none() {
        let state = TuiCoreState::new(1);
        assert!(state.get_focused_cursor().is_none());
    }

    #[test]
    fn test_get_focused_cursor_wrong_window() {
        let mut state = TuiCoreState::new(1);
        state.focused_window_id = 10;
        state.update_local_cursor(20, 5, 3); // Different window

        assert!(state.get_focused_cursor().is_none());
    }

    #[test]
    fn test_cursor_position_default() {
        let pos = CursorPosition::default();
        assert_eq!(pos.line, 0);
        assert_eq!(pos.column, 0);
    }

    #[test]
    fn test_selection_state_default() {
        let sel = SelectionState::default();
        assert_eq!(sel.start.line, 0);
        assert_eq!(sel.end.line, 0);
        assert!(sel.mode.is_empty());
    }

    #[test]
    fn test_client_role_default() {
        assert_eq!(ClientRole::default(), ClientRole::Owner);
    }

    #[test]
    fn test_remote_client_debug() {
        let remote = RemoteClient {
            client_id: 2,
            display_name: "Test".to_string(),
            cursor_line: 5,
            cursor_col: 10,
            buffer_id: Some(1),
            mode: "NORMAL".to_string(),
            selection: None,
        };
        let debug = format!("{remote:?}");
        assert!(debug.contains("RemoteClient"));
    }

    #[test]
    fn test_line_number_mode_variants() {
        // Test all variants for equality
        assert_ne!(LineNumberMode::None, LineNumberMode::Absolute);
        assert_ne!(LineNumberMode::Absolute, LineNumberMode::Relative);
        assert_ne!(LineNumberMode::Relative, LineNumberMode::Hybrid);
    }

    #[test]
    fn test_core_state_needs_redraw() {
        let mut state = TuiCoreState::new(1);
        assert!(!state.needs_redraw);
        state.needs_redraw = true;
        assert!(state.needs_redraw);
    }

    #[test]
    fn test_core_state_last_error() {
        let mut state = TuiCoreState::new(1);
        assert!(state.last_error.is_none());
        state.last_error = Some("test error".to_string());
        assert_eq!(state.last_error.as_deref(), Some("test error"));
    }

    #[test]
    fn test_core_state_buffer_cache() {
        let mut state = TuiCoreState::new(1);
        state
            .buffer_cache
            .insert(100, vec!["line 1".to_string(), "line 2".to_string()]);
        assert_eq!(state.buffer_cache.get(&100).unwrap().len(), 2);
        assert!(!state.buffer_cache.contains_key(&999));
    }
}
