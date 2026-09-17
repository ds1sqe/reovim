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
    /// Ctrl-L: the screen is the app's, and the line is drawn again at
    /// the top of it.
    Clear,
    /// Ctrl-C, or Ctrl-D on an empty line.
    Quit,
}

/// A line being edited, and the lines submitted before it.
///
/// `cursor` is a byte index on a char boundary, `at` the history line
/// being viewed, and `seek` what was typed before the first Up, which
/// every later Up and Down looks for at the start of a line.
#[derive(Debug, Default)]
pub struct Line {
    text: String,
    cursor: usize,
    history: Vec<String>,
    at: Option<usize>,
    seek: Option<String>,
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
            Key::Home | Key::Ctrl('a') => self.go(0),
            Key::End | Key::Ctrl('e') => self.go(self.text.len()),
            Key::Ctrl('u') => self.cut(0),
            Key::Ctrl('w') => self.cut(word(&self.text[..self.cursor])),
            Key::Ctrl('l') => Act::Clear,
            Key::Ctrl('c') => Act::Quit,
            // As a shell reads it: the end of the input on an empty
            // line, the character under the cursor on any other.
            Key::Ctrl('d') if self.text.is_empty() => Act::Quit,
            Key::Ctrl('d') => self.delete(),
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
        self.edited();
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

    /// The text is the typed one again: the next Up searches for what
    /// stands in the line now, not for what a walk began with.
    fn edited(&mut self) {
        self.at = None;
        self.seek = None;
    }

    fn insert(&mut self, c: char) -> Act {
        self.text.insert(self.cursor, c);
        self.cursor += c.len_utf8();
        self.edited();
        Act::Redraw
    }

    fn submit(&mut self) -> Act {
        let text = core::mem::take(&mut self.text);
        self.cursor = 0;
        self.edited();
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
        self.edited();
        Act::Redraw
    }

    /// Ctrl-D on a line with something in it: the char under the
    /// cursor, and nothing at the end of the line.
    fn delete(&mut self) -> Act {
        if self.text[self.cursor..].chars().next().is_none() {
            return Act::Nothing;
        }
        self.text.remove(self.cursor);
        self.edited();
        Act::Redraw
    }

    /// Ctrl-U and Ctrl-W: what stands between `from` and the cursor
    /// goes, and the cursor with it.
    fn cut(&mut self, from: usize) -> Act {
        if from == self.cursor {
            return Act::Nothing;
        }
        self.text.replace_range(from..self.cursor, "");
        self.cursor = from;
        self.edited();
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

    /// Up walks back through the lines that begin as the line did when
    /// the walk started, Down forward through them, to that beginning
    /// again past the newest. An empty line walks them all.
    fn recall(&mut self, back: bool) -> Act {
        let seek = self.seek.clone().unwrap_or_else(|| self.text.clone());
        self.seek = Some(seek.clone());
        let starts = |at: &usize| self.history[*at].starts_with(&seek);
        let last = self.history.len();
        let found = match (self.at, back) {
            (None, true) => (0..last).rev().find(starts),
            (Some(at), true) => (0..at).rev().find(starts),
            (Some(at), false) => (at + 1..last).find(starts),
            (None, false) => None,
        };
        if let Some(at) = found {
            self.at = Some(at);
            self.text.clone_from(&self.history[at]);
            self.cursor = self.text.len();
            return Act::Redraw;
        }
        // Past the newest of them is what was typed before the walk;
        // older than the oldest is where the line already stands.
        let walking = self.at.is_some();
        if walking && !back {
            self.at = None;
            self.seek = None;
            self.text = seek;
            self.cursor = self.text.len();
            return Act::Redraw;
        }
        // A walk that never started forgets what it was looking for.
        if !walking {
            self.seek = None;
        }
        Act::Nothing
    }
}

/// Where the last word of `text` begins, `text` being what stands
/// before the cursor: the spaces just before the cursor are part of
/// what Ctrl-W cuts, the ones before that word are not.
fn word(text: &str) -> usize {
    let end = text.trim_end_matches(' ').len();
    text[..end].rfind(' ').map_or(0, |at| at + 1)
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
