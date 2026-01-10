#![allow(clippy::missing_errors_doc)]

pub mod border;
mod render;

/// Z-order constants for core rendering layers
///
/// Higher values render on top (occlude lower values).
/// Plugins define their own z-orders via `OverlayRenderer::z_order()`.
pub mod z_order {
    /// Base layer: tab line, status line (always visible)
    pub const BASE: u8 = 0;
    /// Editor windows (text content)
    pub const EDITOR: u8 = 2;
}

/// Bounds of a layer (x, y, width, height)
#[derive(Debug, Clone, Copy, Default)]
pub struct LayerBounds {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl LayerBounds {
    /// Create new bounds
    #[must_use]
    pub const fn new(x: u16, y: u16, width: u16, height: u16) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Create full-screen bounds
    #[must_use]
    pub const fn full_screen(width: u16, height: u16) -> Self {
        Self {
            x: 0,
            y: 0,
            width,
            height,
        }
    }

    /// Check if this bounds contains a point
    #[must_use]
    pub const fn contains(&self, px: u16, py: u16) -> bool {
        px >= self.x && px < self.x + self.width && py >= self.y && py < self.y + self.height
    }
}

use {
    crate::{
        buffer::Buffer,
        command::terminal::{Clear, ClearType},
        component::RenderState,
        frame::{FrameBuffer, FrameRenderer},
        highlight::Theme,
        modd::ComponentId,
    },
    reovim_sys::{
        event::{DisableMouseCapture, EnableMouseCapture},
        queue,
        terminal::size,
    },
    std::{
        collections::BTreeMap,
        io::{self, Write},
    },
    window::{SignColumnMode, Window},
};

pub mod cusor;
pub mod layout;
pub mod window;

pub use {
    border::{BorderConfig, BorderMode, BorderStyle, WindowAdjacency},
    layout::{
        LayoutManager, WindowType, is_plugin_window_id, plugin_window_id,
        split::{
            NavigateDirection, SplitDirection, SplitNode, WindowLayout, WindowRect,
            compute_adjacency, compute_all_adjacencies,
        },
        tab::{TabInfo, TabManager, TabPage},
    },
    window::{
        WindowId,
        store::{CloseResult, WindowStore},
    },
};

pub struct ScreenSize {
    pub height: u16,
    pub width: u16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Position {
    pub x: u16,
    pub y: u16,
}

/// Information about a viewport scroll event
#[derive(Debug, Clone, Copy)]
pub struct ViewportScrollInfo {
    /// Window that scrolled
    pub window_id: usize,
    /// Buffer being viewed
    pub buffer_id: usize,
    /// First visible line (0-indexed)
    pub top_line: u32,
    /// Last visible line (0-indexed)
    pub bottom_line: u32,
}

pub struct Screen {
    size: ScreenSize,
    out_stream: Box<dyn Write>,
    /// Centralized window store - single source of truth for windows, tabs, and buffer mappings
    window_store: WindowStore,
    layout: LayoutManager,
    /// Frame renderer for buffered rendering (always Some after initialization)
    frame_renderer: Option<FrameRenderer>,
    /// Pending viewport scroll events to be emitted by the runtime
    pending_viewport_scrolls: Vec<ViewportScrollInfo>,
}

impl Default for Screen {
    fn default() -> Self {
        let stdout = io::stdout();
        let (columns, rows) = size().expect("failed to get screen size on screen creation");
        let editor_height = rows.saturating_sub(1); // Reserve last row for status line

        // Create window store with initial window
        let window_store = WindowStore::new(0, columns, editor_height);
        let layout = LayoutManager::new(columns, editor_height);

        Self {
            size: ScreenSize {
                width: columns,
                height: rows,
            },
            out_stream: Box::new(stdout),
            window_store,
            layout,
            frame_renderer: Some(FrameRenderer::new(columns, rows)),
            pending_viewport_scrolls: Vec::new(),
        }
    }
}

impl Screen {
    /// Create a new Screen with a custom writer (useful for testing/benchmarking)
    #[must_use]
    pub fn with_writer<W: Write + 'static>(writer: W, width: u16, height: u16) -> Self {
        let editor_height = height.saturating_sub(1);

