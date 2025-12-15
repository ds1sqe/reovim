use std::collections::BTreeMap;

use crate::command::{BufferCommandExecutor, Command, CommandContext, CommandResult};
use crate::command_line::ExCommand;
use crate::command_line::CommandLine;
use crate::event::{
    BufferEvent, CommandEvent, CommandHandler, InputEventBroker, TerminateHandler,
};
use crate::modd::Mod;

use {
    crate::{buffer::Buffer, event::InnerEvent, screen::Screen},
    tokio::sync::mpsc,
};

// own buffers and screen, windows
pub struct Runtime {
    pub buffers: BTreeMap<usize, Buffer>,
    pub screen: Screen,
    pub current_mode: Mod,
    pub clipboard: String,
    pub command_line: CommandLine,
    pub tx: mpsc::Sender<InnerEvent>,
    pub rx: mpsc::Receiver<InnerEvent>,
}

impl Default for Runtime {
    fn default() -> Self {
        Self::new(Screen::default())
    }
}

impl Runtime {
    pub fn new(screen: Screen) -> Self {
        let (tx, rx) = mpsc::channel(255);
        Self {
            buffers: BTreeMap::new(),
            screen,
            current_mode: Mod::Normal,
            clipboard: String::new(),
            command_line: CommandLine::default(),
            tx,
            rx,
        }
    }

    pub async fn init(mut self) {
        self.buffers.insert(0, Buffer::empty(0));
        let input_broker = InputEventBroker::default();

        // Command handler for key-to-command translation
        let mut command_hdr = CommandHandler::new(self.tx.clone());
        let mut terminate_hdr = TerminateHandler::new(self.tx.clone());

        input_broker.key_broker.enlist(&mut command_hdr);
        input_broker.key_broker.enlist(&mut terminate_hdr);

        tokio::spawn(async move { command_hdr.run().await });
        tokio::spawn(async move { terminate_hdr.run().await });
        tokio::spawn(async move { input_broker.subscribe().await });

        loop {
            let next = self.rx.recv().await;
            match next {
                Some(ev) => match ev {
                    InnerEvent::BufferEvent(buffer_event) => match buffer_event {
                        BufferEvent::SetContent { buffer_id, content } => {
                            if let Some(b) = self.buffers.get_mut(&buffer_id) {
                                b.set_content(&content);
                                self.render();
                            }
                        }
                    },
                    InnerEvent::CommandEvent(cmd_event) => {
                        if self.handle_command(cmd_event) {
                            break;
                        }
                    }
                    InnerEvent::ModeChangeEvent(new_mode) => {
                        // Handle state changes on mode change
                        match &new_mode {
                            Mod::Visual(_) => {
                                // Start selection when entering visual mode
                                if let Some(buffer) = self.buffers.get_mut(&0) {
                                    buffer.start_selection();
                                }
                            }
                            Mod::Normal => {
                                // Clear selection when returning to normal mode
                                if let Some(buffer) = self.buffers.get_mut(&0) {
                                    buffer.clear_selection();
                                }
                                // Note: command line is cleared in handle_command_line_command
                                // after the command is executed, not here (to avoid race condition)
                            }
                            Mod::Command => {
                                // Activate command line when entering command mode
                                self.command_line.activate();
                            }
                            _ => {}
                        }
                        self.current_mode = new_mode;
                        self.render();
                    }
                    InnerEvent::WindowEvent => todo!(),
                    InnerEvent::RenderSignal => {
                        self.render();
                    }
                    InnerEvent::KillSignal => {
                        break;
                    }
                },
                None => {
                    self.tx
                        .send(InnerEvent::KillSignal)
                        .await
                        .expect("cannot broadcast kill signal");
                    break;
                }
            }
        }
        self.screen.finalize();
    }

    /// Helper to render the screen with current state
    fn render(&mut self) {
        let buffers: Vec<Buffer> = self.buffers.values().cloned().collect();
        self.screen
            .render(&buffers, &self.current_mode, &self.command_line)
            .expect("failed to render");
        self.screen.flush().expect("failed to flush");
    }

    /// Handle command line mode commands. Returns true if editor should quit.
    fn handle_command_line_command(&mut self, cmd: &Command, _ctx: &CommandContext) -> bool {
        match cmd {
            Command::CommandLineChar(c) => {
                self.command_line.insert_char(*c);
                self.render();
            }
            Command::CommandLineBackspace => {
                self.command_line.delete_char();
                self.render();
            }
            Command::CommandLineExecute => {
                if let Some(ex_cmd) = self.command_line.execute() {
                    match ex_cmd {
                        ExCommand::Quit => {
                            self.command_line.clear();
                            return true;
                        }
                        ExCommand::Write { filename: _ } => {
                            // TODO: implement file writing
                            // For now, just show that we received the command
                        }
                        ExCommand::WriteQuit => {
                            // TODO: implement file writing then quit
                            self.command_line.clear();
                            return true;
                        }
                        ExCommand::Unknown(_) => {
                            // TODO: show "unknown command" error
                        }
                    }
                }
                // Clear command line after execution
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
    fn handle_command(&mut self, cmd_event: CommandEvent) -> bool {
        let CommandEvent { command, context } = cmd_event;

        if let Some(buffer) = self.buffers.get_mut(&context.buffer_id) {
            let result =
                BufferCommandExecutor::execute_on_buffer(buffer, &command, &context);

            match result {
                CommandResult::NeedsRender => {
                    self.render();
                }
                CommandResult::ModeChange(_new_mode) => {
                    // Mode is tracked via ModeChangeEvent
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
                    if let Some(buf) = self.buffers.get_mut(&context.buffer_id) {
                        if !self.clipboard.is_empty() {
                            for c in self.clipboard.chars() {
                                buf.insert_char(c);
                            }
                        }
                    }
                    self.render();
                }
                CommandResult::CommandLineCommand => {
                    // Handle command line commands directly in Runtime
                    return self.handle_command_line_command(&command, &context);
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
