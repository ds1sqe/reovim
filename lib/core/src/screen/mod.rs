#![allow(clippy::missing_errors_doc)]

pub mod border;
mod render_pipeline;

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
        content::WindowContentSource,
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
    window::{Anchor, LineNumber, SignColumnMode, Window},
};

pub mod cmdline;
pub mod cusor;
pub mod layout;
pub mod separators;
pub mod split;
pub mod statusline;
pub mod tab;
pub mod tabline;
pub mod window;

pub use {
    border::{BorderConfig, BorderMode, BorderStyle, WindowAdjacency},
    layout::{LayoutManager, WindowType, is_plugin_window_id, plugin_window_id},
    split::{
        NavigateDirection, SplitDirection, SplitNode, WindowLayout, WindowRect, compute_adjacency,
        compute_all_adjacencies,
    },
    tab::{TabInfo, TabManager, TabPage},
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
    windows: Vec<Window>,
    layout: LayoutManager,
    /// Tab manager for split windows and tabs
    tab_manager: TabManager,
    /// Next available window ID
    next_window_id: usize,
    /// Mapping from `window_id` to `buffer_id`
    window_buffers: std::collections::BTreeMap<usize, usize>,
    /// Frame renderer for buffered rendering (always Some after initialization)
    frame_renderer: Option<FrameRenderer>,
    /// Pending viewport scroll events to be emitted by the runtime
    pending_viewport_scrolls: Vec<ViewportScrollInfo>,
}

impl Default for Screen {
    fn default() -> Self {
        let stdout = io::stdout();
        let (columns, rows) = size().expect("failed to get screen size on screen creation");
        let mut windows = Vec::new();
        let anchor = Anchor { x: 0, y: 0 };
        let editor_height = rows.saturating_sub(1); // Reserve last row for status line

        // Initial window ID is 0
        let initial_window_id = 0;

        windows.push(Window {
            id: initial_window_id,
            source: WindowContentSource::FileBuffer {
                buffer_id: 0,
                buffer_anchor: Anchor { x: 0, y: 0 },
            },
            anchor,
            width: columns,
            height: editor_height,
            z_order: 100, // Editor windows are in 100-199 range
            is_active: true,
            is_floating: false,
            line_number: Some(LineNumber::default()),
            scrollbar_enabled: false,
            sign_column_mode: SignColumnMode::Yes(2),
            cursor: Position { x: 0, y: 0 },
            desired_col: None,
            border_config: None,
        });

        let layout = LayoutManager::new(columns, editor_height);

        // Initialize tab manager with the first window
        let mut tab_manager = TabManager::new();
        tab_manager.init_with_window(initial_window_id);

        // Initial window -> buffer mapping
        let mut window_buffers = std::collections::BTreeMap::new();
        window_buffers.insert(initial_window_id, 0);

        Self {
            size: ScreenSize {
                width: columns,
                height: rows,
            },
            out_stream: Box::new(stdout),
            windows,
            layout,
            tab_manager,
            next_window_id: 1, // Next window will be ID 1
            window_buffers,
            frame_renderer: Some(FrameRenderer::new(columns, rows)),
            pending_viewport_scrolls: Vec::new(),
        }
    }
}

impl Screen {
    /// Create a new Screen with a custom writer (useful for testing/benchmarking)
    #[must_use]
    pub fn with_writer<W: Write + 'static>(writer: W, width: u16, height: u16) -> Self {
        let anchor = Anchor { x: 0, y: 0 };
        let editor_height = height.saturating_sub(1);

        let initial_window_id = 0;

        let windows = vec![Window {
            id: initial_window_id,
            source: WindowContentSource::FileBuffer {
                buffer_id: 0,
                buffer_anchor: Anchor { x: 0, y: 0 },
            },
            anchor,
            width,
            height: editor_height,
            z_order: 100, // Editor windows are in 100-199 range
            is_active: true,
            is_floating: false,
            line_number: Some(LineNumber::default()),
            scrollbar_enabled: false,
            sign_column_mode: SignColumnMode::Yes(2),
            cursor: Position { x: 0, y: 0 },
            desired_col: None,
            border_config: None,
        }];

