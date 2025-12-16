#![allow(clippy::missing_errors_doc)]

mod status_line;

use {
    crate::{
        buffer::Buffer,
        command::terminal::{Clear, ClearType},
        command_line::CommandLine,
        constants::RESET_STYLE,
        highlight::HighlightStore,
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
pub mod window;

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
}

impl Default for Screen {
    fn default() -> Self {
        let stdout = io::stdout();
        let (columns, rows) =
            size().expect("failed to get screen size on screen creation");
        let mut windows = Vec::new();
        let anchor = Anchor { x: 0, y: 0 };

        windows.push(Window {
            anchor,
            width: columns,
            height: rows.saturating_sub(1), // Reserve last row for status line
            buffer_anchor: anchor,
            buffer_id: 0,
            line_number: LineNumber::default(),
        });

        Self {
            size: ScreenSize {
                width: columns,
                height: rows,
            },
            out_stream: Box::new(stdout),
            windows,
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
    pub fn render(
        &mut self,
        buffers: &[Buffer],
        highlight_store: &HighlightStore,
        mode: &Mod,
        cmd_line: &CommandLine,
        pending_keys: &str,
        last_command: &str,
    ) -> std::result::Result<(), std::io::Error> {
        // Reset all styling before clearing
        queue!(self.out_stream, Print(RESET_STYLE))?;
        self.clear(ClearType::All)?;

        // Track cursor position from main buffer
        let mut cursor_pos: Option<(u16, u16)> = None;
        let mut current_buffer: Option<&Buffer> = None;

        for win in &self.windows {
            if let Some(buf) = buffers.get(win.buffer_id) {
                current_buffer = Some(buf);

                // Render each line with explicit cursor positioning
                let lines = win.render(buf, highlight_store);
                for (row_offset, line) in lines.iter().enumerate() {
                    queue!(
                        self.out_stream,
                        MoveTo(win.anchor.x, win.anchor.y + row_offset as u16)
                    )?;
                    queue!(self.out_stream, Print(line))?;
                }

                // Calculate cursor position relative to window
                // Account for line number gutter width
                let gutter_width = win.line_number_width(buf.contents.len());
                let cursor_x = win.anchor.x + gutter_width + buf.cur.x;
                let cursor_y = win.anchor.y + buf.cur.y;
                cursor_pos = Some((cursor_x, cursor_y));
            }
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
