#![allow(clippy::missing_errors_doc)]

mod status_line;
mod which_key;

use {
    crate::{
        buffer::Buffer,
        command::terminal::{Clear, ClearType},
        command_line::CommandLine,
        completion::CompletionState,
        constants::RESET_STYLE,
        explorer::{ExplorerState, render_explorer},
        folding::FoldManager,
        highlight::{ColorMode, HighlightStore, Theme},
        indent::IndentAnalyzer,
        leap::LeapState,
        modd::ModeState,
        telescope::TelescopeState,
    },
    reovim_sys::{
        cursor::MoveTo,
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
    }

    /// update screen
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
        // Reset all styling before clearing
        queue!(self.out_stream, Print(RESET_STYLE))?;
        self.clear(ClearType::All)?;

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
            return Ok(());
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
            return Ok(());
        }

        // Position cursor at buffer cursor (not in command mode)
        if !mode.is_command()
            && let Some((x, y)) = cursor_pos
        {
            queue!(self.out_stream, MoveTo(x, y))?;
        }

        Ok(())
    }

    /// Render only the telescope overlay without full screen clear.
    /// Used for telescope-internal updates to avoid flicker.
    #[allow(clippy::cast_possible_truncation)]
    pub fn render_telescope_only(
        &mut self,
        telescope_state: &TelescopeState,
        color_mode: ColorMode,
        theme: &Theme,
    ) -> std::result::Result<(), std::io::Error> {
        if !telescope_state.is_visible() {
            return Ok(());
        }

        self.render_telescope(telescope_state, color_mode, theme)?;

        // Set cursor in telescope input
        let prompt_len = telescope_state.prompt.len() as u16;
        let cursor_x =
            telescope_state.layout.x + 1 + prompt_len + telescope_state.cursor_pos as u16;
        let cursor_y = telescope_state.layout.y + telescope_state.layout.height - 2;
        queue!(self.out_stream, MoveTo(cursor_x, cursor_y))?;

        Ok(())
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

            // Rebuild the windows vec from split tree layouts
            self.windows.clear();
            for layout in &layouts {
                let buffer_id = self
                    .window_buffers
                    .get(&layout.window_id)
                    .copied()
                    .unwrap_or(0);
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
                    return Some(true);
                } else if current_id == split::EXPLORER_WINDOW_ID {
                    // Moving from explorer to editor window
                    self.focus_editor();
                    if let Some(tab_mut) = self.tab_manager.active_tab_mut() {
                        tab_mut.active_window_id = next_id;
                    }
                    return Some(false);
                }
                // Moving between editor windows
                if let Some(tab_mut) = self.tab_manager.active_tab_mut() {
                    tab_mut.active_window_id = next_id;
                }
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
