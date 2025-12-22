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
        content::WindowContentSource,
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
    /// Frame renderer for buffered rendering (always Some after initialization)
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
        )
    }

    /// Execute the render pipeline for a window
    #[allow(clippy::too_many_arguments)]
    fn execute_pipeline(
        &self,
        window: &Window,
        text_buffer: &Buffer,
        _highlight_store: &HighlightStore,
        theme: &Theme,
        color_mode: ColorMode,
        _visibility_source: &dyn BufferVisibilitySource,
        _indent_analyzer: &IndentAnalyzer,
        _decoration_store: Option<&DecorationStore>,
        render_stages: &std::sync::Arc<std::sync::RwLock<crate::render::RenderStageRegistry>>,
    ) -> crate::render::RenderData {
        use crate::{component::RenderContext, render::RenderData};

        // Stage 1: Extract buffer content
        let mut data = RenderData::from_buffer(window, text_buffer);
        data.buffer_id = window.buffer_id().unwrap_or(0);

        // Create render context
        let ctx = RenderContext::new(self.size.width, self.size.height, theme, color_mode);

        // Execute registered render stages in order
        {
            let stages_guard = render_stages.read().unwrap();
            for stage in stages_guard.stages() {
                tracing::trace!(stage_name = stage.name(), "Executing render stage");
                data = stage.transform(data, &ctx);
            }
        }

        // TODO: Stage 5: Visual selection overlay
        // TODO: Stage 6: Indent guides

        data
    }

    /// Render pipeline data to frame buffer
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::too_many_lines)]
    fn render_data_to_framebuffer(
        &self,
        render_data: &crate::render::RenderData,
        window: &Window,
        frame_buffer: &mut FrameBuffer,
        theme: &Theme,
        buffer: &Buffer,
    ) {
        use crate::render::LineVisibility;

        // Get effective cursor position (active window uses buffer cursor, inactive uses window cursor)
        let cursor_y = if window.is_active {
            buffer.cur.y
        } else {
            window.cursor.y
        };

        // Calculate selection bounds if active
        let selection_bounds = if buffer.selection.active {
            use crate::buffer::SelectionMode;
            match buffer.selection.mode {
                SelectionMode::Block => {
                    let anchor = buffer.selection.anchor;
                    let cursor = buffer.cur;
                    let top_left = Position {
                        x: anchor.x.min(cursor.x),
                        y: anchor.y.min(cursor.y),
                    };
                    let bottom_right = Position {
                        x: anchor.x.max(cursor.x),
                        y: anchor.y.max(cursor.y),
                    };
                    Some((top_left, bottom_right, SelectionMode::Block))
                }
                SelectionMode::Character | SelectionMode::Line => {
                    let anchor = buffer.selection.anchor;
                    let cursor = buffer.cur;
                    let (start, end) =
                        if anchor.y < cursor.y || (anchor.y == cursor.y && anchor.x <= cursor.x) {
                            (anchor, cursor)
                        } else {
                            (cursor, anchor)
                        };
                    Some((start, end, buffer.selection.mode))
                }
            }
        } else {
            None
        };

        // Get scroll offset from buffer anchor
        let scroll_offset = window.buffer_anchor().map_or(0, |anchor| anchor.y);

        // Calculate line number width
        let total_lines = render_data.lines.len();
        #[allow(clippy::cast_precision_loss)]
        #[allow(clippy::cast_possible_truncation)]
        #[allow(clippy::cast_sign_loss)]
        let num_width = if window
            .line_number
            .as_ref()
            .is_some_and(LineNumber::is_shown)
            && total_lines > 0
        {
            (total_lines as f64).log10().floor() as usize + 1
        } else {
            1
        };

        // Render each visible line, starting from scroll offset
        let mut display_row = 0u16;
        let start_line = scroll_offset as usize;
        for (idx, line) in render_data
            .lines
            .iter()
            .enumerate()
            .skip(start_line)
        {
            let line_idx = idx;
            if display_row >= window.height {
                break;
            }

            // Check visibility
            let visibility = &render_data.visibility[line_idx];
            match visibility {
                LineVisibility::Hidden => {}
                LineVisibility::FoldMarker { preview, .. } => {
                    let screen_y = window.anchor.y + display_row;
                    let gutter_width = self.render_line_number_to_buffer_simple(
                        frame_buffer,
                        window.anchor.x,
                        screen_y,
                        line_idx as u16,
                        cursor_y,
                        num_width,
                        theme,
                        window,
                    );

                    // Render fold marker
                    let fold_style = &theme.fold.marker;
                    let mut col = window.anchor.x + gutter_width;
                    for ch in preview.chars() {
                        if col < frame_buffer.width() {
                            frame_buffer.put_char(col, screen_y, ch, fold_style);
                            col += 1;
                        }
                    }
                    display_row += 1;
                }
                LineVisibility::Visible => {
                    let screen_y = window.anchor.y + display_row;
                    let gutter_width = self.render_line_number_to_buffer_simple(
                        frame_buffer,
                        window.anchor.x,
                        screen_y,
                        line_idx as u16,
                        cursor_y,
                        num_width,
                        theme,
                        window,
                    );

                    // Render line content
                    // TODO: Apply highlights and decorations
                    let mut col = window.anchor.x + gutter_width;
                    #[allow(clippy::cast_possible_truncation)]
                    let buffer_line_y = line_idx as u16;

                    for (char_idx, ch) in line.chars().enumerate() {
                        if col >= window.anchor.x + window.width {
                            break;
                        }

                        // Check if this character is within the selection
                        #[allow(clippy::cast_possible_truncation)]
                        let char_x = char_idx as u16;
                        let is_selected = if let Some((start, end, mode)) = selection_bounds {
                            use crate::buffer::SelectionMode;
                            match mode {
                                SelectionMode::Block => {
                                    // Block selection: check if within rectangle
                                    buffer_line_y >= start.y
                                        && buffer_line_y <= end.y
                                        && char_x >= start.x
                                        && char_x <= end.x
                                }
                                SelectionMode::Character | SelectionMode::Line => {
                                    // Character/line selection: check if within range
                                    if buffer_line_y < start.y || buffer_line_y > end.y {
                                        false
                                    } else if buffer_line_y == start.y && buffer_line_y == end.y {
                                        // Single line selection
                                        char_x >= start.x && char_x <= end.x
                                    } else if buffer_line_y == start.y {
                                        // First line of selection
                                        char_x >= start.x
                                    } else if buffer_line_y == end.y {
                                        // Last line of selection
                                        char_x <= end.x
                                    } else {
                                        // Middle lines - all selected
                                        true
                                    }
                                }
                            }
                        } else {
                            false
                        };

                        // Apply appropriate style
                        let style = if is_selected {
                            &theme.selection.visual
                        } else {
                            &theme.base.default
                        };

                        frame_buffer.put_char(col, screen_y, ch, style);
                        col += 1;
                    }
                    display_row += 1;
                }
            }
        }

        // Fill remaining rows with tilde markers
        let tilde_style = &theme.gutter.line_number;
        while display_row < window.height {
            let screen_y = window.anchor.y + display_row;
            frame_buffer.put_char(window.anchor.x, screen_y, '~', tilde_style);
            display_row += 1;
        }
    }

    /// Simplified line number rendering for pipeline
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::unused_self)]
    #[allow(clippy::cast_possible_truncation)]
    fn render_line_number_to_buffer_simple(
        &self,
        buffer: &mut FrameBuffer,
        x: u16,
        y: u16,
        row: u16,
        cursor_y: u16,
        num_width: usize,
        theme: &Theme,
        window: &Window,
    ) -> u16 {
        use crate::screen::window::LineNumberMode;

        let Some(line_number) = &window.line_number else {
            return 0;
        };
        if !line_number.is_shown() {
            return 0;
        }

        // Calculate plain text line number (without ANSI codes for frame buffer)
        let is_current_line = row == cursor_y;

        let num_str = if window.is_active {
            match line_number.mode() {
                LineNumberMode::Absolute => format!("{}", row + 1),
                LineNumberMode::Relative => {
                    let rel = (i32::from(row) - i32::from(cursor_y)).abs();
                    format!("{rel}")
                }
                LineNumberMode::Hybrid => {
                    if is_current_line {
                        format!("{}", row + 1)
                    } else {
                        let rel = (i32::from(row) - i32::from(cursor_y)).abs();
                        format!("{rel}")
                    }
                }
            }
        } else {
            format!("{}", row + 1)
        };

        // Format with padding and trailing space (plain text only)
        let line_num_str = format!("{num_str:>num_width$} ");

        // Render line number with style
        let line_num_style = if !window.is_active {
            &theme.gutter.inactive_line_number
        } else if is_current_line {
            &theme.gutter.current_line_number
        } else {
            &theme.gutter.line_number
        };

        let mut col = x;
        for ch in line_num_str.chars() {
            buffer.put_char(col, y, ch, line_num_style);
            col += 1;
        }

        line_num_str.len() as u16
    }

    /// Unified rendering using frame buffer (Phase 2 implementation)
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_lines)]
    fn render_windows(
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
        render_stages: &std::sync::Arc<std::sync::RwLock<crate::render::RenderStageRegistry>>,
        plugin_state: &std::sync::Arc<crate::plugin::PluginStateRegistry>,
    ) -> std::result::Result<(), std::io::Error> {
        // Take frame renderer out (borrow checker workaround)
        let mut renderer = self
            .frame_renderer
            .take()
            .expect("frame renderer should always be initialized");

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

        // Phase 2: Render editor windows
        // Phase 3 will add overlay collection and unified z-order rendering

        // Update scroll positions BEFORE cloning windows
        for win in &mut self.windows {
            if let Some(buffer_id) = win.buffer_id()
                && let Some(buf) = buffers.get(&buffer_id)
            {
                let effective_cursor_y = if win.is_active {
                    buf.cur.y
                } else {
                    win.cursor.y
                };
                win.update_scroll(effective_cursor_y);
            }
        }

        // Collect all windows (editor + plugin windows)
        let mut windows_to_render = self.windows.clone();

        // Collect windows from registered window providers
        for provider in plugin_state.window_providers() {
            let plugin_windows = provider.get_windows(plugin_state);
            windows_to_render.extend(plugin_windows);
        }

        // Sort all windows by z-order
        windows_to_render.sort_by_key(|w| w.z_order);

        // Render all windows
        for win in &windows_to_render {
            // Handle PluginBuffer windows differently
            if let crate::content::WindowContentSource::PluginBuffer { provider, .. } = &win.source
            {
                // For plugin buffers, generate virtual content via provider
                use crate::content::BufferContext;
                let buffer_ctx = BufferContext {
                    buffer_id: 0,
                    width: win.width,
                    height: win.height,
                    state: plugin_state,
                };
                let lines = provider.get_lines(&buffer_ctx);

                // Render plugin buffer lines directly to frame buffer
                for (line_idx, line) in lines.iter().enumerate() {
                    let y = win.anchor.y + line_idx as u16;
                    if y >= self.size.height {
                        break;
                    }
                    let mut x = win.anchor.x;
                    for ch in line.chars() {
                        if x >= win.anchor.x + win.width {
                            break;
                        }
                        buffer.put_char(x, y, ch, &theme.base.default);
                        x += 1;
                    }
                }
                continue; // Skip normal window rendering
            }

            // Normal FileBuffer windows
            if let Some(buffer_id) = win.buffer_id()
                && let Some(buf) = buffers.get(&buffer_id)
            {
                current_buffer = Some(buf);

                // Find the corresponding window in self.windows (for mutable access)
                // Plugin windows won't have a match, so we skip modifier/scroll updates for them
                let editor_win_idx = self.windows.iter().position(|w| w.id == win.id);

                // Evaluate modifiers for this window (only for editor windows)
                if let Some(editor_idx) = editor_win_idx
                    && let Some(registry) = modifier_registry
                {
                    let filetype = buf
                        .file_path
                        .as_ref()
                        .map(|p| crate::filetype::filetype_id(p));
                    let mod_ctx = ModifierContext::new(
                        ComponentId::EDITOR,
                        &mode.edit_mode,
                        &mode.sub_mode,
                        win.id,
                        buffer_id,
                    )
                    .with_filetype(filetype)
                    .with_active(win.is_active)
                    .with_modified(buf.modified)
                    .with_floating(win.is_floating);

                    let style_state = registry.evaluate(&mod_ctx);

                    // Apply window decorations from modifiers (need mutable access)
                    let win_mut = &mut self.windows[editor_idx];
                    if let Some(show_ln) = style_state.style.decorations.line_numbers {
                        win_mut.set_number(show_ln);
                    }
                    if let Some(relative) = style_state.style.decorations.relative_numbers {
                        win_mut.set_relative_number(relative);
                    }
                    if let Some(scrollbar) = style_state.style.decorations.scrollbar {
                        win_mut.scrollbar_enabled = scrollbar;
                    }
                    // Note: scroll position already updated before window cloning
                }

                // Execute render pipeline with the window from windows_to_render
                let render_data = self.execute_pipeline(
                    win,
                    buf,
                    highlight_store,
                    theme,
                    color_mode,
                    visibility_source,
                    indent_analyzer,
                    decoration_store,
                    render_stages,
                );

                // Render pipeline data to frame buffer
                self.render_data_to_framebuffer(&render_data, win, buffer, theme, buf);

                // Calculate cursor position only for the ACTIVE window (and if editor is focused)
                if win.is_active {
                    let gutter_width = win.line_number_width(buf.contents.len());
                    let cursor_x = win.anchor.x + gutter_width + buf.cur.x;
                    let cursor_y = win.anchor.y
                        + buf
                            .cur
                            .y
                            .saturating_sub(win.buffer_anchor().map_or(0, |a| a.y));
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
                    source: WindowContentSource::FileBuffer {
                        buffer_id,
                        buffer_anchor: Anchor { x: 0, y: 0 },
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

    /// Get the Y offset for the editor area (1 if tabs are shown, 0 otherwise)
    #[must_use]
    fn tab_line_height(&self) -> u16 {
        u16::from(self.tab_manager.tab_count() > 1)
    }

    // === Frame Buffer Rendering Methods ===

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
