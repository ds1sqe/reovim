//! Event handler dispatch methods for Runtime

use crate::buffer::TextOps;
use crate::command::{BufferCommandExecutor, Command, CommandContext, CommandResult};
use crate::command_line::{ExCommand, SetOption};
use crate::event::CommandEvent;

use super::Runtime;

impl Runtime {
    /// Handle command line mode commands. Returns true if editor should quit.
    #[allow(clippy::match_same_arms)]
    pub(crate) fn handle_command_line_command(
        &mut self,
        cmd: &Command,
        _ctx: &CommandContext,
    ) -> bool {
        match cmd {
            Command::CommandLineChar(c) => {
                self.command_line.insert_char(*c);
            }
            Command::CommandLineBackspace => {
                self.command_line.delete_char();
            }
            Command::CommandLineExecute => {
                if let Some(ex_cmd) = self.command_line.execute() {
                    match ex_cmd {
                        ExCommand::Quit => {
                            self.command_line.clear();
                            return true;
                        }
                        ExCommand::Write { filename } => {
                            // Determine file path: use provided filename or buffer's file_path
                            let path = filename.or_else(|| {
                                self.buffers.get(&0).and_then(|b| b.file_path.clone())
                            });

                            if let Some(path) = path
                                && let Some(buffer) = self.buffers.get_mut(&0)
                            {
                                let content = buffer.content_to_string();
                                if std::fs::write(&path, &content).is_ok() {
                                    buffer.file_path = Some(path);
                                }
                            }
                        }
                        ExCommand::WriteQuit => {
                            // Write file then quit
                            let path = self.buffers.get(&0).and_then(|b| b.file_path.clone());
                            if let Some(path) = path
                                && let Some(buffer) = self.buffers.get(&0)
                            {
                                let content = buffer.content_to_string();
                                let _ = std::fs::write(&path, &content);
                            }
                            self.command_line.clear();
                            return true;
                        }
                        ExCommand::Set { option } => match option {
                            SetOption::Number(enabled) => {
                                self.screen.set_number(enabled);
                            }
                            SetOption::RelativeNumber(enabled) => {
                                self.screen.set_relative_number(enabled);
                            }
                        },
                        ExCommand::Unknown(_) => {
                            // TODO: show "unknown command" error
                        }
                    }
                }
                // Clear command line after execution (render handled by handle_command)
                self.command_line.clear();
            }
            Command::CommandLineCancel => {
                // Just clear the command line, mode change handled by CommandHandler
                self.command_line.clear();
            }
            _ => {}
        }
        false
    }

    /// Handle a command event. Returns true if the editor should quit.
    #[allow(clippy::match_same_arms)]
    pub(crate) fn handle_command(&mut self, cmd_event: CommandEvent) -> bool {
        let CommandEvent { command, context } = cmd_event;

        if let Some(buffer) = self.buffers.get_mut(&context.buffer_id) {
            let result = BufferCommandExecutor::execute_on_buffer(buffer, &command, &context);

            match result {
                CommandResult::NeedsRender => {
                    self.render();
                }
                CommandResult::ModeChange(new_mode) => {
                    // Actually change the mode for commands like o, O
                    self.set_mode(new_mode);
                    self.render();
                }
                CommandResult::Quit => {
                    return true;
                }
                CommandResult::VisualDeleteResult(text) => {
                    self.clipboard = text;
                    self.render();
                }
                CommandResult::VisualYankResult(text) => {
                    self.clipboard = text;
                    self.render();
                }
                CommandResult::PasteCommand => {
                    // Handle paste
                    if let Some(buf) = self.buffers.get_mut(&context.buffer_id)
                        && !self.clipboard.is_empty()
                    {
                        for c in self.clipboard.chars() {
                            buf.insert_char(c);
                        }
                    }
                    self.render();
                }
                CommandResult::CommandLineCommand => {
                    // Handle command line commands directly in Runtime
                    let should_quit = self.handle_command_line_command(&command, &context);
                    self.render();
                    return should_quit;
                }
                CommandResult::Error(_msg) => {
                    // TODO: display error message
                }
                CommandResult::Success => {}
            }
        }
        false
    }
}
