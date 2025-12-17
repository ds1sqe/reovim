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
        explorer::{render_explorer, ExplorerState},
        folding::FoldManager,
        highlight::{ColorMode, HighlightStore, Theme},
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
    std::collections::BTreeMap,
    std::io::{self, Write},
    window::{Anchor, LineNumber, Window},
};

pub use status_line::{render_command_line_to, render_status_line_to, StatusLineRenderer};
pub use which_key::{WhichKeyConfig, WhichKeyPanel};

pub mod cusor;
pub mod layout;
pub mod window;

pub use layout::{LayoutManager, WindowType};

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
}

impl Default for Screen {
    fn default() -> Self {
        let stdout = io::stdout();
        let (columns, rows) =
            size().expect("failed to get screen size on screen creation");
        let mut windows = Vec::new();
        let anchor = Anchor { x: 0, y: 0 };
        let editor_height = rows.saturating_sub(1); // Reserve last row for status line

        windows.push(Window {
            id: 0,
            window_type: WindowType::Editor,
            anchor,
            width: columns,
            height: editor_height,
            buffer_anchor: anchor,
            buffer_id: 0,
            line_number: LineNumber::default(),
        });

        let layout = LayoutManager::new(columns, editor_height);

        Self {
            size: ScreenSize {
                width: columns,
                height: rows,
            },
            out_stream: Box::new(stdout),
            windows,
            layout,
        }
    }
}

impl Screen {
    /// Create a new Screen with a custom writer (useful for testing/benchmarking)
    #[must_use]
    pub fn with_writer<W: Write + 'static>(writer: W, width: u16, height: u16) -> Self {
        let anchor = Anchor { x: 0, y: 0 };
        let editor_height = height.saturating_sub(1);

        let windows = vec![Window {
            id: 0,
            window_type: WindowType::Editor,
            anchor,
            width,
            height: editor_height,
            buffer_anchor: anchor,
            buffer_id: 0,
            line_number: LineNumber::default(),
        }];

        let layout = LayoutManager::new(width, editor_height);

