use crate::buffer::Buffer;

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

    pub buffer_id: usize,
    // where this buffer's top left positioned is
    pub buffer_anchor: Anchor,
    pub line_number: LineNumber,
}

pub enum LineNumberMode {
    Absolute,
    Relative,
    Hybrid,
}

pub struct LineNumber {
    show: bool,
    mode: LineNumberMode,
}

impl Default for LineNumber {
    fn default() -> Self {
        LineNumber {
            show: false,
            mode: LineNumberMode::Absolute,
        }
    }
}

impl Window {
    pub fn render(&self, buf: &Buffer) -> String {
        let mut out = String::new();

        for row in self.buffer_anchor.y..(self.height + self.buffer_anchor.y) {
            let line_content = buf.contents.get(row as usize);
            let line_out = match line_content {
                Some(content) => {
                    let head = if self.line_number.show {
                        match self.line_number.mode {
                            // TODO:  align line number
                            LineNumberMode::Absolute => format!("{row}"),
                            LineNumberMode::Relative => {
                                format!("{}", (row - buf.cur.y))
                            }
                            LineNumberMode::Hybrid => {
                                if row != buf.cur.y {
                                    format!("{row}")
                                } else {
                                    format!("{}", (row - buf.cur.y))
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

    // TODO: split
}
