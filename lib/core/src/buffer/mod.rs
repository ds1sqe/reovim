use std::default;

use crate::api::screen::{Position, ScreenSize};

pub enum BufferState {
    Open,
    Hided,
}

/// represent top left corner
#[derive(Clone, Copy)]
pub struct Anchor {
    pub x: u16,
    pub y: u16,
}

/// window is a intermediate object between buffer and screen
pub struct Window {
    // where this window's of top left positioned on the screen is
    pub anchor: Anchor,
    pub width: u16,
    pub height: u16,
    pub buffer: Buffer,
    // where this buffer's top left positioned is
    pub buffer_anchor: Anchor,
}
pub struct Line {
    inner: String,
}
impl From<&str> for Line {
    fn from(value: &str) -> Self {
        Line {
            inner: value.to_string(),
        }
    }
}

pub struct Buffer {
    id: usize,
    cur: Position,
    state: BufferState,
    contents: Vec<Line>,
    line: BufferLine,
}

pub enum BufferLineMode {
    Absolute,
    Relative,
    Hybrid,
}

pub struct BufferLine {
    show: bool,
    modd: BufferLineMode,
}

impl Default for BufferLine {
    fn default() -> Self {
        BufferLine {
            show: false,
            modd: BufferLineMode::Absolute,
        }
    }
}

impl Buffer {
    pub fn empty(id: usize) -> Self {
        Self {
            id,
            cur: Position { x: 0, y: 0 },
            state: BufferState::Open,
            contents: Vec::new(),
            line: BufferLine::default(),
        }
    }
    pub fn render(&self, window: &Window) -> String {
        let mut out = String::new();

        for row in
            window.buffer_anchor.y..(window.height + window.buffer_anchor.y)
        {
            let line_content = self.contents.get(row as usize);
            let line_out = match line_content {
                Some(content) => {
                    let head = if self.line.show {
                        match self.line.modd {
                            // TODO:  align line number
                            BufferLineMode::Absolute => format!("{row}"),
                            BufferLineMode::Relative => {
                                format!("{}", (row - self.cur.y))
                            }
                            BufferLineMode::Hybrid => {
                                if row != self.cur.y {
                                    format!("{row}")
                                } else {
                                    format!("{}", (row - self.cur.y))
                                }
                            }
                        }
                    } else {
                        "".to_string()
                    };
                    head + &content.inner
                }
                None => "".to_string(),
            };
            out.push_str(&line_out);
        }
        out
    }

    pub fn set_content(&mut self, content: &str) {
        self.contents.clear();
        for line in content.to_string().lines() {
            let new_line = Line::from(line);
            self.contents.push(new_line);
        }
    }
}