        Self {
            size: ScreenSize { height, width },
            out_stream: Box::new(writer),
            windows,
            layout,
        }
    }

    pub fn clear(
        &mut self,
        ctype: ClearType,
    ) -> std::result::Result<(), std::io::Error> {
        queue!(self.out_stream, Clear(ctype))
    }

    pub fn flush(&mut self) -> std::result::Result<(), std::io::Error> {
        self.out_stream.flush()
    }

    pub fn enable_mouse_capture(
        &mut self,
    ) -> std::result::Result<(), std::io::Error> {
        queue!(self.out_stream, EnableMouseCapture)
    }

    pub fn disable_mouse_capture(
        &mut self,
    ) -> std::result::Result<(), std::io::Error> {
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
    ) -> std::result::Result<(), std::io::Error> {
        // Reset all styling before clearing
        queue!(self.out_stream, Print(RESET_STYLE))?;
        self.clear(ClearType::All)?;

        // Track cursor position from main buffer
        let mut cursor_pos: Option<(u16, u16)> = None;
        let mut current_buffer: Option<&Buffer> = None;

        // Render explorer sidebar if visible
        if self.layout.is_explorer_visible()
            && let Some(explorer) = explorer_state
            && let Some(layout) = self.layout.explorer_layout()
        {
            let lines = render_explorer(explorer, layout.height, theme, color_mode);
            for (row_offset, line) in lines.iter().enumerate() {
                queue!(
                    self.out_stream,
                    MoveTo(layout.anchor.x, layout.anchor.y + row_offset as u16)
                )?;
                queue!(self.out_stream, Print(line))?;
            }

            // If explorer is focused, set cursor position in explorer
            if self.layout.is_explorer_focused() {
                let cursor_y = explorer.cursor_index.saturating_sub(explorer.scroll_offset);
                cursor_pos = Some((layout.anchor.x, layout.anchor.y + cursor_y as u16));
            }
        }

        // Collect leap rendering info before iterating windows
        let mut leap_render_info: Option<(u16, u16, u16)> = None;

        // Render editor windows
        for win in &self.windows {
            if let Some(buf) = buffers.get(&win.buffer_id) {
                current_buffer = Some(buf);

                // Get fold state for this buffer
                let fold_state = fold_manager.get(buf.id);

                // Render each line with explicit cursor positioning
                let lines = win.render(buf, highlight_store, color_mode, theme, fold_state);
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
                    let cursor_y = win.anchor.y + buf.cur.y;
                    cursor_pos = Some((cursor_x, cursor_y));

                    // Collect leap rendering info if needed
                    if leap_state.is_showing_labels() {
                        leap_render_info = Some((win.anchor.x + gutter_width, win.anchor.y, win.buffer_anchor.y));
                    }
                }
            }
        }

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
            self.render_completion_popup(
                completion_state,
                cursor_x,
                cursor_y,
                color_mode,
                theme,
            )?;
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
            let panel_lines = which_key_panel.render(
                self.size.width,
                self.size.height,
                color_mode,
                theme,
            );
            for (line, x, y) in panel_lines {
                queue!(self.out_stream, MoveTo(x, y))?;
                queue!(self.out_stream, Print(line))?;
            }
        }

        // Render telescope overlay (takes over entire screen when active)
        if telescope_state.is_visible() {
            self.render_telescope(telescope_state, color_mode, theme)?;
            // Set cursor in telescope input
            let prompt_len = telescope_state.prompt.len() as u16;
            let cursor_x = telescope_state.layout.x + 1 + prompt_len + telescope_state.cursor_pos as u16;
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
                &theme.popup_selected
            } else {
                &theme.popup_normal
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
        let border_style = theme.telescope_border.to_ansi_start(color_mode);

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
                    &theme.telescope_selected
                } else {
                    &theme.telescope_normal
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
                        &theme.telescope_preview_highlight
                    } else {
                        &theme.telescope_preview
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
        let separator = format!(
            "├{}┤",
            "─".repeat((total_width as usize).saturating_sub(2))
        );
        queue!(self.out_stream, Print(&separator))?;
        queue!(self.out_stream, Print(RESET_STYLE))?;

        // Prompt line
        let prompt_y = y + height - 2;
        queue!(self.out_stream, MoveTo(x, prompt_y))?;
        queue!(self.out_stream, Print(&border_style))?;
        queue!(self.out_stream, Print("│"))?;
        queue!(self.out_stream, Print(RESET_STYLE))?;

        // Prompt and query
        let prompt_style = theme.telescope_prompt.to_ansi_start(color_mode);
        queue!(self.out_stream, Print(&prompt_style))?;
        queue!(self.out_stream, Print(&state.prompt))?;
        queue!(self.out_stream, Print(RESET_STYLE))?;

        let query_style = theme.telescope_input.to_ansi_start(color_mode);
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

    /// Set the buffer ID for the editor window
    pub fn set_editor_buffer(&mut self, buffer_id: usize) {
        for win in &mut self.windows {
            if win.window_type == WindowType::Editor {
                win.buffer_id = buffer_id;
                break;
            }
        }
    }

    /// Update window layouts based on current layout manager state
    fn update_window_layouts(&mut self) {
        let editor_layout = self.layout.editor_layout();

        // Find and update the editor window
        for win in &mut self.windows {
            if win.window_type == WindowType::Editor {
                win.anchor = editor_layout.anchor;
                win.width = editor_layout.width;
                win.height = editor_layout.height;
            }
        }
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
        use reovim_sys::style::{Attribute, Color, SetAttribute, SetBackgroundColor, SetForegroundColor};

        // Use leap theme style or fallback to search highlight
        let label_fg = theme.leap_label.fg.unwrap_or(Color::Black);
        let label_bg = theme.leap_label.bg.unwrap_or(Color::Yellow);

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
