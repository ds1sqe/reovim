use {
    crate::{
        buffer::Buffer,
        command::terminal::{Clear, ClearType},
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
#[derive(Clone, Debug)]
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
            width: rows,
            height: columns,
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

    /// update screen?
    ///
    pub fn render(
        &mut self,
        buffers: &[Buffer],
    ) -> std::result::Result<(), std::io::Error> {
        self.clear(ClearType::All)?;
        for (wid, win) in self.windows.iter().enumerate() {
            match buffers.get(win.buffer_id) {
                Some(buf) => {
                    queue!(
                        self.out_stream,
                        MoveTo(win.anchor.x, win.anchor.y)
                    )?;
                    let content = win.render(buf);
                    queue!(self.out_stream, Print(content))?;
                }
                None => {
                    // TODO: handle buffer not found
                }
            }
        }
        Ok(())
    }
}
