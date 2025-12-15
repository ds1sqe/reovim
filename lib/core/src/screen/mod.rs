use {
    crate::{
        buffer::Buffer,
        command::terminal::{Clear, ClearType},
        command_line::CommandLine,
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

pub mod cusor;
pub mod window;

pub struct ScreenSize {
    pub height: u16,
    pub width: u16,
}
#[derive(Clone, Debug, Default)]
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

    /// Render the status line showing current mode
    pub fn render_status_line(
        &mut self,
        mode: &Mod,
    ) -> std::result::Result<(), std::io::Error> {
        let mode_str = match mode {
            Mod::Normal => "-- NORMAL --",
            Mod::Insert(_) => "-- INSERT --",
            Mod::Visual(_) => "-- VISUAL --",
            Mod::Command => "", // Command mode shows command line instead
        };
        let status_row = self.size.height.saturating_sub(1);
        queue!(self.out_stream, MoveTo(0, status_row))?;
        queue!(self.out_stream, Print(mode_str))
    }

    /// Render the command line input (shown in Command mode)
    pub fn render_command_line(
        &mut self,
        cmd_line: &CommandLine,
    ) -> std::result::Result<(), std::io::Error> {
        let status_row = self.size.height.saturating_sub(1);
        queue!(self.out_stream, MoveTo(0, status_row))?;
        let display = format!(":{}", cmd_line.input);
        queue!(self.out_stream, Print(display))?;
        // Position cursor after the input
        let cursor_col = 1 + cmd_line.cursor as u16;
        queue!(self.out_stream, MoveTo(cursor_col, status_row))
    }

    /// update screen
    pub fn render(
        &mut self,
        buffers: &[Buffer],
        mode: &Mod,
        cmd_line: &CommandLine,
    ) -> std::result::Result<(), std::io::Error> {
        // Reset all styling before clearing
        queue!(self.out_stream, Print("\x1b[0m"))?;
        self.clear(ClearType::All)?;

        // Track cursor position from main buffer
        let mut cursor_pos: Option<(u16, u16)> = None;

        for (_wid, win) in self.windows.iter().enumerate() {
            match buffers.get(win.buffer_id) {
                Some(buf) => {
                    queue!(
                        self.out_stream,
                        MoveTo(win.anchor.x, win.anchor.y)
                    )?;
                    let content = win.render(buf);
                    queue!(self.out_stream, Print(content))?;

                    // Calculate cursor position relative to window
                    let cursor_x = win.anchor.x + buf.cur.x;
                    let cursor_y = win.anchor.y + buf.cur.y;
                    cursor_pos = Some((cursor_x, cursor_y));
                }
                None => {
                    // TODO: handle buffer not found
                }
            }
        }

        // Show command line in Command mode, status line otherwise
        if matches!(mode, Mod::Command) {
            self.render_command_line(cmd_line)?;
        } else {
            self.render_status_line(mode)?;
            // Position cursor at buffer cursor (not in command mode)
            if let Some((x, y)) = cursor_pos {
                queue!(self.out_stream, MoveTo(x, y))?;
            }
        }
        Ok(())
    }
}