        // Create window store with initial window
        let window_store = WindowStore::new(0, width, editor_height);
        let layout = LayoutManager::new(width, editor_height);

        Self {
            size: ScreenSize { height, width },
            out_stream: Box::new(writer),
            window_store,
            layout,
            frame_renderer: Some(FrameRenderer::new(width, height)),
            pending_viewport_scrolls: Vec::new(),
        }
    }

    pub fn clear(&mut self, ctype: ClearType) -> std::result::Result<(), std::io::Error> {
        queue!(self.out_stream, Clear(ctype))
    }

    pub fn flush(&mut self) -> std::result::Result<(), std::io::Error> {
        self.out_stream.flush()
    }

    pub fn enable_mouse_capture(&mut self) -> std::result::Result<(), std::io::Error> {
        queue!(self.out_stream, EnableMouseCapture)
    }

    pub fn disable_mouse_capture(&mut self) -> std::result::Result<(), std::io::Error> {
        queue!(self.out_stream, DisableMouseCapture)
    }

    pub fn initialize(&mut self) -> std::result::Result<(), std::io::Error> {
        self.enable_mouse_capture()?;
        self.clear(ClearType::All)?;
        self.out_stream.flush()
    }

    pub fn finalize(&mut self) -> std::result::Result<(), std::io::Error> {
        self.disable_mouse_capture()?;
        self.clear(ClearType::All)?;
        self.out_stream.flush()
    }

    #[must_use]
    pub const fn width(&self) -> u16 {
        self.size.width
    }

    #[must_use]
    pub const fn height(&self) -> u16 {
        self.size.height
    }

    /// Get screen dimensions as (width, height) tuple
    #[must_use]
    pub const fn size(&self) -> (u16, u16) {
        (self.size.width, self.size.height)
    }

    /// Update screen dimensions on terminal resize
    pub fn resize(&mut self, width: u16, height: u16) {
        let editor_height = height.saturating_sub(1); // Reserve status line
        self.size = ScreenSize { height, width };
        self.layout.set_screen_size(width, editor_height);
        self.update_window_layouts();

        // Debug: log the updated window positions
        tracing::debug!(screen_width = width, screen_height = height, "Screen resized");
        for win in self.window_store.iter() {
            tracing::debug!(
                window_id = win.id.raw(),
                x = win.bounds.x,
                y = win.bounds.y,
                win_width = win.bounds.width,
                win_height = win.bounds.height,
                "Window layout after resize"
            );
        }

        // Resize frame renderer
        if let Some(ref mut renderer) = self.frame_renderer {
            renderer.resize(width, height);
        }
    }

    /// Find the window at a given screen position
    ///
    /// Returns the window ID if a window exists at the position, or None if the
    /// position is outside all windows (e.g., on the status line).
    ///
    /// When multiple windows overlap (floating windows), returns the topmost
    /// window (highest z-order).
    #[must_use]
    pub fn window_at_position(&self, x: u16, y: u16) -> Option<usize> {
        self.window_store
            .window_at_position(x, y)
            .map(|id| id.raw())
    }

    /// Get a reference to a window by ID
    #[must_use]
    pub fn window(&self, window_id: usize) -> Option<&Window> {
        self.window_store.get(WindowId::new(window_id))
    }

    /// Get a mutable reference to a window by ID
    pub fn window_mut(&mut self, window_id: usize) -> Option<&mut Window> {
        self.window_store.get_mut(WindowId::new(window_id))
    }

    /// Set the active window by ID
    ///
    /// Returns true if the window was found and activated, false otherwise.
    pub fn set_active_window(&mut self, window_id: usize) -> bool {
        self.window_store
            .set_active_window(WindowId::new(window_id))
    }

    /// Enable frame buffer capture and return a handle for external readers
    ///
    /// This enables capture on the frame renderer and returns a handle that
    /// provides thread-safe access to the latest complete frame.
    /// Used by RPC server for `CellGrid` format.
    pub fn enable_frame_capture(&mut self) -> Option<crate::frame::FrameBufferHandle> {
        self.frame_renderer
            .as_mut()
            .map(FrameRenderer::enable_capture)
    }

