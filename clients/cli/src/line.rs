//! A line being edited, with history. No io: keys in, acts out.

use alloc::string::String;
use alloc::vec::Vec;

use reovim_client::Key;

use crate::Completion;

/// What a key did to the line.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Act {
    Nothing,
    /// The text or the cursor moved.
    Redraw,
    /// Enter: the line, recorded in history and cleared.
    Submit(String),
    /// Tab: the app asks `complete` and gives back `apply`.
    Complete,
    /// Ctrl-C or Ctrl-D.
    Quit,
}

/// The text, the cursor as a byte index on a char boundary, and the
/// lines submitted before. `at` is the history line being viewed.
#[derive(Debug, Default)]
pub struct Line {
    text: String,
    cursor: usize,
    history: Vec<String>,
    at: Option<usize>,
}

impl Line {
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// One key.
    pub fn key(&mut self, key: Key) -> Act {
        match key {
            Key::Char(c) => self.insert(c),
            Key::Enter => self.submit(),
            Key::Tab => Act::Complete,
            Key::Backspace => self.backspace(),
            Key::Left => self.left(),
            Key::Right => self.right(),
            Key::Up => self.recall(true),
            Key::Down => self.recall(false),
            Key::Home => self.go(0),
            Key::End => self.go(self.text.len()),
            Key::Ctrl('c' | 'd') => Act::Quit,
            Key::Ctrl(_) | Key::Esc => Act::Nothing,
        }
    }

    /// One candidate replaces the word and gets a space, unless it ends
    /// in `/`; several extend the word to their common prefix.
    pub fn apply(&mut self, completion: &Completion) {
        let Some(first) = completion.candidates.first() else { return };
        let mut word = common_prefix(&completion.candidates);
        if completion.candidates.len() == 1 && !first.ends_with('/') {
            word.push(' ');
        }
        self.text.truncate(completion.from);
        self.text.push_str(&word);
        self.cursor = self.text.len();
    }

    /// The line after `prompt`, and the cursor column.
    #[must_use]
    pub fn render(&self, prompt: &str) -> (String, u16) {
        let column =
            prompt.chars().count() + self.text[..self.cursor].chars().count();
        let mut line = String::from(prompt);
        line.push_str(&self.text);
        (line, u16::try_from(column).unwrap_or(u16::MAX))
    }

    fn insert(&mut self, c: char) -> Act {
        self.text.insert(self.cursor, c);
        self.cursor += c.len_utf8();
        Act::Redraw
    }

    fn submit(&mut self) -> Act {
        let text = core::mem::take(&mut self.text);
        self.cursor = 0;
        self.at = None;
        if !text.trim().is_empty() {
            self.history.push(text.clone());
        }
        Act::Submit(text)
    }

    fn backspace(&mut self) -> Act {
        let Some(c) = self.text[..self.cursor].chars().next_back() else {
            return Act::Nothing;
        };
        self.cursor -= c.len_utf8();
        self.text.remove(self.cursor);
        Act::Redraw
    }

    fn left(&mut self) -> Act {
        match self.text[..self.cursor].chars().next_back() {
            Some(c) => self.go(self.cursor - c.len_utf8()),
            None => Act::Nothing,
        }
    }

    fn right(&mut self) -> Act {
        match self.text[self.cursor..].chars().next() {
            Some(c) => self.go(self.cursor + c.len_utf8()),
            None => Act::Nothing,
        }
    }

    const fn go(&mut self, to: usize) -> Act {
        if to == self.cursor {
            return Act::Nothing;
        }
        self.cursor = to;
        Act::Redraw
    }

    /// Up walks back through history; Down forward, to an empty line
    /// past the newest.
    fn recall(&mut self, back: bool) -> Act {
        let last = self.history.len();
        let at = match (self.at, back) {
            (None, true) if last > 0 => last - 1,
            (Some(at), true) if at > 0 => at - 1,
            (Some(at), false) if at + 1 < last => at + 1,
            (Some(_), false) => {
                self.at = None;
                self.text.clear();
                self.cursor = 0;
                return Act::Redraw;
            }
            _ => return Act::Nothing,
        };
        self.at = Some(at);
        self.text.clone_from(&self.history[at]);
        self.cursor = self.text.len();
        Act::Redraw
    }
}

fn common_prefix(words: &[String]) -> String {
    let Some(first) = words.first() else { return String::new() };
    let len = first
        .char_indices()
        .map(|(at, c)| at + c.len_utf8())
        .take_while(|&end| words.iter().all(|w| w.starts_with(&first[..end])))
        .last()
        .unwrap_or(0);
    first[..len].into()
}
