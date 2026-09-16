//! The command line: `:` and what a person types after it.

use alloc::string::String;

use crate::{Effect, Key, Mode, Quit, Vim};

impl Vim {
    /// One key on the command line: it grows, shrinks, is left, or
    /// runs.
    pub(crate) fn command(&mut self, key: Key) -> Effect {
        match key {
            Key::Char(c) => self.line.push(c),
            Key::Backspace if self.line.is_empty() => self.mode = Mode::Normal,
            Key::Backspace => {
                self.line.pop();
            }
            Key::Esc => self.leave_command(),
            Key::Enter => return self.run(),
            _ => return Effect::Nothing,
        }
        Effect::Moved
    }

    fn leave_command(&mut self) {
        self.mode = Mode::Normal;
        self.line.clear();
    }

    /// The line as a command: `w`, `q`, `q!`, `wq`, `x`, `cq`; any
    /// other is E492.
    fn run(&mut self) -> Effect {
        let line = core::mem::take(&mut self.line);
        self.mode = Mode::Normal;
        match line.trim() {
            "" => Effect::Moved,
            "w" => Effect::Save,
            "q" => Effect::Quit(Quit::Ask),
            "q!" => Effect::Quit(Quit::Force),
            "wq" | "x" => Effect::Quit(Quit::Write),
            "cq" => Effect::Quit(Quit::Abort),
            other => {
                self.message =
                    alloc::format!("E492: Not an editor command: {other}");
                Effect::Moved
            }
        }
    }

    /// A message shown in the status row until the next key, as
    /// Vim's own are: the tui's E37, for one.
    pub fn notice(&mut self, message: &str) {
        self.message = String::from(message);
    }
}