    /// Get the frame buffer capture handle (if capture is enabled)
    #[must_use]
    pub fn frame_capture_handle(&self) -> Option<crate::frame::FrameBufferHandle> {
        self.frame_renderer
            .as_ref()
            .and_then(FrameRenderer::capture_handle)
    }

    /// Get a read-only reference to the frame buffer (if enabled)
    #[must_use]
    pub fn frame_buffer(&self) -> Option<&FrameBuffer> {
        self.frame_renderer.as_ref().map(FrameRenderer::buffer)
    }

    /// Get the screen as ASCII art (for debugging)
    ///
    /// Returns None if frame renderer is not enabled.
    #[must_use]
    pub fn to_ascii(&self) -> Option<String> {
        self.frame_buffer().map(FrameBuffer::to_ascii)
    }

    /// Get annotated ASCII art with cursor position
    ///
    /// Returns None if frame renderer is not enabled.
    #[must_use]
    pub fn to_annotated_ascii(&self, cursor: Option<(u16, u16)>) -> Option<String> {
        self.frame_buffer().map(|buf| {
            let mut config = crate::visual::AsciiRenderConfig::new().annotated();
            if let Some((x, y)) = cursor {
                config = config.with_cursor(x, y);
            }
            buf.to_annotated_ascii(&config)
        })
    }

    /// Take pending viewport scroll events
    ///
    /// Returns scroll events that occurred during the last render and clears the pending list.
    /// The runtime should call this after rendering to emit `ViewportScrolled` events.
    pub fn take_viewport_scrolls(&mut self) -> Vec<ViewportScrollInfo> {
        std::mem::take(&mut self.pending_viewport_scrolls)
    }

    /// Check and update viewport scroll positions
    ///
    /// Call this after cursor movement to detect if the viewport needs to scroll.
    /// Returns scroll events for any windows that scrolled.
    pub fn update_viewport_scrolls(
        &mut self,
        buffers: &std::collections::BTreeMap<usize, Buffer>,
    ) -> Vec<ViewportScrollInfo> {
        let mut scrolls = Vec::new();

        for win in self.window_store.iter_mut() {
            if let Some(buffer_id) = win.buffer_id()
                && let Some(buf) = buffers.get(&buffer_id)
            {
                let effective_cursor_y = if win.is_active {
                    buf.cur.y
                } else {
                    win.viewport.cursor.y
                };
                let scrolled = win.update_scroll(effective_cursor_y);
                if scrolled {
                    let (top_line, bottom_line) = win.viewport_bounds();
                    scrolls.push(ViewportScrollInfo {
                        window_id: win.id.raw(),
                        buffer_id,
                        top_line,
                        bottom_line,
                    });
                }
            }
        }

        scrolls
    }

    /// Get current viewport info for a buffer (for initial context requests)
    ///
    /// Returns viewport info for the window displaying the given buffer, if any.
    #[must_use]
    pub fn get_viewport_info(&self, buffer_id: usize) -> Option<ViewportScrollInfo> {
        self.window_store
            .iter()
            .find(|w| w.buffer_id() == Some(buffer_id))
            .map(|win| {
                let (top_line, bottom_line) = win.viewport_bounds();
                ViewportScrollInfo {
                    window_id: win.id.raw(),
                    buffer_id,
                    top_line,
                    bottom_line,
                }
            })
    }

    /// Render with bundled state
    ///
    /// This is the preferred rendering method that takes all runtime state as a single
    /// `RenderState` struct instead of many individual parameters.
    pub fn render_with_state(
        &mut self,
        state: &RenderState<'_>,
    ) -> std::result::Result<(), std::io::Error> {
        // Render using frame renderer
        self.render_windows(
            state.buffers,
            state.highlight_store,
            state.mode,
            state.command_line,
            state.pending_keys,
            state.last_command,
            state.color_mode,
            state.theme,
            state.visibility_source,
            state.indent_analyzer,
            state.modifier_registry,
            state.decoration_store,
            state.render_stages,
            state.plugin_state,
            state.display_registry,
        )
    }

    pub fn set_number(&mut self, enabled: bool) {
        for window in self.window_store.iter_mut() {
            window.set_number(enabled);
        }
    }

