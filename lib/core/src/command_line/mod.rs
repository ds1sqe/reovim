mod ex_command;

pub use ex_command::{ExCommand, SetOption};

/// State for command-line mode input
#[derive(Clone, Debug, Default)]
pub struct CommandLine {
    /// Current command input buffer
    pub input: String,
    /// Cursor position within input
    pub cursor: usize,
    /// Whether command line is active
    pub active: bool,
}

impl CommandLine {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn activate(&mut self) {
        self.active = true;
        self.input.clear();
        self.cursor = 0;
    }

    pub fn insert_char(&mut self, c: char) {
        self.input.insert(self.cursor, c);
        self.cursor += 1;
    }

    pub fn delete_char(&mut self) {
        if self.cursor > 0 {
            self.cursor -= 1;
            self.input.remove(self.cursor);
        }
    }

    pub fn clear(&mut self) {
        self.input.clear();
        self.cursor = 0;
        self.active = false;
    }

    #[must_use]
    pub fn execute(&self) -> Option<ExCommand> {
        ExCommand::parse(&self.input)
    }
}
