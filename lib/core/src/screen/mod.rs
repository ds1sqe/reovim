#![allow(clippy::missing_errors_doc)]

pub mod border;
mod status_line;

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
        command_line::CommandLine,
        component::RenderState,
        constants::RESET_STYLE,
        decoration::DecorationStore,
        frame::{FrameBuffer, FrameRenderer},
        highlight::{ColorMode, HighlightStore, Theme},
        indent::IndentAnalyzer,
        modd::ModeState,
        modifier::{ModifierContext, ModifierRegistry},
        ui_component::ComponentId,
        visibility::BufferVisibilitySource,
    },
    reovim_sys::{
        cursor::{Hide, MoveTo, Show},
        event::{DisableMouseCapture, EnableMouseCapture},
        queue,
        style::Print,
        terminal::size,
    },
    std::{
        collections::BTreeMap,
        io::{self, Write},
    },
    window::{Anchor, LineNumber, Window},
};

pub use status_line::{StatusLineRenderer, render_command_line_to, render_status_line_to};

pub mod cusor;
pub mod layout;
pub mod split;
pub mod tab;
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
    /// Optional frame renderer for buffered rendering
    frame_renderer: Option<FrameRenderer>,
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
            window_type: WindowType::Editor,
            anchor,
            width: columns,
            height: editor_height,
            buffer_anchor: anchor,
            buffer_id: 0,
            line_number: LineNumber::default(),
            scrollbar_enabled: false,
            is_active: true,
            cursor: Position { x: 0, y: 0 },
            desired_col: None,
            border_config: None,
            is_floating: false,
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
            frame_renderer: None,
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
            window_type: WindowType::Editor,
            anchor,
            width,
            height: editor_height,
            buffer_anchor: anchor,
            buffer_id: 0,
            line_number: LineNumber::default(),
            scrollbar_enabled: false,
            is_active: true,
            cursor: Position { x: 0, y: 0 },
            desired_col: None,
            border_config: None,
            is_floating: false,
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
            frame_renderer: None,
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

        // Resize frame renderer if enabled
        if let Some(ref mut renderer) = self.frame_renderer {
            renderer.resize(width, height);
        }
    }

    /// Enable frame-buffered rendering
    ///
    /// When enabled, rendering will use double-buffering and differential
    /// updates instead of clearing the entire screen each frame.
    ///
    /// This is idempotent - if a frame renderer is already enabled, this does nothing.
    /// This ensures any existing capture handles remain valid.
    pub fn enable_frame_renderer(&mut self) {
        if self.frame_renderer.is_some() {
            return; // Already enabled, preserve existing capture handles
        }
        let renderer = FrameRenderer::new(self.size.width, self.size.height);
        self.frame_renderer = Some(renderer);
        tracing::info!("Frame renderer enabled");
    }

    /// Enable frame buffer capture and return a handle for external readers
    ///
    /// This enables capture on the frame renderer and returns a handle that
    /// provides thread-safe access to the latest complete frame.
    /// Used by RPC server for `CellGrid` format.
    ///
    /// Returns `None` if frame renderer is not enabled.
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

    /// Check if frame-buffered rendering is enabled
    #[must_use]
    pub const fn is_frame_renderer_enabled(&self) -> bool {
        self.frame_renderer.is_some()
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

    /// Get layer visibility information for visual debugging
    ///
    /// Dynamically queries the overlay registry for visible overlays.
    /// Core knows only about base and editor layers; plugins provide their own.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)]
    pub fn layer_info(
        &self,
        overlay_registry: &crate::overlay::OverlayRegistry,
        ctx: &crate::component::RenderContext<'_>,
    ) -> Vec<crate::visual::LayerInfo> {
        use crate::visual::{BoundsInfo, LayerInfo};

        let mut layers = Vec::new();

        // Base layer (tab line, status line)
        layers.push(LayerInfo {
            name: "base".to_string(),
            z_order: z_order::BASE,
            visible: true,
            bounds: BoundsInfo::new(0, 0, self.size.width, self.size.height),
        });

        // Editor layer
        layers.push(LayerInfo {
            name: "editor".to_string(),
            z_order: z_order::EDITOR,
            visible: true,
            bounds: BoundsInfo::new(0, 1, self.size.width, self.size.height.saturating_sub(2)),
        });

        // Query overlay registry for visible overlays
        for overlay in overlay_registry.visible_overlays_sorted(ctx) {
            if let Ok(o) = overlay.read() {
                let bounds = o.bounds(ctx);
                layers.push(LayerInfo {
                    name: o.id().to_string(),
                    z_order: o.z_order() as u8,
                    visible: true,
                    bounds: BoundsInfo::new(bounds.x, bounds.y, bounds.width, bounds.height),
                });
            }
        }

        layers
    }

    /// Render with bundled state
    ///
    /// This is the preferred rendering method that takes all runtime state as a single
    /// `RenderState` struct instead of many individual parameters.
    pub fn render_with_state(
        &mut self,
        state: &RenderState<'_>,
    ) -> std::result::Result<(), std::io::Error> {
        // Delegate to the existing render_buffered with extracted fields
        if self.frame_renderer.is_some() {
            return self.render_buffered(
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
            );
        }

        // Fallback: direct rendering (legacy path)
        self.render_direct(
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
        )
    }

    /// Diff-based rendering using frame buffer
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_lines)]
    fn render_buffered(
        &mut self,
        buffers: &BTreeMap<usize, Buffer>,
        highlight_store: &HighlightStore,
        mode: &ModeState,
        cmd_line: &CommandLine,
        pending_keys: &str,
        last_command: &str,
        color_mode: ColorMode,
        theme: &Theme,
        visibility_source: &dyn BufferVisibilitySource,
        indent_analyzer: &IndentAnalyzer,
        modifier_registry: Option<&ModifierRegistry>,
        decoration_store: Option<&DecorationStore>,
    ) -> std::result::Result<(), std::io::Error> {
        // Take frame renderer out (borrow checker workaround)
        let mut renderer = self
            .frame_renderer
            .take()
            .expect("render_buffered called without frame renderer");

        // Set color mode for style conversion
        renderer.set_color_mode(color_mode);

        // Clear buffer for fresh frame
        renderer.clear();
        let buffer = renderer.buffer_mut();

        // Track cursor position
        let mut cursor_pos: Option<(u16, u16)> = None;
        let mut current_buffer: Option<&Buffer> = None;

        // Render tab line if multiple tabs exist
        self.render_tab_line_to_buffer(buffer, color_mode, theme);

        // Render editor windows
        for win in &mut self.windows {
            if let Some(buf) = buffers.get(&win.buffer_id) {
                current_buffer = Some(buf);

                // Evaluate modifiers for this window
                if let Some(registry) = modifier_registry {
                    let filetype = buf
                        .file_path
                        .as_ref()
                        .map(|p| crate::filetype::filetype_id(p));
                    let mod_ctx = ModifierContext::new(
                        ComponentId::EDITOR,
                        &mode.edit_mode,
                        &mode.sub_mode,
                        win.id,
                        win.buffer_id,
                    )
                    .with_filetype(filetype)
                    .with_active(win.is_active)
                    .with_modified(buf.modified)
                    .with_floating(win.is_floating);

                    let style_state = registry.evaluate(&mod_ctx);

                    // Apply window decorations from modifiers
                    if let Some(show_ln) = style_state.style.decorations.line_numbers {
                        win.line_number.set_number(show_ln);
                    }
                    if let Some(relative) = style_state.style.decorations.relative_numbers {
                        win.line_number.set_relative_number(relative);
                    }
                    if let Some(scrollbar) = style_state.style.decorations.scrollbar {
                        win.scrollbar_enabled = scrollbar;
                    }
                }

                // Update scroll to keep cursor visible
                // Active window uses buffer's live cursor; inactive windows use saved cursor
                let effective_cursor_y = if win.is_active {
                    buf.cur.y
                } else {
                    win.cursor.y
                };
                win.update_scroll(effective_cursor_y);

                // Render window content to buffer
                win.render_to_buffer(
                    buffer,
                    buf,
                    highlight_store,
                    theme,
                    visibility_source,
                    indent_analyzer,
                    decoration_store,
                    &mode.edit_mode,
                );

                // Calculate cursor position only for the ACTIVE window (and if editor is focused)
                if win.is_active {
                    let gutter_width = win.line_number_width(buf.contents.len());
                    let cursor_x = win.anchor.x + gutter_width + buf.cur.x;
                    let cursor_y = win.anchor.y + buf.cur.y.saturating_sub(win.buffer_anchor.y);
                    cursor_pos = Some((cursor_x, cursor_y));
                }
            }
        }

        // Render window separators
        self.render_window_separators_to_buffer(buffer, theme);

        // Render status line or command line
        if mode.is_command() {
            self.render_command_line_to_buffer(buffer, cmd_line, theme);
        } else {
            self.render_status_line_to_buffer(
                buffer,
                mode,
                current_buffer,
                pending_keys,
                last_command,
                theme,
                color_mode,
            );
        }

        // Settings menu overlay is now rendered by the settings-menu plugin

        // Put renderer back and flush
        self.frame_renderer = Some(renderer);
        let renderer = self.frame_renderer.as_mut().unwrap();
        queue!(self.out_stream, Hide)?;
        renderer.flush(&mut self.out_stream)?;

        // Position cursor
        if let Some((x, y)) = cursor_pos {
            queue!(self.out_stream, MoveTo(x, y))?;
        }

        queue!(self.out_stream, Show)?;
        self.out_stream.flush()
    }

    /// Direct rendering (legacy path without frame buffer)
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_lines)]
    fn render_direct(
        &mut self,
        buffers: &BTreeMap<usize, Buffer>,
        highlight_store: &HighlightStore,
        mode: &ModeState,
        cmd_line: &CommandLine,
        pending_keys: &str,
        last_command: &str,
        color_mode: ColorMode,
        theme: &Theme,
        visibility_source: &dyn BufferVisibilitySource,
        indent_analyzer: &IndentAnalyzer,
    ) -> std::result::Result<(), std::io::Error> {
        // Reset all styling and hide cursor during render
        queue!(self.out_stream, Print(RESET_STYLE))?;
        queue!(self.out_stream, Hide)?;

        // Render tab line if multiple tabs exist
        self.render_tab_line(color_mode, theme)?;

        // Track cursor position from main buffer
        let mut cursor_pos: Option<(u16, u16)> = None;
        let mut current_buffer: Option<&Buffer> = None;

        // Render editor windows
        for win in &mut self.windows {
            if let Some(buf) = buffers.get(&win.buffer_id) {
                current_buffer = Some(buf);

                // Update scroll to keep cursor visible
                // Active window uses buffer's live cursor; inactive windows use saved cursor
                let effective_cursor_y = if win.is_active {
                    buf.cur.y
                } else {
                    win.cursor.y
                };
                win.update_scroll(effective_cursor_y);

                // Render each line with explicit cursor positioning
                let lines = win.render(
                    buf,
                    highlight_store,
                    color_mode,
                    theme,
                    visibility_source,
                    indent_analyzer,
                );
                for (row_offset, line) in lines.iter().enumerate() {
                    queue!(
                        self.out_stream,
                        MoveTo(win.anchor.x, win.anchor.y + row_offset as u16)
                    )?;
                    queue!(self.out_stream, Print(line))?;
                }

                // Calculate cursor position relative to window (only if editor is focused)
                if !self.layout.has_plugin_focus() {
                    // Account for line number gutter width
                    let gutter_width = win.line_number_width(buf.contents.len());
                    let cursor_x = win.anchor.x + gutter_width + buf.cur.x;
                    // Account for scroll offset (buffer_anchor.y)
                    let cursor_y = win.anchor.y + buf.cur.y.saturating_sub(win.buffer_anchor.y);
                    cursor_pos = Some((cursor_x, cursor_y));
                }
            }
        }

        // Render window separators for split windows
        self.render_window_separators(color_mode, theme)?;

        // Completion popup rendering is handled by the completion plugin overlay

        // Show command line in Command mode, status line otherwise
        if mode.is_command() {
            render_command_line_to(&mut self.out_stream, self.size.height, cmd_line)?;
        } else {
            render_status_line_to(
                &mut self.out_stream,
                self.size.width,
                self.size.height,
                mode,
                current_buffer,
                pending_keys,
                last_command,
                theme,
                color_mode,
            )?;
        }

        // Settings menu overlay is now rendered by the settings-menu plugin
        // Telescope overlay is now rendered by the telescope plugin

        // Position cursor at buffer cursor (not in command mode)
        if !mode.is_command()
            && let Some((x, y)) = cursor_pos
        {
            queue!(self.out_stream, MoveTo(x, y))?;
        }

        queue!(self.out_stream, Show)?;
        self.out_stream.flush()
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
                    win.buffer_id = buffer_id;
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
                win.buffer_id = buffer_id;
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

            // Preserve cursor positions before rebuilding
            let old_cursors: std::collections::HashMap<usize, (Position, Option<u16>)> = self
                .windows
                .iter()
                .map(|w| (w.id, (w.cursor, w.desired_col)))
                .collect();

            // Rebuild the windows vec from split tree layouts
            self.windows.clear();
            for layout in &layouts {
                let buffer_id = self
                    .window_buffers
                    .get(&layout.window_id)
                    .copied()
                    .unwrap_or(0);

                // Try to preserve cursor from previous window, otherwise default to (0,0)
                let (cursor, desired_col) = old_cursors
                    .get(&layout.window_id)
                    .copied()
                    .unwrap_or((Position { x: 0, y: 0 }, None));

                self.windows.push(Window {
                    id: layout.window_id,
                    window_type: WindowType::Editor,
                    anchor: Anchor {
                        x: layout.rect.x,
                        y: layout.rect.y,
                    },
                    width: layout.rect.width,
                    height: layout.rect.height,
                    buffer_anchor: Anchor { x: 0, y: 0 },
                    buffer_id,
                    line_number: LineNumber::default(),
                    scrollbar_enabled: false,
                    is_active: layout.window_id == active_window_id,
                    cursor,
                    desired_col,
                    border_config: None,
                    is_floating: false,
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
    fn update_window_active_state(&mut self) {
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

    /// Render the tab line at the top of the screen
    #[allow(clippy::cast_possible_truncation)]
    fn render_tab_line(
        &mut self,
        color_mode: ColorMode,
        theme: &Theme,
    ) -> std::result::Result<(), std::io::Error> {
        use reovim_sys::style::{Attribute, SetAttribute};

        let tabs = self.tab_manager.tab_info();
        if tabs.len() <= 1 {
            return Ok(());
        }

        queue!(self.out_stream, MoveTo(0, 0))?;

        let mut x = 0u16;
        for tab in &tabs {
            let style = if tab.is_active {
                &theme.tab.active
            } else {
                &theme.tab.inactive
            };

            let style_start = style.to_ansi_start(color_mode);
            let label = format!(" {} ", tab.label);
            let label_len = label.len() as u16;

            // Check if we have room
            if x + label_len > self.size.width {
                break;
            }

            queue!(self.out_stream, Print(&style_start))?;
            if tab.is_active {
                queue!(self.out_stream, SetAttribute(Attribute::Bold))?;
            }
            queue!(self.out_stream, Print(&label))?;
            queue!(self.out_stream, Print(RESET_STYLE))?;

            x += label_len;
        }

        // Fill the rest of the line with tab line background
        if x < self.size.width {
            let fill_style = theme.tab.fill.to_ansi_start(color_mode);
            let spaces = " ".repeat((self.size.width - x) as usize);
            queue!(self.out_stream, Print(&fill_style))?;
            queue!(self.out_stream, Print(&spaces))?;
            queue!(self.out_stream, Print(RESET_STYLE))?;
        }

        Ok(())
    }

    /// Get the Y offset for the editor area (1 if tabs are shown, 0 otherwise)
    #[must_use]
    fn tab_line_height(&self) -> u16 {
        u16::from(self.tab_manager.tab_count() > 1)
    }

    /// Render window separators for split windows
    #[allow(clippy::cast_possible_truncation)]
    fn render_window_separators(
        &mut self,
        color_mode: ColorMode,
        theme: &Theme,
    ) -> std::result::Result<(), std::io::Error> {
        if self.windows.len() <= 1 {
            return Ok(());
        }

        let sep_style = theme.window.separator.to_ansi_start(color_mode);

        // Find vertical separators (where windows meet side-by-side)
        for i in 0..self.windows.len() {
            for j in (i + 1)..self.windows.len() {
                let win_a = &self.windows[i];
                let win_b = &self.windows[j];

                // Check if windows are adjacent horizontally (vertical separator)
                if win_a.anchor.x + win_a.width == win_b.anchor.x {
                    // Draw vertical separator at the boundary
                    let sep_x = win_b.anchor.x.saturating_sub(1);
                    let start_y = win_a.anchor.y.max(win_b.anchor.y);
                    let end_y = (win_a.anchor.y + win_a.height).min(win_b.anchor.y + win_b.height);

                    queue!(self.out_stream, Print(&sep_style))?;
                    for y in start_y..end_y {
                        queue!(self.out_stream, MoveTo(sep_x, y))?;
                        queue!(self.out_stream, Print("│"))?;
                    }
                    queue!(self.out_stream, Print(RESET_STYLE))?;
                }

                // Check if windows are adjacent vertically (horizontal separator)
                if win_a.anchor.y + win_a.height == win_b.anchor.y {
                    // Draw horizontal separator at the boundary
                    let sep_y = win_b.anchor.y.saturating_sub(1);
                    let start_x = win_a.anchor.x.max(win_b.anchor.x);
                    let end_x = (win_a.anchor.x + win_a.width).min(win_b.anchor.x + win_b.width);

                    queue!(self.out_stream, Print(&sep_style))?;
                    queue!(self.out_stream, MoveTo(start_x, sep_y))?;
                    let sep_line = "─".repeat((end_x - start_x) as usize);
                    queue!(self.out_stream, Print(&sep_line))?;
                    queue!(self.out_stream, Print(RESET_STYLE))?;
                }
            }
        }

        Ok(())
    }

    // === Buffer Rendering Methods (for diff-based rendering) ===

    /// Render tab line directly to frame buffer
    #[allow(clippy::cast_possible_truncation)]
    fn render_tab_line_to_buffer(
        &self,
        buffer: &mut FrameBuffer,
        _color_mode: ColorMode,
        theme: &Theme,
    ) {
        let tabs = self.tab_manager.tab_info();
        if tabs.len() <= 1 {
            return;
        }

        let mut x = 0u16;
        for tab in &tabs {
            let style = if tab.is_active {
                &theme.tab.active
            } else {
                &theme.tab.inactive
            };

            let label = format!(" {} ", tab.label);
            for ch in label.chars() {
                if x < buffer.width() {
                    buffer.put_char(x, 0, ch, style);
                    x += 1;
                }
            }
        }

        // Fill rest with tab fill style
        let fill_style = &theme.tab.fill;
        while x < buffer.width() {
            buffer.put_char(x, 0, ' ', fill_style);
            x += 1;
        }
    }

    /// Render window separators to frame buffer
    fn render_window_separators_to_buffer(&self, buffer: &mut FrameBuffer, theme: &Theme) {
        if self.windows.len() <= 1 {
            return;
        }

        let sep_style = &theme.window.separator;

        for i in 0..self.windows.len() {
            for j in (i + 1)..self.windows.len() {
                let win_a = &self.windows[i];
                let win_b = &self.windows[j];

                // Vertical separator
                if win_a.anchor.x + win_a.width == win_b.anchor.x {
                    let sep_x = win_b.anchor.x.saturating_sub(1);
                    let start_y = win_a.anchor.y.max(win_b.anchor.y);
                    let end_y = (win_a.anchor.y + win_a.height).min(win_b.anchor.y + win_b.height);

                    for y in start_y..end_y {
                        buffer.put_char(sep_x, y, '│', sep_style);
                    }
                }

                // Horizontal separator
                if win_a.anchor.y + win_a.height == win_b.anchor.y {
                    let sep_y = win_b.anchor.y.saturating_sub(1);
                    let start_x = win_a.anchor.x.max(win_b.anchor.x);
                    let end_x = (win_a.anchor.x + win_a.width).min(win_b.anchor.x + win_b.width);

                    for x in start_x..end_x {
                        buffer.put_char(x, sep_y, '─', sep_style);
                    }
                }
            }
        }
    }

    /// Render command line to frame buffer
    #[allow(clippy::cast_possible_truncation)]
    fn render_command_line_to_buffer(
        &self,
        buffer: &mut FrameBuffer,
        cmd_line: &CommandLine,
        theme: &Theme,
    ) {
        let y = self.size.height.saturating_sub(1);
        let style = &theme.base.default;

        // Write colon prompt
        buffer.put_char(0, y, ':', style);

        // Write command text
        for (i, ch) in cmd_line.input.chars().enumerate() {
            let x = 1 + i as u16;
            if x < buffer.width() {
                buffer.put_char(x, y, ch, style);
            }
        }

        // Clear rest of line
        let input_len = cmd_line.input.len() as u16;
        for x in (1 + input_len)..buffer.width() {
            buffer.put_char(x, y, ' ', style);
        }
    }

    /// Render status line to frame buffer
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::similar_names)]
    fn render_status_line_to_buffer(
        &self,
        buffer: &mut FrameBuffer,
        mode: &ModeState,
        current_buffer: Option<&Buffer>,
        pending_keys: &str,
        _last_command: &str,
        theme: &Theme,
        _color_mode: ColorMode,
    ) {
        let y = self.size.height.saturating_sub(1);

        // Mode indicator
        let mode_display = mode.display_string();
        let mode_text = format!(" {mode_display} ");
        let mode_style = &theme.statusline.mode.normal;
        let mut x = 0u16;

        for ch in mode_text.chars() {
            if x < buffer.width() {
                buffer.put_char(x, y, ch, mode_style);
                x += 1;
            }
        }

        // File name
        let file_style = &theme.statusline.filename;
        if let Some(buf) = current_buffer {
            let file_name = buf.file_path.as_deref().unwrap_or("[No Name]");
            let file_text = format!(" {file_name} ");
            for ch in file_text.chars() {
                if x < buffer.width() {
                    buffer.put_char(x, y, ch, file_style);
                    x += 1;
                }
            }

            // Modified indicator
            if buf.modified {
                let modified_style = &theme.statusline.modified;
                buffer.put_char(x, y, '[', modified_style);
                x += 1;
                buffer.put_char(x, y, '+', modified_style);
                x += 1;
                buffer.put_char(x, y, ']', modified_style);
                x += 1;
            }
        }

        // Fill middle with background style
        let fill_end = self
            .size
            .width
            .saturating_sub(pending_keys.len() as u16 + 10);
        let bg_style = &theme.statusline.background;
        while x < fill_end {
            buffer.put_char(x, y, ' ', bg_style);
            x += 1;
        }

        // Position info (right side)
        if let Some(buf) = current_buffer {
            let pos_text = format!("{}:{} ", buf.cur.y + 1, buf.cur.x + 1);
            let pos_style = &theme.statusline.position;

            // Right-align position
            let pos_start = self
                .size
                .width
                .saturating_sub(pos_text.len() as u16 + pending_keys.len() as u16);
            for (i, ch) in pos_text.chars().enumerate() {
                let px = pos_start + i as u16;
                if px < buffer.width() {
                    buffer.put_char(px, y, ch, pos_style);
                }
            }
        }

        // Pending keys (far right)
        if !pending_keys.is_empty() {
            let keys_style = &theme.statusline.filetype;
            let keys_start = self.size.width.saturating_sub(pending_keys.len() as u16);
            for (i, ch) in pending_keys.chars().enumerate() {
                let px = keys_start + i as u16;
                if px < buffer.width() {
                    buffer.put_char(px, y, ch, keys_style);
                }
            }
        }
    }

    // Settings menu rendering is now handled by the settings-menu plugin
    // Telescope buffer rendering is now handled by the telescope plugin
}

// Implement StatusLineRenderer trait for Screen
impl StatusLineRenderer for Screen {
    fn render_status_line(
        &mut self,
        mode: &ModeState,
        buffer: Option<&Buffer>,
        pending_keys: &str,
        last_command: &str,
        theme: &Theme,
        color_mode: ColorMode,
    ) -> std::result::Result<(), std::io::Error> {
        render_status_line_to(
            &mut self.out_stream,
            self.size.width,
            self.size.height,
            mode,
            buffer,
            pending_keys,
            last_command,
            theme,
            color_mode,
        )
    }

    fn render_command_line(
        &mut self,
        cmd_line: &CommandLine,
    ) -> std::result::Result<(), std::io::Error> {
        render_command_line_to(&mut self.out_stream, self.size.height, cmd_line)
    }
}
