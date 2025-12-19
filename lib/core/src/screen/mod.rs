#![allow(clippy::missing_errors_doc)]

mod compositor;
mod layer;
mod status_line;
mod which_key;

pub mod layers;

use {
    crate::{
        buffer::Buffer,
        command::terminal::{Clear, ClearType},
        command_line::CommandLine,
        completion::CompletionState,
        constants::RESET_STYLE,
        explorer::{ExplorerState, render_explorer},
        folding::FoldManager,
        frame::{FrameBuffer, FrameRenderer, RenderStrategyConfig},
        highlight::{ColorMode, HighlightStore, Theme},
        indent::IndentAnalyzer,
        leap::LeapState,
        modd::ModeState,
        telescope::TelescopeState,
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

pub use {
    compositor::Compositor,
    layer::{Layer, LayerBounds, z_order},
    status_line::{StatusLineRenderer, render_command_line_to, render_status_line_to},
    which_key::{WhichKeyConfig, WhichKeyPanel},
};

pub mod cusor;
pub mod layout;
pub mod split;
pub mod tab;
pub mod window;

pub use {
    layout::{LayoutManager, WindowType},
    split::{NavigateDirection, SplitDirection, SplitNode, WindowLayout, WindowRect},
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
    /// Z-layer compositor for flicker-free rendering
    compositor: Option<Compositor>,
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
            compositor: None,
        }
    }
}

impl Screen {
    /// Enable compositor-based rendering
    ///
    /// When enabled, rendering uses the z-layer compositor for flicker-free updates
    pub fn enable_compositor(&mut self) {
        self.compositor = Some(Compositor::new(self.size.width, self.size.height));
        tracing::info!("Z-layer compositor enabled");
    }

