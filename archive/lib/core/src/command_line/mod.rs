mod ex_command;
pub mod registry;

pub use {
    ex_command::{ExCommand, SetOption},
    registry::{ExCommandHandler, ExCommandRegistry},
};

/// Context for command-line completion
#[derive(Debug, Clone)]
pub enum CmdlineCompletionContext {
    /// Completing command name (e.g., `:wri` -> `:write`)
    Command {
        /// The prefix typed so far
        prefix: String,
    },
    /// Completing argument (e.g., `:e src/` -> path completion)
    Argument {
        /// The command being completed (e.g., "e", "edit")
        command: String,
        /// The prefix of the argument typed so far
        prefix: String,
        /// Position in input where the argument starts
        arg_start: usize,
    },
}

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

    /// Get the completion context based on current input and cursor position
    ///
    /// Determines whether we're completing a command name or an argument.
    #[must_use]
    pub fn completion_context(&self) -> CmdlineCompletionContext {
        let input = &self.input;
        let cursor = self.cursor;

        // Find the first space in the input
        let first_space = input.find(' ');

        match first_space {
            None => {
                // No space yet - completing command name
                CmdlineCompletionContext::Command {
                    prefix: input[..cursor].to_string(),
                }
            }
            Some(space_pos) if cursor <= space_pos => {
                // Cursor is before or at the first space - completing command name
                CmdlineCompletionContext::Command {
                    prefix: input[..cursor].to_string(),
                }
            }
            Some(_) => {
                // Cursor is after the first space - completing argument
                let command = input.split_whitespace().next().unwrap_or("").to_string();
                // Find where the current word starts
                let arg_start = input[..cursor]
                    .rfind(|c: char| c.is_whitespace())
                    .map_or(0, |p| p + 1);
                let prefix = input[arg_start..cursor].to_string();

                CmdlineCompletionContext::Argument {
                    command,
                    prefix,
                    arg_start,
                }
            }
        }
    }

    /// Apply a completion result to the input
    ///
    /// Replaces text from `replace_start` to current cursor with `text`.
    pub fn apply_completion(&mut self, text: &str, replace_start: usize) {
        // Clamp replace_start to valid range
        let replace_start = replace_start.min(self.cursor);

        // Replace the text from replace_start to cursor
        self.input.replace_range(replace_start..self.cursor, text);

        // Update cursor to end of inserted text
        self.cursor = replace_start + text.len();
    }
}