    pub fn set_relative_number(&mut self, enabled: bool) {
        for window in self.window_store.iter_mut() {
            window.set_relative_number(enabled);
        }
    }

    pub fn set_sign_column_mode(&mut self, mode: SignColumnMode) {
        for window in self.window_store.iter_mut() {
            window.config.sign_column_mode = mode;
        }
    }

    pub fn set_scrollbar(&mut self, enabled: bool) {
        for window in self.window_store.iter_mut() {
            window.set_scrollbar(enabled);
        }
    }

    /// Get a reference to the layout manager
    #[must_use]
    pub const fn layout(&self) -> &LayoutManager {
        &self.layout
    }

    /// Get a mutable reference to the layout manager
    pub const fn layout_mut(&mut self) -> &mut LayoutManager {
        &mut self.layout
    }

    // === Generic plugin focus API ===

    /// Focus a plugin window by component ID
    pub const fn focus_plugin(&mut self, id: ComponentId) {
        self.layout.focus_plugin(id);
    }

    /// Unfocus any plugin window, returning focus to editor
    pub const fn unfocus_plugin(&mut self) {
        self.layout.unfocus_plugin();
    }

    /// Check if a specific plugin window is focused
    #[must_use]
    pub fn is_plugin_focused(&self, id: ComponentId) -> bool {
        self.layout.is_plugin_focused(id)
    }

    /// Check if any plugin window is focused
    #[must_use]
    pub const fn has_plugin_focus(&self) -> bool {
        self.layout.has_plugin_focus()
    }

    /// Get the currently focused plugin component ID
    #[must_use]
    pub const fn focused_plugin(&self) -> Option<ComponentId> {
        self.layout.focused_plugin()
    }

    /// Focus the editor window (unfocus any plugin)
    pub const fn focus_editor(&mut self) {
        self.layout.focus_editor();
    }

    /// Set the buffer ID for the editor window (legacy - use `set_window_buffer` for splits)
    pub fn set_editor_buffer(&mut self, buffer_id: usize) {
        self.window_store.set_active_buffer(buffer_id);
    }

    /// Set the buffer ID for a specific window
    pub fn set_window_buffer(&mut self, window_id: usize, buffer_id: usize) {
        self.window_store
            .set_window_buffer(WindowId::new(window_id), buffer_id);
    }

    /// Get the buffer ID for the active window
    #[must_use]
    pub fn active_buffer_id(&self) -> Option<usize> {
        self.window_store.active_buffer_id()
    }

    /// Get the active window ID
    #[must_use]
    pub fn active_window_id(&self) -> Option<usize> {
        self.window_store.active_window_id().map(|id| id.raw())
    }

    /// Get a reference to the active window
    #[must_use]
    pub fn active_window(&self) -> Option<&Window> {
        self.window_store.active_window()
    }

    /// Get a mutable reference to the active window
    pub fn active_window_mut(&mut self) -> Option<&mut Window> {
        self.window_store.active_window_mut()
    }

    /// Save cursor from buffer to active window.
    ///
    /// Used before split to ensure new window inherits current cursor.
    /// Returns the active window ID if save was performed.
    pub fn save_cursor_to_active_window(
        &mut self,
        buffers: &BTreeMap<usize, Buffer>,
    ) -> Option<usize> {
        let active_id = self.active_window_id()?;
        let window = self.active_window_mut()?;
        let buffer_id = window.buffer_id()?;
        let buffer = buffers.get(&buffer_id)?;

        window.viewport.cursor = buffer.cur;
        window.viewport.desired_col = buffer.desired_col;

        tracing::debug!(
            "[CURSOR_SYNC] SAVE: win={} buffer.cur=({},{}) -> window.viewport.cursor",
            active_id,
            buffer.cur.x,
            buffer.cur.y,
        );

        Some(active_id)
    }