        let layout = LayoutManager::new(width, editor_height);

        // Initialize tab manager with the first window
        let mut tab_manager = TabManager::new();
        tab_manager.init_with_window(initial_window_id);

        // Initial window -> buffer mapping
        let mut window_buffers = std::collections::BTreeMap::new();
        window_buffers.insert(initial_window_id, 0);

        Self {
            size: ScreenSize { height, width },
            out_stream: Box::new(writer),
            windows,
            layout,
            tab_manager,
            next_window_id: 1,
            window_buffers,
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
        for win in &self.windows {
            tracing::debug!(
                window_id = win.id,
                anchor_x = win.anchor.x,
                anchor_y = win.anchor.y,
                win_width = win.width,
                win_height = win.height,
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
        // Find all windows containing this position, sorted by z-order (highest first)
        self.windows
            .iter()
            .filter(|w| w.contains_screen_position(x, y))
            .max_by_key(|w| w.z_order)
            .map(|w| w.id)
    }

    /// Get a reference to a window by ID
    #[must_use]
    pub fn window(&self, window_id: usize) -> Option<&Window> {
        self.windows.iter().find(|w| w.id == window_id)
    }

    /// Get a mutable reference to a window by ID
    pub fn window_mut(&mut self, window_id: usize) -> Option<&mut Window> {
        self.windows.iter_mut().find(|w| w.id == window_id)
    }

    /// Set the active window by ID
    ///
    /// Returns true if the window was found and activated, false otherwise.
    pub fn set_active_window(&mut self, window_id: usize) -> bool {
        let found = self.windows.iter().any(|w| w.id == window_id);
        if found {
            for w in &mut self.windows {
                w.is_active = w.id == window_id;
            }
        }
        found
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

        for win in &mut self.windows {
            if let Some(buffer_id) = win.buffer_id()
                && let Some(buf) = buffers.get(&buffer_id)
            {
                let effective_cursor_y = if win.is_active {
                    buf.cur.y
                } else {
                    win.cursor.y
                };
                let scrolled = win.update_scroll(effective_cursor_y);
                if scrolled {
                    let (top_line, bottom_line) = win.viewport_bounds();
                    scrolls.push(ViewportScrollInfo {
                        window_id: win.id,
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
        self.windows
            .iter()
            .find(|w| w.buffer_id() == Some(buffer_id))
            .map(|win| {
                let (top_line, bottom_line) = win.viewport_bounds();
                ViewportScrollInfo {
                    window_id: win.id,
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
        for window in &mut self.windows {
            window.set_number(enabled);
        }
    }

    pub fn set_relative_number(&mut self, enabled: bool) {
        for window in &mut self.windows {
            window.set_relative_number(enabled);
        }
    }

    pub fn set_sign_column_mode(&mut self, mode: SignColumnMode) {
        for window in &mut self.windows {
            window.sign_column_mode = mode;
        }
    }

    pub fn set_scrollbar(&mut self, enabled: bool) {
        for window in &mut self.windows {
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
        // Set buffer for active window
        if let Some(window_id) = self.tab_manager.active_window_id() {
            self.window_buffers.insert(window_id, buffer_id);
            // Update the Window struct
            for win in &mut self.windows {
                if win.id == window_id {
                    win.set_buffer_id(buffer_id);
                    break;
                }
            }
        }
    }

    /// Set the buffer ID for a specific window
    pub fn set_window_buffer(&mut self, window_id: usize, buffer_id: usize) {
        self.window_buffers.insert(window_id, buffer_id);
        for win in &mut self.windows {
            if win.id == window_id {
                win.set_buffer_id(buffer_id);
                break;
            }
        }
    }

    /// Get the buffer ID for the active window
    #[must_use]
    pub fn active_buffer_id(&self) -> Option<usize> {
        self.tab_manager
            .active_window_id()
            .and_then(|wid| self.window_buffers.get(&wid).copied())
    }

    /// Get the active window ID
    #[must_use]
    pub fn active_window_id(&self) -> Option<usize> {
        self.tab_manager.active_window_id()
    }

    /// Get a reference to the active window
    #[must_use]
    pub fn active_window(&self) -> Option<&Window> {
        let active_id = self.active_window_id()?;
        self.windows.iter().find(|w| w.id == active_id)
    }

    /// Get a mutable reference to the active window
    pub fn active_window_mut(&mut self) -> Option<&mut Window> {
        let active_id = self.active_window_id()?;
        self.windows.iter_mut().find(|w| w.id == active_id)
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

        window.cursor = buffer.cur;
        window.desired_col = buffer.desired_col;

        tracing::debug!(
            "[CURSOR_SYNC] SAVE: win={} buffer.cur=({},{}) -> window.cursor",
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
        let current_active = self.active_window_id();

        // Early return if same window (no-op)
        if current_active == Some(window_id) {
            return None;
        }

        // Step 1: Save cursor to old window
        self.save_cursor_to_active_window(buffers);

        // Step 2: Update is_active flags
        for window in &mut self.windows {
            window.is_active = window.id == window_id;
        }
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            tab.active_window_id = window_id;
        }

        // Step 3: Load cursor from new window into buffer
        let new_window_data = self
            .windows
            .iter()
            .find(|w| w.id == window_id)
            .map(|w| (w.buffer_id(), w.cursor, w.desired_col));

        if let Some((Some(buffer_id), cursor, desired_col)) = new_window_data
            && let Some(buffer) = buffers.get_mut(&buffer_id)
        {
            buffer.cur = cursor;
            buffer.desired_col = desired_col;
            tracing::debug!(
                "[CURSOR_SYNC] LOAD: win={} window.cursor=({},{}) -> buffer.cur",
                window_id,
                cursor.x,
                cursor.y,
            );
        }

        current_active
    }

    /// Get the number of windows in the active tab
    #[must_use]
    pub fn window_count(&self) -> usize {
        self.tab_manager
            .active_tab()
            .map_or(0, tab::TabPage::window_count)
    }

    /// Get a reference to all windows
    #[must_use]
    pub fn windows(&self) -> &[Window] {
        &self.windows
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
        self.windows.clone()
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

        // Account for tab line height (1 row when multiple tabs exist)
        let tab_offset = self.tab_line_height();

        // Get the editor area rect, adjusted for tab line
        let editor_rect = WindowRect::new(
            editor_layout.anchor.x,
            editor_layout.anchor.y + tab_offset,
            editor_layout.width,
            editor_layout.height.saturating_sub(tab_offset),
        );

        // Calculate window layouts from the active tab's split tree
        if let Some(tab) = self.tab_manager.active_tab() {
            let layouts = tab.calculate_layouts(editor_rect);
            let active_window_id = tab.active_window_id;

            // Preserve window state before rebuilding
            let old_state: std::collections::HashMap<
                usize,
                (Position, Option<u16>, Option<Anchor>),
            > = self
                .windows
                .iter()
                .map(|w| (w.id, (w.cursor, w.desired_col, w.buffer_anchor())))
                .collect();

            // Rebuild the windows vec from split tree layouts
            self.windows.clear();
            for layout in &layouts {
                let buffer_id = self
                    .window_buffers
                    .get(&layout.window_id)
                    .copied()
                    .unwrap_or(0);

                // Try to preserve state from previous window, otherwise use defaults
                let (cursor, desired_col, buffer_anchor) =
                    old_state.get(&layout.window_id).copied().map_or(
                        (Position { x: 0, y: 0 }, None, Anchor { x: 0, y: 0 }),
                        |(c, d, a)| (c, d, a.unwrap_or(Anchor { x: 0, y: 0 })),
                    );

                self.windows.push(Window {
                    id: layout.window_id,
                    source: WindowContentSource::FileBuffer {
                        buffer_id,
                        buffer_anchor,
                    },
                    anchor: Anchor {
                        x: layout.rect.x,
                        y: layout.rect.y,
                    },
                    width: layout.rect.width,
                    height: layout.rect.height,
                    z_order: 100, // Editor windows are in 100-199 range
                    is_active: layout.window_id == active_window_id,
                    is_floating: false,
                    line_number: Some(LineNumber::default()),
                    scrollbar_enabled: false,
                    sign_column_mode: SignColumnMode::Yes(2),
                    cursor,
                    desired_col,
                    border_config: None,
                });
            }
        }
    }

    // === Window Split Operations ===

    /// Split the active window
    ///
    /// Returns the new window ID if successful
    pub fn split_window(&mut self, direction: SplitDirection) -> Option<usize> {
        let new_window_id = self.next_window_id;
        self.next_window_id += 1;

        // Save the current window's cursor to copy to the new window
        let parent_cursor = self
            .active_window()
            .map_or((Position { x: 0, y: 0 }, None), |w| (w.cursor, w.desired_col));

        if let Some(tab) = self.tab_manager.active_tab_mut() {
            // Get the current window's buffer to clone into the new window
            let current_buffer_id = self
                .window_buffers
                .get(&tab.active_window_id)
                .copied()
                .unwrap_or(0);

            tab.split(new_window_id, direction);
            self.window_buffers.insert(new_window_id, current_buffer_id);
            self.update_window_layouts();

            // Copy parent's cursor to the new window
            if let Some(new_win) = self.windows.iter_mut().find(|w| w.id == new_window_id) {
                new_win.cursor = parent_cursor.0;
                new_win.desired_col = parent_cursor.1;
            }

            Some(new_window_id)
        } else {
            None
        }
    }

    /// Close the active window
    ///
    /// Returns true if the last window in the last tab was closed (editor should quit)
    pub fn close_window(&mut self) -> bool {
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            let closing_window_id = tab.active_window_id;
            let is_last_window = tab.close_window(closing_window_id);

            // Remove window from buffer mapping
            self.window_buffers.remove(&closing_window_id);

            if is_last_window && !self.tab_manager.close_tab() {
                // Last window in last tab - return true to signal quit
                return true;
            }

            self.update_window_layouts();
        }
        false
    }

    /// Close all windows except the active one
    pub fn close_other_windows(&mut self) {
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            let active_id = tab.active_window_id;
            let all_ids: Vec<usize> = tab.window_ids();

            // Remove all other windows from buffer mapping
            for &id in &all_ids {
                if id != active_id {
                    self.window_buffers.remove(&id);
                }
            }

            // Reset the split tree to just the active window
            tab.root = SplitNode::leaf(active_id);
            self.update_window_layouts();
        }
    }

    /// Update `is_active` flag on all windows based on active window ID
    pub fn update_window_active_state(&mut self) {
        if let Some(active_id) = self.tab_manager.active_window_id() {
            for window in &mut self.windows {
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
        // Calculate layouts for navigation
        if let Some(tab) = self.tab_manager.active_tab() {
            let editor_layout = self.layout.editor_layout();
            let editor_rect = WindowRect::new(
                editor_layout.anchor.x,
                editor_layout.anchor.y,
                editor_layout.width,
                editor_layout.height,
            );
            let layouts = tab.calculate_layouts(editor_rect);

            // If a plugin has focus, unfocus it and focus the editor
            if self.layout.has_plugin_focus() {
                self.focus_editor();
                self.update_window_active_state();
                return Some(false);
            }

            // Navigate between editor windows
            let current_id = tab.active_window_id;
            if let Some(next_id) = split::find_adjacent_window(current_id, direction, &layouts) {
                if let Some(tab_mut) = self.tab_manager.active_tab_mut() {
                    tab_mut.active_window_id = next_id;
                }
                self.update_window_active_state();
            }
        }
        None
    }

    /// Equalize window sizes
    pub fn equalize_windows(&mut self) {
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            tab.equalize();
            self.update_window_layouts();
        }
    }

    /// Resize the active window in the specified direction
    ///
    /// - `Vertical` direction adjusts the width of vertical splits (left/right)
    /// - `Horizontal` direction adjusts the height of horizontal splits (top/bottom)
    /// - Positive delta increases size in that direction
    /// - Negative delta decreases size
    pub fn resize_window(&mut self, direction: SplitDirection, delta: f32) {
        if let Some(tab) = self.tab_manager.active_tab_mut() {
            tab.adjust_ratio_in_direction(direction, delta);
            self.update_window_layouts();
        }
    }

    /// Swap the active window with the window in the given direction
    ///
    /// Returns true if a swap occurred
    pub fn swap_window(&mut self, direction: NavigateDirection) -> bool {
        let Some(tab) = self.tab_manager.active_tab() else {
            return false;
        };

        // Calculate layouts for navigation
        let editor_layout = self.layout.editor_layout();
        let editor_rect = WindowRect::new(
            editor_layout.anchor.x,
            editor_layout.anchor.y,
            editor_layout.width,
            editor_layout.height,
        );
        let layouts = tab.calculate_layouts(editor_rect);
        let current_id = tab.active_window_id;

        // Find adjacent window
        let Some(target_id) = split::find_adjacent_window(current_id, direction, &layouts) else {
            return false;
        };

        // Swap window IDs in the split tree
        if let Some(tab_mut) = self.tab_manager.active_tab_mut()
            && tab_mut.root.swap_windows(current_id, target_id)
        {
            // Also swap buffer assignments
            let buf_a = self.window_buffers.get(&current_id).copied();
            let buf_b = self.window_buffers.get(&target_id).copied();
            if let Some(buf_a) = buf_a {
                self.window_buffers.insert(target_id, buf_a);
            }
            if let Some(buf_b) = buf_b {
                self.window_buffers.insert(current_id, buf_b);
            }
            self.update_window_layouts();
            return true;
        }
        false
    }

    // === Tab Operations ===

    /// Create a new tab
    ///
    /// Returns the new tab ID
    pub fn new_tab(&mut self, buffer_id: usize) -> usize {
        let new_window_id = self.next_window_id;
        self.next_window_id += 1;

        self.window_buffers.insert(new_window_id, buffer_id);
        let tab_id = self.tab_manager.new_tab(new_window_id);
        self.update_window_layouts();
        tab_id
    }

    /// Close the current tab
    ///
    /// Returns true if the last tab was closed (editor should quit)
    pub fn close_tab(&mut self) -> bool {
        // Remove window buffers for all windows in the current tab
        if let Some(tab) = self.tab_manager.active_tab() {
            for &window_id in &tab.window_ids() {
                self.window_buffers.remove(&window_id);
            }
        }

        if self.tab_manager.close_tab() {
            self.update_window_layouts();
            false
        } else {
            true // Last tab - should quit
        }
    }

    /// Switch to next tab
    pub fn next_tab(&mut self) {
        self.tab_manager.next_tab();
        self.update_window_layouts();
    }

    /// Switch to previous tab
    pub fn prev_tab(&mut self) {
        self.tab_manager.prev_tab();
        self.update_window_layouts();
    }

    /// Go to a specific tab by index
    pub fn goto_tab(&mut self, index: usize) {
        self.tab_manager.goto_tab(index);
        self.update_window_layouts();
    }

    /// Get tab information for display
    #[must_use]
    pub fn tab_info(&self) -> Vec<TabInfo> {
        self.tab_manager.tab_info()
    }

    /// Get the number of tabs
    #[must_use]
    pub fn tab_count(&self) -> usize {
        self.tab_manager.tab_count()
    }

    /// Get a reference to the tab manager
    #[must_use]
    pub const fn tab_manager(&self) -> &TabManager {
        &self.tab_manager
    }

    /// Get the Y offset for the editor area (1 if tabs are shown, 0 otherwise)
    #[must_use]
    fn tab_line_height(&self) -> u16 {
        u16::from(self.tab_manager.tab_count() > 1)
    }
}
