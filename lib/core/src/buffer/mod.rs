use crate::screen::Position;

#[derive(Clone, Debug)]
pub struct Line {
    pub inner: String,
}

impl From<&str> for Line {
    fn from(value: &str) -> Self {
        Line {
            inner: value.to_string(),
        }
    }
}

#[derive(Clone, Debug)]
pub struct Buffer {
    pub id: usize,
    pub cur: Position,
    pub contents: Vec<Line>,
}

impl Buffer {
    pub fn empty(id: usize) -> Self {
        Self {
            id,
            cur: Position { x: 0, y: 0 },
            contents: Vec::new(),
        }
    }

    pub fn set_content(&mut self, content: &str) {
        self.contents.clear();
        for line in content.to_string().lines() {
            let new_line = Line::from(line);
            self.contents.push(new_line);
        }
    }
}