    /// Switch active window with full cursor synchronization.
    ///
    /// Use this for keyboard-driven window navigation (Ctrl-W h/j/k/l).
    /// For mouse clicks, use `set_active_window()` instead since the mouse
    /// directly sets cursor position.
    ///
    /// Performs the complete cursor handoff:
    /// 1. Saves current buffer cursor to old active window
    /// 2. Changes active window (updates `is_active` flags)
    /// 3. Loads new window's cursor into buffer
    ///
    /// Returns the previous active window ID, or `None` if same window (no-op).
    pub fn switch_active_window(
        &mut self,
        window_id: usize,
        buffers: &mut BTreeMap<usize, Buffer>,
    ) -> Option<usize> {
        self.window_store
            .switch_active_window(WindowId::new(window_id), buffers)
            .map(|id| id.raw())
    }

    /// Get the number of windows in the active tab
    #[must_use]
    pub fn window_count(&self) -> usize {
        self.window_store.window_count()
    }

    /// Get all windows as a Vec (for compatibility)
    ///
    /// Note: This clones windows. Prefer iterating with `window_store.iter()` when possible.
    #[must_use]
    pub fn windows(&self) -> Vec<Window> {
        self.window_store.windows_vec()
    }

    /// Collect all renderables as `Windows` (editor windows + overlays)
    ///
    /// Returns a vector of `Windows` sorted by z-order (lower values first).
    /// Overlays are converted to temporary `Window` structs with `WindowContentSource::Overlay`.
    ///
    /// TODO Phase 3: Integrate `overlay_registry` to collect overlay windows
    #[allow(dead_code)]
    fn collect_all_windows(&self) -> Vec<Window> {
        // Phase 2: Just return clones of editor windows
        // Phase 3 will add overlay collection here
        self.window_store.windows_vec()
    }

    /// Render all registered plugin windows
    ///
    /// Plugin windows are sorted by `z_order` and rendered on top of editor windows.
    /// Each plugin window implements the `PluginWindow` trait which provides
    /// both configuration (position, visibility) and rendering.
    #[allow(clippy::unused_self)]
    fn render_plugin_windows(
        &self,
        buffer: &mut FrameBuffer,
        plugin_state: &std::sync::Arc<crate::plugin::PluginStateRegistry>,
        ctx: &crate::plugin::EditorContext,
        theme: &Theme,
    ) {
        use crate::plugin::Rect;

        // Collect windows with their configs
        let mut windows_with_config: Vec<_> = plugin_state
            .plugin_windows()
            .into_iter()
            .filter_map(|window| {
                window
                    .window_config(plugin_state, ctx)
                    .filter(|config| config.visible)
                    .map(|config| (window, config))
            })
            .collect();

        // Sort by z_order (lower first, so they render below higher ones)
        windows_with_config.sort_by_key(|(_, config)| config.z_order);

        // Render each visible plugin window
        for (window, config) in windows_with_config {
            tracing::info!("==> Rendering plugin window at z_order={}", config.z_order);
            let bounds = Rect::new(
                config.bounds.x,
                config.bounds.y,
                config.bounds.width,
                config.bounds.height,
            );

            // Render the window content
            tracing::info!("==> Calling window.render()");
            window.render(plugin_state, ctx, buffer, bounds, theme);
            tracing::info!("==> Window rendered successfully");
        }
        tracing::info!("==> All plugin windows rendered");
    }

    /// Update window layouts based on current layout manager state and split tree
    fn update_window_layouts(&mut self) {
        let editor_layout = self.layout.editor_layout();
        let tab_offset = self.tab_line_height();

        let editor_rect = WindowRect::new(
            editor_layout.anchor.x,
            editor_layout.anchor.y,
            editor_layout.width,
            editor_layout.height,
        );

        self.window_store.update_layouts(editor_rect, tab_offset);
    }

    // === Window Split Operations ===

    /// Split the active window
    ///
    /// Returns the new window ID if successful
    pub fn split_window(&mut self, direction: SplitDirection) -> Option<usize> {
        let new_id = self.window_store.split(direction)?;
        self.update_window_layouts();
        Some(new_id.raw())
    }

    /// Close the active window
    ///
    /// Returns true if the last window in the last tab was closed (editor should quit)
    pub fn close_window(&mut self) -> bool {
        let result = self.window_store.close();
        self.update_window_layouts();
        result == CloseResult::ShouldQuit
    }

    /// Close all windows except the active one
    pub fn close_other_windows(&mut self) {
        self.window_store.close_others();
        self.update_window_layouts();
    }