    /// Check if compositor rendering is enabled
    #[must_use]
    pub const fn is_compositor_enabled(&self) -> bool {
        self.compositor.is_some()
    }

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
            compositor: None,
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
        self.clear(ClearType::All)
    }

    pub fn finalize(&mut self) -> std::result::Result<(), std::io::Error> {
        self.disable_mouse_capture()?;
        self.clear(ClearType::All)
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
        tracing::debug!(
            screen_width = width,
            screen_height = height,
            explorer_visible = self.layout.is_explorer_visible(),
            explorer_width = self.layout.explorer_width(),
            "Screen resized"
        );
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

        // Resize compositor if enabled
        if let Some(ref mut compositor) = self.compositor {
            compositor.resize(width, height);
        }
    }

    /// Enable frame-buffered rendering with the specified strategy
    ///
    /// When enabled, rendering will use double-buffering and differential
    /// updates instead of clearing the entire screen each frame.
    pub fn enable_frame_renderer(&mut self, strategy: RenderStrategyConfig) {
        let mut renderer = FrameRenderer::new(self.size.width, self.size.height);
        renderer.set_strategy(strategy);
        self.frame_renderer = Some(renderer);
        tracing::info!(
            strategy = %strategy.name(),
            "Frame renderer enabled"
        );
    }

    /// Set the render strategy (only if frame renderer is enabled)
    pub fn set_render_strategy(&mut self, strategy: RenderStrategyConfig) {
        if let Some(ref mut renderer) = self.frame_renderer {
            renderer.set_strategy(strategy);
            tracing::debug!(strategy = %strategy.name(), "Render strategy changed");
        } else {
            // Enable frame renderer if not already enabled
            self.enable_frame_renderer(strategy);
        }
    }

    /// Get the current render strategy config
    #[must_use]
    pub fn render_strategy(&self) -> Option<RenderStrategyConfig> {
        self.frame_renderer.as_ref().map(FrameRenderer::strategy)
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
    #[must_use]
    #[allow(clippy::fn_params_excessive_bools)]
    pub fn layer_info(
        &self,
        explorer_visible: bool,
        which_key_visible: bool,
        completion_visible: bool,
        telescope_active: bool,
        leap_active: bool,
        settings_visible: bool,
    ) -> Vec<crate::visual::LayerInfo> {
        use crate::visual::{BoundsInfo, LayerInfo};

        let mut layers = Vec::new();

        // Base layer (tab line, status line)
        layers.push(LayerInfo {
            name: "base".to_string(),
            z_order: layer::z_order::BASE,
            visible: true,
            bounds: BoundsInfo::new(0, 0, self.size.width, self.size.height),
        });

        // Explorer
        if explorer_visible {
            let explorer_width = self.layout.explorer_width();
            if explorer_width > 0 {
                layers.push(LayerInfo {
                    name: "explorer".to_string(),
                    z_order: layer::z_order::EXPLORER,
                    visible: true,
                    bounds: BoundsInfo::new(
                        0,
                        1,
                        explorer_width,
                        self.size.height.saturating_sub(2),
                    ),
                });
            }
        }

        // Editor
        layers.push(LayerInfo {
            name: "editor".to_string(),
            z_order: layer::z_order::EDITOR,
            visible: true,
            bounds: BoundsInfo::new(0, 1, self.size.width, self.size.height.saturating_sub(2)),
        });

        // Leap
        if leap_active {
            layers.push(LayerInfo {
                name: "leap".to_string(),
                z_order: layer::z_order::LEAP,
                visible: true,
                bounds: BoundsInfo::new(0, 1, self.size.width, self.size.height.saturating_sub(2)),
            });
        }

        // Completion
        if completion_visible {
            layers.push(LayerInfo {
                name: "completion".to_string(),
                z_order: layer::z_order::COMPLETION,
                visible: true,
                bounds: BoundsInfo::new(0, 0, 40, 10), // Approximate
            });
        }

        // Which-key
        if which_key_visible {
            layers.push(LayerInfo {
                name: "which_key".to_string(),
                z_order: layer::z_order::WHICH_KEY,
                visible: true,
                bounds: BoundsInfo::new(
                    0,
                    self.size.height.saturating_sub(10),
                    self.size.width,
                    10,
                ),
            });
        }

        // Telescope
        if telescope_active {
            layers.push(LayerInfo {
                name: "telescope".to_string(),
                z_order: layer::z_order::TELESCOPE,
                visible: true,
                bounds: BoundsInfo::new(0, 0, self.size.width, self.size.height),
            });
        }

        // Settings menu
        if settings_visible {
            layers.push(LayerInfo {
                name: "settings".to_string(),
                z_order: layer::z_order::SETTINGS_MENU,
                visible: true,
                bounds: BoundsInfo::new(0, 0, self.size.width, self.size.height),
            });
        }

        layers
    }

    /// update screen using diff-based rendering
    ///
    /// When frame renderer is enabled, renders all components to a frame buffer
    /// and emits only changed cells to the terminal. This eliminates flickering.
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::too_many_arguments)]
    #[allow(clippy::too_many_lines)]
    pub fn render(
        &mut self,
        buffers: &BTreeMap<usize, Buffer>,
        highlight_store: &HighlightStore,
        mode: &ModeState,
        cmd_line: &CommandLine,
        pending_keys: &str,
        last_command: &str,
        color_mode: ColorMode,
        theme: &Theme,
        explorer_state: Option<&ExplorerState>,
        which_key_panel: &WhichKeyPanel,
        completion_state: &CompletionState,
        telescope_state: &TelescopeState,
        leap_state: &LeapState,
        fold_manager: &FoldManager,
        indent_analyzer: &IndentAnalyzer,
        settings_menu: &crate::settings_menu::SettingsMenuState,
    ) -> std::result::Result<(), std::io::Error> {
        // Use frame buffer for diff-based rendering when enabled
        if self.frame_renderer.is_some() {
            return self.render_buffered(
                buffers,
                highlight_store,
                mode,
                cmd_line,
                pending_keys,
                last_command,
                color_mode,
                theme,
                explorer_state,
                which_key_panel,
                completion_state,
                telescope_state,
                leap_state,
                fold_manager,
                indent_analyzer,
                settings_menu,
            );
        }

        // Fallback: direct rendering (legacy path)
        self.render_direct(
            buffers,
            highlight_store,
            mode,
            cmd_line,
            pending_keys,
            last_command,
            color_mode,
            theme,
            explorer_state,
            which_key_panel,
            completion_state,
            telescope_state,
            leap_state,
            fold_manager,
            indent_analyzer,
            settings_menu,
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
        explorer_state: Option<&ExplorerState>,
        which_key_panel: &WhichKeyPanel,
        completion_state: &CompletionState,
        telescope_state: &TelescopeState,
        leap_state: &LeapState,
        fold_manager: &FoldManager,
        indent_analyzer: &IndentAnalyzer,
        settings_menu: &crate::settings_menu::SettingsMenuState,
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

        // Render explorer sidebar if visible
        if self.layout.is_explorer_visible()
            && let Some(explorer) = explorer_state
            && let Some(layout) = self.layout.explorer_layout()
        {
            self.render_explorer_to_buffer(buffer, explorer, layout, theme, color_mode);

            // If explorer is focused, set cursor position
            if self.layout.is_explorer_focused() {
                let cursor_y = explorer.cursor_index.saturating_sub(explorer.scroll_offset);
                cursor_pos = Some((layout.anchor.x, layout.anchor.y + cursor_y as u16));
            }
        }

        // Collect leap rendering info before iterating windows
        let mut leap_render_info: Option<(u16, u16, u16)> = None;

        // Render editor windows
        for win in &mut self.windows {
            if let Some(buf) = buffers.get(&win.buffer_id) {
                current_buffer = Some(buf);

                // Update scroll to keep cursor visible
                win.update_scroll(buf.cur.y);

                // Get fold state for this buffer
                let fold_state = fold_manager.get(buf.id);

                // Render window content to buffer
                win.render_to_buffer(
                    buffer,
                    buf,
                    highlight_store,
                    theme,
                    fold_state,
                    indent_analyzer,
                );

                // Calculate cursor position only for the ACTIVE window (and if editor is focused)
                if win.is_active && !self.layout.is_explorer_focused() {
                    let gutter_width = win.line_number_width(buf.contents.len());
                    let cursor_x = win.anchor.x + gutter_width + buf.cur.x;
                    let cursor_y = win.anchor.y + buf.cur.y.saturating_sub(win.buffer_anchor.y);
                    cursor_pos = Some((cursor_x, cursor_y));

                    if leap_state.is_showing_labels() {
                        leap_render_info =
                            Some((win.anchor.x + gutter_width, win.anchor.y, win.buffer_anchor.y));
                    }
                }
            }
        }

        // Render window separators
        self.render_window_separators_to_buffer(buffer, theme);

        // Render leap labels
        if let Some((window_x, window_y, scroll_offset)) = leap_render_info {
            self.render_leap_labels_to_buffer(
                buffer,
                leap_state,
                window_x,
                window_y,
                scroll_offset,
                theme,
            );
        }

        // Render completion popup if visible
        if completion_state.is_visible()
            && let Some((cursor_x, cursor_y)) = cursor_pos
        {
            self.render_completion_to_buffer(buffer, completion_state, cursor_x, cursor_y, theme);
        }

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

        // Render which-key panel overlay
        if which_key_panel.visible {
            self.render_which_key_to_buffer(buffer, which_key_panel, color_mode, theme);
        }

        // Render settings menu overlay
        if settings_menu.visible {
            self.render_settings_menu_to_buffer(buffer, settings_menu, theme, color_mode);
            // Cursor position for settings menu
            let cursor_y = settings_menu.layout.y
                + 2
                + settings_menu
                    .selected_index
                    .saturating_sub(settings_menu.scroll_offset) as u16;
            let cursor_x = settings_menu.layout.x + 2;
            cursor_pos = Some((cursor_x, cursor_y));
        }

        // Render telescope overlay (takes over screen when active)
        if telescope_state.is_visible() {
            self.render_telescope_to_buffer(buffer, telescope_state, theme, color_mode);
            let prompt_len = telescope_state.prompt.len() as u16;
            let cursor_x =
                telescope_state.layout.x + 1 + prompt_len + telescope_state.cursor_pos as u16;
            let cursor_y = telescope_state.layout.y + telescope_state.layout.height - 2;
            cursor_pos = Some((cursor_x, cursor_y));
        }

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
        explorer_state: Option<&ExplorerState>,
        which_key_panel: &WhichKeyPanel,
        completion_state: &CompletionState,
        telescope_state: &TelescopeState,
        leap_state: &LeapState,
        fold_manager: &FoldManager,
        indent_analyzer: &IndentAnalyzer,
        settings_menu: &crate::settings_menu::SettingsMenuState,
    ) -> std::result::Result<(), std::io::Error> {
        // Reset all styling and hide cursor during render
        queue!(self.out_stream, Print(RESET_STYLE))?;
        queue!(self.out_stream, Hide)?;

        // Render tab line if multiple tabs exist
        self.render_tab_line(color_mode, theme)?;

        // Track cursor position from main buffer
        let mut cursor_pos: Option<(u16, u16)> = None;
        let mut current_buffer: Option<&Buffer> = None;

        // Render explorer sidebar if visible
        if self.layout.is_explorer_visible()
            && let Some(explorer) = explorer_state
            && let Some(layout) = self.layout.explorer_layout()
        {
            // Account for tab line when multiple tabs exist
            let tab_offset = self.tab_line_height();
            let explorer_height = layout.height.saturating_sub(tab_offset);

            let lines = render_explorer(explorer, layout.width, explorer_height, theme, color_mode);
            for (row_offset, line) in lines.iter().enumerate() {
                queue!(
                    self.out_stream,
                    MoveTo(layout.anchor.x, layout.anchor.y + tab_offset + row_offset as u16)
                )?;
                queue!(self.out_stream, Print(line))?;
            }

            // If explorer is focused, set cursor position in explorer
            if self.layout.is_explorer_focused() {
                let cursor_y = explorer.cursor_index.saturating_sub(explorer.scroll_offset);
                cursor_pos =
                    Some((layout.anchor.x, layout.anchor.y + tab_offset + cursor_y as u16));
            }
        }

        // Collect leap rendering info before iterating windows
        let mut leap_render_info: Option<(u16, u16, u16)> = None;

        // Render editor windows
        for win in &mut self.windows {
            if let Some(buf) = buffers.get(&win.buffer_id) {
                current_buffer = Some(buf);

                // Update scroll to keep cursor visible
                win.update_scroll(buf.cur.y);

                // Get fold state for this buffer
                let fold_state = fold_manager.get(buf.id);

                // Render each line with explicit cursor positioning
                let lines = win.render(
                    buf,
                    highlight_store,
                    color_mode,
                    theme,
                    fold_state,
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
                if !self.layout.is_explorer_focused() {
                    // Account for line number gutter width
                    let gutter_width = win.line_number_width(buf.contents.len());
                    let cursor_x = win.anchor.x + gutter_width + buf.cur.x;
                    // Account for scroll offset (buffer_anchor.y)
                    let cursor_y = win.anchor.y + buf.cur.y.saturating_sub(win.buffer_anchor.y);
                    cursor_pos = Some((cursor_x, cursor_y));

                    // Collect leap rendering info if needed
                    if leap_state.is_showing_labels() {
                        leap_render_info =
                            Some((win.anchor.x + gutter_width, win.anchor.y, win.buffer_anchor.y));
                    }
                }
            }
        }

        // Render window separators for split windows
        self.render_window_separators(color_mode, theme)?;

        // Render leap labels after windows (avoids borrow conflict)
        if let Some((window_x, window_y, scroll_offset)) = leap_render_info {
            self.render_leap_labels(
                leap_state,
                window_x,
                window_y,
                scroll_offset,
                color_mode,
                theme,
            )?;
        }

        // Render completion popup if visible
        if completion_state.is_visible()
            && let Some((cursor_x, cursor_y)) = cursor_pos
        {
            self.render_completion_popup(completion_state, cursor_x, cursor_y, color_mode, theme)?;
        }

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

        // Render which-key panel overlay (after windows and status line)
        if which_key_panel.visible {
            let panel_lines =
                which_key_panel.render(self.size.width, self.size.height, color_mode, theme);
            for (line, x, y) in panel_lines {
                queue!(self.out_stream, MoveTo(x, y))?;
                queue!(self.out_stream, Print(line))?;
            }
        }

        // Render settings menu overlay (centered popup when active)
        if settings_menu.visible {
            let menu_lines = settings_menu.render(theme, color_mode);
            for (line, x, y) in menu_lines {
                queue!(self.out_stream, MoveTo(x, y))?;
                queue!(self.out_stream, Print(line))?;
            }
            // Position cursor at selected item
            let cursor_y = settings_menu.layout.y
                + 2
                + settings_menu
                    .selected_index
                    .saturating_sub(settings_menu.scroll_offset) as u16;
            let cursor_x = settings_menu.layout.x + 2;
            queue!(self.out_stream, MoveTo(cursor_x, cursor_y))?;
            queue!(self.out_stream, Show)?;
            return self.out_stream.flush();
        }

        // Render telescope overlay (takes over entire screen when active)
        if telescope_state.is_visible() {
            self.render_telescope(telescope_state, color_mode, theme)?;
            // Set cursor in telescope input
            let prompt_len = telescope_state.prompt.len() as u16;
            let cursor_x =
                telescope_state.layout.x + 1 + prompt_len + telescope_state.cursor_pos as u16;
            let cursor_y = telescope_state.layout.y + telescope_state.layout.height - 2;
            queue!(self.out_stream, MoveTo(cursor_x, cursor_y))?;
            queue!(self.out_stream, Show)?;
            return self.out_stream.flush();
        }

        // Position cursor at buffer cursor (not in command mode)
        if !mode.is_command()
            && let Some((x, y)) = cursor_pos
        {
            queue!(self.out_stream, MoveTo(x, y))?;
        }

        queue!(self.out_stream, Show)?;
        self.out_stream.flush()
    }

    /// Render the completion popup
    #[allow(clippy::cast_possible_truncation)]
    fn render_completion_popup(
        &mut self,
        state: &CompletionState,
        cursor_x: u16,
        cursor_y: u16,
        color_mode: ColorMode,
        theme: &Theme,
    ) -> std::result::Result<(), std::io::Error> {
        let items = &state.items;
        if items.is_empty() {
            return Ok(());
        }

        // Calculate popup dimensions
        let max_items = 10.min(items.len());
        let max_label_width = items
            .iter()
            .take(max_items)
            .map(|i| i.label.len())
            .max()
            .unwrap_or(10);
        let popup_width = (max_label_width + 2).min(40) as u16; // +2 for padding

        // Calculate popup position (below cursor by default)
        // Use cursor_x minus prefix length to start at word beginning
        let prefix_len = state.prefix.len() as u16;
        let popup_x = cursor_x.saturating_sub(prefix_len);
        let space_below = self.size.height.saturating_sub(cursor_y + 2); // -1 for cursor line, -1 for status
        let space_above = cursor_y;

        let (popup_y, render_above) = if space_below >= max_items as u16 {
            (cursor_y + 1, false)
        } else if space_above >= max_items as u16 {
            (cursor_y.saturating_sub(max_items as u16), true)
        } else {
            // Not enough space either way, show below with truncation
            (cursor_y + 1, false)
        };

        // Clamp popup_x to screen bounds
        let popup_x = popup_x.min(self.size.width.saturating_sub(popup_width));

        // Render each item (render_above is reserved for future use)
        let _ = render_above;
        let visible_items: Vec<_> = items.iter().take(max_items).collect();

        for (idx, item) in visible_items.iter().enumerate() {
            let is_selected = idx == state.selected_index;
            let style = if is_selected {
                &theme.popup.selected
            } else {
                &theme.popup.normal
            };

            let row = popup_y + idx as u16;

            // Skip if row is off screen or in status line
            if row >= self.size.height.saturating_sub(1) {
                break;
            }

            queue!(self.out_stream, MoveTo(popup_x, row))?;

            // Render styled item
            let ansi_start = style.to_ansi_start(color_mode);
            let label = if item.label.len() > popup_width as usize - 2 {
                format!(" {}.. ", &item.label[..popup_width as usize - 4])
            } else {
                format!(" {:width$} ", item.label, width = popup_width as usize - 2)
            };

            queue!(self.out_stream, Print(&ansi_start))?;
            queue!(self.out_stream, Print(&label))?;
            queue!(self.out_stream, Print(RESET_STYLE))?;
        }

        Ok(())
    }

    /// Render the telescope fuzzy finder overlay
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::too_many_lines)]
    fn render_telescope(
        &mut self,
        state: &TelescopeState,
        color_mode: ColorMode,
        theme: &Theme,
    ) -> std::result::Result<(), std::io::Error> {
        let layout = &state.layout;
        let x = layout.x;
        let y = layout.y;
        let width = layout.width;
        let height = layout.height;
        let preview_width = layout.preview_width;

        // Calculate total width including preview
        let total_width = preview_width.map_or(width, |pw| width + 1 + pw);

        // Draw border characters
        let border_style = theme.telescope.border.to_ansi_start(color_mode);

        // Top border with title
        queue!(self.out_stream, MoveTo(x, y))?;
        queue!(self.out_stream, Print(&border_style))?;
        let title = if state.title.is_empty() {
            format!(" {} ", state.picker_name)
        } else {
            format!(" {} ", state.title)
        };
        let title_len = title.len();
        let top_border = format!(
            "╭{}{}{}╮",
            &title,
            "─".repeat((total_width as usize).saturating_sub(title_len + 2)),
            ""
        );
        queue!(self.out_stream, Print(&top_border))?;
        queue!(self.out_stream, Print(RESET_STYLE))?;

        // Results area (items)
        let items_height = height.saturating_sub(4); // -4 for top border, bottom border, prompt line, separator
        let visible_items = state.visible_items();

        for row in 0..items_height {
            let screen_y = y + 1 + row;
            queue!(self.out_stream, MoveTo(x, screen_y))?;
            queue!(self.out_stream, Print(&border_style))?;
            queue!(self.out_stream, Print("│"))?;
            queue!(self.out_stream, Print(RESET_STYLE))?;

            let idx = row as usize;
            if idx < visible_items.len() {
                let item = &visible_items[idx];
                let absolute_idx = state.scroll_offset + idx;
                let is_selected = absolute_idx == state.selected_index;

                let style = if is_selected {
                    &theme.telescope.selected
                } else {
                    &theme.telescope.normal
                };

                let style_start = style.to_ansi_start(color_mode);
                let display = &item.display;

                // Truncate display if too long
                let max_display_len = (width as usize).saturating_sub(2);
                let display_str = if display.len() > max_display_len {
                    format!("{}..", &display[..max_display_len.saturating_sub(2)])
                } else {
                    format!("{display:max_display_len$}")
                };

                queue!(self.out_stream, Print(&style_start))?;
                queue!(self.out_stream, Print(&display_str))?;
                queue!(self.out_stream, Print(RESET_STYLE))?;
            } else {
                // Empty row
                let spaces = " ".repeat((width as usize).saturating_sub(2));
                queue!(self.out_stream, Print(&spaces))?;
            }

            // Right border of results
            queue!(self.out_stream, Print(&border_style))?;
            queue!(self.out_stream, Print("│"))?;
            queue!(self.out_stream, Print(RESET_STYLE))?;

            // Preview panel (if enabled)
            if let Some(pw) = preview_width {
                let preview_content = state.preview.as_ref();
                let line_idx = row as usize;

                if let Some(preview) = preview_content
                    && line_idx < preview.lines.len()
                {
                    let preview_line = &preview.lines[line_idx];
                    let is_highlight_line = preview.highlight_line == Some(line_idx);

                    let style = if is_highlight_line {
                        &theme.telescope.preview_highlight
                    } else {
                        &theme.telescope.preview
                    };

                    let style_start = style.to_ansi_start(color_mode);
                    let max_preview_len = (pw as usize).saturating_sub(1);
                    let preview_str = if preview_line.len() > max_preview_len {
                        format!("{}..", &preview_line[..max_preview_len.saturating_sub(2)])
                    } else {
                        format!("{preview_line:max_preview_len$}")
                    };

                    queue!(self.out_stream, Print(&style_start))?;
                    queue!(self.out_stream, Print(&preview_str))?;
                    queue!(self.out_stream, Print(RESET_STYLE))?;
                } else {
                    let spaces = " ".repeat((pw as usize).saturating_sub(1));
                    queue!(self.out_stream, Print(&spaces))?;
                }

                queue!(self.out_stream, Print(&border_style))?;
                queue!(self.out_stream, Print("│"))?;
                queue!(self.out_stream, Print(RESET_STYLE))?;
            }
        }

        // Separator line above prompt
        let sep_y = y + height - 3;
        queue!(self.out_stream, MoveTo(x, sep_y))?;
        queue!(self.out_stream, Print(&border_style))?;
        let separator = format!("├{}┤", "─".repeat((total_width as usize).saturating_sub(2)));
        queue!(self.out_stream, Print(&separator))?;
        queue!(self.out_stream, Print(RESET_STYLE))?;

        // Prompt line
        let prompt_y = y + height - 2;
        queue!(self.out_stream, MoveTo(x, prompt_y))?;
        queue!(self.out_stream, Print(&border_style))?;
        queue!(self.out_stream, Print("│"))?;
        queue!(self.out_stream, Print(RESET_STYLE))?;

        // Prompt and query
        let prompt_style = theme.telescope.prompt.to_ansi_start(color_mode);
        queue!(self.out_stream, Print(&prompt_style))?;
        queue!(self.out_stream, Print(&state.prompt))?;
        queue!(self.out_stream, Print(RESET_STYLE))?;

        let query_style = theme.telescope.input.to_ansi_start(color_mode);
        let query_max_len = (total_width as usize).saturating_sub(state.prompt.len() + 3);
        let query_display = if state.query.len() > query_max_len {
            &state.query[state.query.len() - query_max_len..]
        } else {
            &state.query
        };
        let query_padded = format!("{query_display:query_max_len$}");
        queue!(self.out_stream, Print(&query_style))?;
        queue!(self.out_stream, Print(&query_padded))?;
        queue!(self.out_stream, Print(RESET_STYLE))?;

        queue!(self.out_stream, Print(&border_style))?;
        queue!(self.out_stream, Print("│"))?;
        queue!(self.out_stream, Print(RESET_STYLE))?;

        // Bottom border with item count
        let bottom_y = y + height - 1;
        queue!(self.out_stream, MoveTo(x, bottom_y))?;
        queue!(self.out_stream, Print(&border_style))?;
        let item_count = format!(" {}/{} ", state.selected_index + 1, state.items.len());
        let count_len = item_count.len();
        let bottom_border = format!(
            "╰{}{}╯",
            "─".repeat((total_width as usize).saturating_sub(count_len + 2)),
            &item_count
        );
        queue!(self.out_stream, Print(&bottom_border))?;
        queue!(self.out_stream, Print(RESET_STYLE))?;

        Ok(())
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

    /// Toggle the explorer sidebar visibility and update window layouts
    pub fn toggle_explorer(&mut self) {
        self.layout.toggle_explorer();
        self.update_window_layouts();
    }

    /// Check if explorer is visible
    #[must_use]
    pub const fn is_explorer_visible(&self) -> bool {
        self.layout.is_explorer_visible()
    }

    /// Check if explorer is focused
    #[must_use]
    pub const fn is_explorer_focused(&self) -> bool {
        self.layout.is_explorer_focused()
    }

    /// Focus the explorer window
    pub const fn focus_explorer(&mut self) {
        self.layout.focus_explorer();
    }

    /// Focus the editor window
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

    /// Navigate focus to an adjacent window (including explorer as a window)
    ///
    /// Returns `Some(true)` if focus moved to explorer, `Some(false)` if moved from explorer to editor,
    /// `None` if navigation stayed within editor windows.
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
            let mut layouts = tab.calculate_layouts(editor_rect);

            // Include explorer as a window in navigation if visible
            if let Some(exp_layout) = self.layout.explorer_layout() {
                layouts.push(split::WindowLayout {
                    window_id: split::EXPLORER_WINDOW_ID,
                    rect: WindowRect::new(
                        exp_layout.anchor.x,
                        exp_layout.anchor.y,
                        exp_layout.width,
                        exp_layout.height,
                    ),
                });
            }

            // Determine current window ID (explorer or editor window)
            let current_id = if self.layout.is_explorer_focused() {
                split::EXPLORER_WINDOW_ID
            } else {
                tab.active_window_id
            };

            // Find adjacent window
            if let Some(next_id) = split::find_adjacent_window(current_id, direction, &layouts) {
                if next_id == split::EXPLORER_WINDOW_ID {
                    // Moving to explorer
                    self.focus_explorer();
                    self.update_window_active_state();
                    return Some(true);
                } else if current_id == split::EXPLORER_WINDOW_ID {
                    // Moving from explorer to editor window
                    self.focus_editor();
                    if let Some(tab_mut) = self.tab_manager.active_tab_mut() {
                        tab_mut.active_window_id = next_id;
                    }
                    self.update_window_active_state();
                    return Some(false);
                }
                // Moving between editor windows
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

    /// Render leap motion labels as overlays on match positions
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::similar_names)]
    fn render_leap_labels(
        &mut self,
        leap_state: &LeapState,
        window_x: u16,
        window_y: u16,
        scroll_offset: u16,
        _color_mode: ColorMode,
        theme: &Theme,
    ) -> std::result::Result<(), std::io::Error> {
        use reovim_sys::style::{
            Attribute, Color, SetAttribute, SetBackgroundColor, SetForegroundColor,
        };

        // Use leap theme style or fallback to search highlight
        let label_fg = theme.leap.label.fg.unwrap_or(Color::Black);
        let label_bg = theme.leap.label.bg.unwrap_or(Color::Yellow);

        for m in &leap_state.matches {
            // Calculate screen position
            let screen_line = m.line.saturating_sub(scroll_offset);
            let screen_x = window_x + m.col;
            let screen_y = window_y + screen_line;

            // Skip if off screen
            if screen_y >= self.size.height.saturating_sub(1) {
                continue;
            }

            // Move to position and render label with highlight
            queue!(self.out_stream, MoveTo(screen_x, screen_y))?;
            queue!(self.out_stream, SetForegroundColor(label_fg))?;
            queue!(self.out_stream, SetBackgroundColor(label_bg))?;
            queue!(self.out_stream, SetAttribute(Attribute::Bold))?;
            queue!(self.out_stream, Print(&m.label))?;
            queue!(self.out_stream, Print(RESET_STYLE))?;
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

    /// Render explorer sidebar to frame buffer
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::unused_self)]
    fn render_explorer_to_buffer(
        &self,
        buffer: &mut FrameBuffer,
        explorer: &ExplorerState,
        layout: layout::WindowLayout,
        theme: &Theme,
        _color_mode: ColorMode,
    ) {
        use crate::explorer::NodeType;

        // Get visible nodes from explorer
        let nodes = explorer.visible_nodes();
        let visible_count = layout
            .height
            .min(nodes.len().saturating_sub(explorer.scroll_offset) as u16);

        for row in 0..visible_count {
            let node_idx = explorer.scroll_offset + row as usize;
            if node_idx >= nodes.len() {
                break;
            }

            let node = nodes[node_idx];
            let is_selected = node_idx == explorer.cursor_index;

            // Use selection or base styles (no dedicated explorer theme)
            let style = if is_selected {
                &theme.selection.visual
            } else {
                &theme.base.default
            };

            let screen_y = layout.anchor.y + row;

            // Indent based on depth
            let indent = "  ".repeat(node.depth);
            let icon = match &node.node_type {
                NodeType::Directory { expanded, .. } => {
                    if *expanded {
                        "▼ "
                    } else {
                        "▶ "
                    }
                }
                NodeType::File { .. } | NodeType::Symlink { .. } => "  ",
            };

            let display = format!("{}{}{}", indent, icon, node.name);

            let mut col = layout.anchor.x;
            for ch in display.chars() {
                if col < layout.anchor.x + layout.width {
                    buffer.put_char(col, screen_y, ch, style);
                    col += 1;
                }
            }

            // Fill rest of line
            while col < layout.anchor.x + layout.width {
                buffer.put_char(col, screen_y, ' ', style);
                col += 1;
            }
        }

        // Fill empty rows with theme background
        for row in visible_count..layout.height {
            let screen_y = layout.anchor.y + row;
            for col in layout.anchor.x..layout.anchor.x + layout.width {
                buffer.put_char(col, screen_y, ' ', &theme.base.default);
            }
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

    /// Render leap labels to frame buffer
    #[allow(clippy::cast_possible_truncation)]
    fn render_leap_labels_to_buffer(
        &self,
        buffer: &mut FrameBuffer,
        leap_state: &LeapState,
        window_x: u16,
        window_y: u16,
        scroll_offset: u16,
        theme: &Theme,
    ) {
        let label_style = &theme.leap.label;

        for m in &leap_state.matches {
            let screen_line = m.line.saturating_sub(scroll_offset);
            let screen_x = window_x + m.col;
            let screen_y = window_y + screen_line;

            if screen_y >= self.size.height.saturating_sub(1) {
                continue;
            }

            for (i, ch) in m.label.chars().enumerate() {
                let x = screen_x + i as u16;
                if x < buffer.width() {
                    buffer.put_char(x, screen_y, ch, label_style);
                }
            }
        }
    }

    /// Render completion popup to frame buffer
    #[allow(clippy::cast_possible_truncation)]
    fn render_completion_to_buffer(
        &self,
        buffer: &mut FrameBuffer,
        state: &CompletionState,
        cursor_x: u16,
        cursor_y: u16,
        theme: &Theme,
    ) {
        let items = &state.items;
        if items.is_empty() {
            return;
        }

        let max_items = 10.min(items.len());
        let max_label_width = items
            .iter()
            .take(max_items)
            .map(|i| i.label.len())
            .max()
            .unwrap_or(10);
        let popup_width = (max_label_width + 2).min(40) as u16;

        let prefix_len = state.prefix.len() as u16;
        let popup_x = cursor_x
            .saturating_sub(prefix_len)
            .min(self.size.width.saturating_sub(popup_width));
        let popup_y = cursor_y + 1;

        for (idx, item) in items.iter().take(max_items).enumerate() {
            let is_selected = idx == state.selected_index;
            let style = if is_selected {
                &theme.popup.selected
            } else {
                &theme.popup.normal
            };

            let row = popup_y + idx as u16;
            if row >= self.size.height.saturating_sub(1) {
                break;
            }

            // Write item label
            buffer.put_char(popup_x, row, ' ', style);
            let label_chars: Vec<char> = item.label.chars().collect();
            for (i, &ch) in label_chars
                .iter()
                .take(popup_width as usize - 2)
                .enumerate()
            {
                buffer.put_char(popup_x + 1 + i as u16, row, ch, style);
            }
            // Pad to width
            for i in label_chars.len().min(popup_width as usize - 2)..popup_width as usize - 1 {
                buffer.put_char(popup_x + 1 + i as u16, row, ' ', style);
            }
            buffer.put_char(popup_x + popup_width - 1, row, ' ', style);
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

    /// Render which-key panel to frame buffer
    #[allow(clippy::cast_possible_truncation)]
    fn render_which_key_to_buffer(
        &self,
        buffer: &mut FrameBuffer,
        panel: &WhichKeyPanel,
        _color_mode: ColorMode,
        theme: &Theme,
    ) {
        use crate::overlay::OverlayGeometry;

        // Get panel bounds to fill background
        let bounds = panel.compute_bounds(self.size.width, self.size.height);
        let bg_style = &theme.whichkey.background;

        // Fill panel area with background
        for row in bounds.y..(bounds.y + bounds.height).min(buffer.height()) {
            for col in bounds.x..(bounds.x + bounds.width).min(buffer.width()) {
                buffer.put_char(col, row, ' ', bg_style);
            }
        }

        // Render the panel text on top
        let lines = panel.render(self.size.width, self.size.height, ColorMode::TrueColor, theme);
        for (line, x, y) in lines {
            // Strip ANSI codes from the pre-rendered line
            let stripped = crate::io::output::strip_ansi_codes(&line);
            for (i, ch) in stripped.chars().enumerate() {
                let col = x + i as u16;
                if col < buffer.width() && y < buffer.height() {
                    buffer.put_char(col, y, ch, &theme.whichkey.key);
                }
            }
        }
    }

    /// Render settings menu to frame buffer
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::unused_self)]
    fn render_settings_menu_to_buffer(
        &self,
        buffer: &mut FrameBuffer,
        settings: &crate::settings_menu::SettingsMenuState,
        theme: &Theme,
        _color_mode: ColorMode,
    ) {
        use crate::settings_menu::FlatItem;

        let layout = &settings.layout;
        let border_style = &theme.popup.border;

        // Top border with title
        buffer.put_char(layout.x, layout.y, '╭', border_style);
        let title = " Settings ";
        for (i, ch) in title.chars().enumerate() {
            buffer.put_char(layout.x + 1 + i as u16, layout.y, ch, border_style);
        }
        for x in (layout.x + 1 + title.len() as u16)..(layout.x + layout.width - 1) {
            buffer.put_char(x, layout.y, '─', border_style);
        }
        buffer.put_char(layout.x + layout.width - 1, layout.y, '╮', border_style);

        // Menu items
        for (row, item) in settings.flat_items.iter().enumerate() {
            let y = layout.y + 1 + row as u16;
            if y >= layout.y + layout.height - 1 {
                break;
            }

            let is_selected = row == settings.selected_index;
            let style = if is_selected {
                &theme.popup.selected
            } else {
                &theme.popup.normal
            };

            buffer.put_char(layout.x, y, '│', border_style);

            // Get display text based on item type
            let item_text = match item {
                FlatItem::SectionHeader(name) => format!(" [{name}] "),
                FlatItem::Setting {
                    section_idx,
                    item_idx,
                } => {
                    if let Some(section) = settings.sections.get(*section_idx)
                        && let Some(setting) = section.items.get(*item_idx)
                    {
                        format!("  {} ", setting.label)
                    } else {
                        "  ??? ".to_string()
                    }
                }
            };

            for (i, ch) in item_text.chars().enumerate() {
                let x = layout.x + 1 + i as u16;
                if x < layout.x + layout.width - 1 {
                    buffer.put_char(x, y, ch, style);
                }
            }

            // Fill rest
            for x in (layout.x + 1 + item_text.len() as u16)..(layout.x + layout.width - 1) {
                buffer.put_char(x, y, ' ', style);
            }

            buffer.put_char(layout.x + layout.width - 1, y, '│', border_style);
        }

        // Bottom border
        let bottom_y = layout.y + layout.height - 1;
        buffer.put_char(layout.x, bottom_y, '╰', border_style);
        for x in (layout.x + 1)..(layout.x + layout.width - 1) {
            buffer.put_char(x, bottom_y, '─', border_style);
        }
        buffer.put_char(layout.x + layout.width - 1, bottom_y, '╯', border_style);
    }

    /// Render telescope overlay to frame buffer
    #[allow(clippy::cast_possible_truncation)]
    #[allow(clippy::unused_self)]
    fn render_telescope_to_buffer(
        &self,
        buffer: &mut FrameBuffer,
        state: &TelescopeState,
        theme: &Theme,
        _color_mode: ColorMode,
    ) {
        let layout = &state.layout;
        let x = layout.x;
        let y = layout.y;
        let width = layout.width;
        let height = layout.height;

        let border_style = &theme.telescope.border;

        // Top border with title
        buffer.put_char(x, y, '╭', border_style);
        let title = format!(" {} ", state.picker_name);
        for (i, ch) in title.chars().enumerate() {
            let col = x + 1 + i as u16;
            if col < x + width - 1 {
                buffer.put_char(col, y, ch, border_style);
            }
        }
        for col in (x + 1 + title.len() as u16)..(x + width - 1) {
            buffer.put_char(col, y, '─', border_style);
        }
        buffer.put_char(x + width - 1, y, '╮', border_style);

        // Results area
        let items_height = height.saturating_sub(4);
        let visible_items = state.visible_items();

        for row in 0..items_height {
            let screen_y = y + 1 + row;
            buffer.put_char(x, screen_y, '│', border_style);

            let idx = row as usize;
            if idx < visible_items.len() {
                let item = &visible_items[idx];
                let absolute_idx = state.scroll_offset + idx;
                let is_selected = absolute_idx == state.selected_index;

                let style = if is_selected {
                    &theme.telescope.selected
                } else {
                    &theme.telescope.normal
                };

                let display = &item.display;
                for (i, ch) in display.chars().take(width as usize - 2).enumerate() {
                    buffer.put_char(x + 1 + i as u16, screen_y, ch, style);
                }
                // Fill rest
                for col in (x + 1 + display.len().min(width as usize - 2) as u16)..(x + width - 1) {
                    buffer.put_char(col, screen_y, ' ', style);
                }
            } else {
                // Empty row - use normal telescope style for consistent background
                for col in (x + 1)..(x + width - 1) {
                    buffer.put_char(col, screen_y, ' ', &theme.telescope.normal);
                }
            }

            buffer.put_char(x + width - 1, screen_y, '│', border_style);
        }

        // Separator
        let sep_y = y + height - 3;
        buffer.put_char(x, sep_y, '├', border_style);
        for col in (x + 1)..(x + width - 1) {
            buffer.put_char(col, sep_y, '─', border_style);
        }
        buffer.put_char(x + width - 1, sep_y, '┤', border_style);

        // Prompt line
        let prompt_y = y + height - 2;
        buffer.put_char(x, prompt_y, '│', border_style);

        let prompt_style = &theme.telescope.prompt;
        for (i, ch) in state.prompt.chars().enumerate() {
            buffer.put_char(x + 1 + i as u16, prompt_y, ch, prompt_style);
        }

        let query_style = &theme.telescope.input;
        let query_start = x + 1 + state.prompt.len() as u16;
        for (i, ch) in state.query.chars().enumerate() {
            let col = query_start + i as u16;
            if col < x + width - 1 {
                buffer.put_char(col, prompt_y, ch, query_style);
            }
        }

        // Fill rest of prompt line with input style for consistency
        for col in (query_start + state.query.len() as u16)..(x + width - 1) {
            buffer.put_char(col, prompt_y, ' ', query_style);
        }
        buffer.put_char(x + width - 1, prompt_y, '│', border_style);

        // Bottom border
        let bottom_y = y + height - 1;
        buffer.put_char(x, bottom_y, '╰', border_style);
        let count_text = format!(" {}/{} ", state.selected_index + 1, state.items.len());
        let count_start = x + width - 1 - count_text.len() as u16;
        for col in (x + 1)..count_start {
            buffer.put_char(col, bottom_y, '─', border_style);
        }
        for (i, ch) in count_text.chars().enumerate() {
            buffer.put_char(count_start + i as u16, bottom_y, ch, border_style);
        }
        buffer.put_char(x + width - 1, bottom_y, '╯', border_style);
    }
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_screen_resize_with_explorer() {
        // Create screen with a dummy writer
        let mut screen = Screen::with_writer(std::io::sink(), 100, 50);

        // Toggle explorer on
        screen.toggle_explorer();
        assert!(screen.is_explorer_visible());

        // Verify window anchor is at explorer width
        let explorer_width = screen.layout().explorer_width();
        assert!(!screen.windows.is_empty());
        assert_eq!(screen.windows[0].anchor.x, explorer_width);

        // Resize screen
        screen.resize(80, 40);

        // Window anchor should still be at explorer width
        assert!(!screen.windows.is_empty());
        assert_eq!(screen.windows[0].anchor.x, explorer_width);
        // Window width should be screen width minus explorer width
        assert_eq!(screen.windows[0].width, 80 - explorer_width);
    }

    #[test]
    fn test_screen_resize_without_explorer() {
        let mut screen = Screen::with_writer(std::io::sink(), 100, 50);

        // Explorer is not visible by default
        assert!(!screen.is_explorer_visible());

        // Window should start at x=0
        assert_eq!(screen.windows[0].anchor.x, 0);

        // Resize screen
        screen.resize(80, 40);

        // Window should still start at x=0
        assert_eq!(screen.windows[0].anchor.x, 0);
        // Window width should be full screen width
        assert_eq!(screen.windows[0].width, 80);
    }

    #[test]
    fn test_screen_resize_with_clamped_explorer() {
        let mut screen = Screen::with_writer(std::io::sink(), 100, 50);

        // Toggle explorer on (default width 30)
        screen.toggle_explorer();
        assert_eq!(screen.layout().explorer_width(), 30);

        // Resize to small screen (40 wide, max explorer is 20)
        screen.resize(40, 40);

        // Explorer width should be clamped
        let clamped_width = screen.layout().explorer_width();
        assert_eq!(clamped_width, 20);

        // Window anchor should use clamped width
        assert_eq!(screen.windows[0].anchor.x, clamped_width);
        assert_eq!(screen.windows[0].width, 20);
    }
}
