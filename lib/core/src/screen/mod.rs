#![allow(clippy::missing_errors_doc)]

mod status_line;

use {
    crate::{
        buffer::Buffer,
        command::terminal::{Clear, ClearType},
        command_line::CommandLine,
        completion::CompletionState,
        constants::RESET_STYLE,
        explorer::{render_explorer, ExplorerState},
        highlight::{ColorMode, HighlightStore, Theme},
        modd::Mod,
    },
    reovim_sys::{
        cursor::MoveTo,
        event::{DisableMouseCapture, EnableMouseCapture},
        queue,
        style::Print,
        terminal::size,
    },
    std::io::{self, Write},
    window::{Anchor, LineNumber, Window},
};

pub use status_line::{render_command_line_to, render_status_line_to, StatusLineRenderer};

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
    pub fn render(
        &mut self,
        buffers: &[Buffer],
        highlight_store: &HighlightStore,
        mode: &Mod,
        cmd_line: &CommandLine,
        pending_keys: &str,
        last_command: &str,
        color_mode: ColorMode,
        theme: &Theme,
        explorer_state: Option<&ExplorerState>,
        completion_state: &CompletionState,
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

        // Render editor windows
        for win in &self.windows {
            if let Some(buf) = buffers.get(win.buffer_id) {
                current_buffer = Some(buf);

                // Render each line with explicit cursor positioning
                let lines = win.render(buf, highlight_store, color_mode, theme);
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
                }
            }
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
        if matches!(mode, Mod::Command) {
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
            )?;
            // Position cursor at buffer cursor (not in command mode)
            if let Some((x, y)) = cursor_pos {
                queue!(self.out_stream, MoveTo(x, y))?;
            }
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
}

// Implement StatusLineRenderer trait for Screen
impl StatusLineRenderer for Screen {
    fn render_status_line(
        &mut self,
        mode: &Mod,
        buffer: Option<&Buffer>,
        pending_keys: &str,
        last_command: &str,
    ) -> std::result::Result<(), std::io::Error> {
        render_status_line_to(
            &mut self.out_stream,
            self.size.width,
            self.size.height,
            mode,
            buffer,
            pending_keys,
            last_command,
        )
    }

    fn render_command_line(
        &mut self,
        cmd_line: &CommandLine,
    ) -> std::result::Result<(), std::io::Error> {
        render_command_line_to(&mut self.out_stream, self.size.height, cmd_line)
    }
}