    /// Update `is_active` flag on all windows based on active window ID
    pub fn update_window_active_state(&mut self) {
        // WindowStore handles this internally via update_active_state()
        // This method is kept for API compatibility
        if let Some(active_id) = self.window_store.active_window_id() {
            for window in self.window_store.iter_mut() {
                window.is_active = window.id == active_id;
            }
        }
    }

    /// Navigate focus to an adjacent editor window.
    ///
    /// Returns `Some(false)` if moved from plugin to editor window,
    /// `None` if navigation stayed within editor windows.
    /// Plugin windows handle their own focus via event bus.
    pub fn navigate_window(&mut self, direction: NavigateDirection) -> Option<bool> {
        // If a plugin has focus, unfocus it and focus the editor
        if self.layout.has_plugin_focus() {
            self.focus_editor();
            self.update_window_active_state();
            return Some(false);
        }

        // Calculate editor rect for navigation
        let editor_layout = self.layout.editor_layout();
        let editor_rect = WindowRect::new(
            editor_layout.anchor.x,
            editor_layout.anchor.y,
            editor_layout.width,
            editor_layout.height,
        );

        self.window_store.navigate(direction, editor_rect);
        None
    }

    /// Equalize window sizes
    pub fn equalize_windows(&mut self) {
        self.window_store.equalize();
        self.update_window_layouts();
    }

    /// Resize the active window in the specified direction
    ///
    /// - `Vertical` direction adjusts the width of vertical splits (left/right)
    /// - `Horizontal` direction adjusts the height of horizontal splits (top/bottom)
    /// - Positive delta increases size in that direction
    /// - Negative delta decreases size
    pub fn resize_window(&mut self, direction: SplitDirection, delta: f32) {
        self.window_store.resize(direction, delta);
        self.update_window_layouts();
    }

    /// Swap the active window with the window in the given direction
    ///
    /// Returns true if a swap occurred
    pub fn swap_window(&mut self, direction: NavigateDirection) -> bool {
        let editor_layout = self.layout.editor_layout();
        let editor_rect = WindowRect::new(
            editor_layout.anchor.x,
            editor_layout.anchor.y,
            editor_layout.width,
            editor_layout.height,
        );

        let swapped = self.window_store.swap(direction, editor_rect);
        if swapped {
            self.update_window_layouts();
        }
        swapped
    }

    // === Tab Operations ===

    /// Create a new tab
    ///
    /// Returns the new tab ID
    pub fn new_tab(&mut self, buffer_id: usize) -> usize {
        let editor_layout = self.layout.editor_layout();
        let tab_id =
            self.window_store
                .new_tab(buffer_id, editor_layout.width, editor_layout.height);
        self.update_window_layouts();
        tab_id
    }

    /// Close the current tab
    ///
    /// Returns true if the last tab was closed (editor should quit)
    pub fn close_tab(&mut self) -> bool {
        let has_more = self.window_store.close_tab();
        if has_more {
            self.update_window_layouts();
            false
        } else {
            true // Last tab - should quit
        }
    }

    /// Switch to next tab
    pub fn next_tab(&mut self) {
        self.window_store.next_tab();
        self.update_window_layouts();
    }

    /// Switch to previous tab
    pub fn prev_tab(&mut self) {
        self.window_store.prev_tab();
        self.update_window_layouts();
    }

    /// Go to a specific tab by index
    pub fn goto_tab(&mut self, index: usize) {
        self.window_store.goto_tab(index);
        self.update_window_layouts();
    }

    /// Get tab information for display
    #[must_use]
    pub fn tab_info(&self) -> Vec<TabInfo> {
        self.window_store.tab_info()
    }

    /// Get the number of tabs
    #[must_use]
    pub fn tab_count(&self) -> usize {
        self.window_store.tab_count()
    }

    /// Get a reference to the tab manager
    #[must_use]
    pub const fn tab_manager(&self) -> &TabManager {
        self.window_store.tab_manager()
    }

    /// Get the Y offset for the editor area (1 if tabs are shown, 0 otherwise)
    #[must_use]
    fn tab_line_height(&self) -> u16 {
        u16::from(self.window_store.tab_count() > 1)
    }
}
