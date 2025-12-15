use std::collections::BTreeMap;

use crate::command::{BufferCommandExecutor, Command, CommandContext, CommandResult};
use crate::command_line::{ExCommand, SetOption};
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
    pub initial_file: Option<String>,
    showing_landing_page: bool,
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
            initial_file: None,
            showing_landing_page: false,
        }
    }

    pub fn with_file(mut self, file_path: Option<String>) -> Self {
        self.initial_file = file_path;
        self
    }

    pub async fn init(mut self) {
        let mut buffer = Buffer::empty(0);

        // Load file if provided, otherwise show landing page
        if let Some(ref path) = self.initial_file {
            if let Ok(content) = std::fs::read_to_string(path) {
                buffer.set_content(&content);
            }
            buffer.file_path = Some(path.clone());
        } else {
            // Show landing page when no file is opened
            let landing_content = crate::landing::generate(
                self.screen.width(),
                self.screen.height().saturating_sub(1), // Reserve status line
            );
            buffer.set_content(&landing_content);
            self.showing_landing_page = true;
        }

        self.buffers.insert(0, buffer);
        let input_broker = InputEventBroker::default();

        // Command handler for key-to-command translation
        let mut command_hdr = CommandHandler::new(self.tx.clone());
        let mut terminate_hdr = TerminateHandler::new(self.tx.clone());

        input_broker.key_broker.enlist(&mut command_hdr);
        input_broker.key_broker.enlist(&mut terminate_hdr);

        tokio::spawn(async move { command_hdr.run().await });
        tokio::spawn(async move { terminate_hdr.run().await });
        tokio::spawn(async move { input_broker.subscribe().await });

        // Initial render to show content immediately
        self.render();

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
                            Mod::Insert(_) => {
                                // Clear landing page content when entering insert mode (only once)
                                if self.showing_landing_page {
                                    if let Some(buffer) = self.buffers.get_mut(&0) {
                                        buffer.contents.clear();
                                        buffer.cur.x = 0;
                                        buffer.cur.y = 0;
                                    }
                                    self.showing_landing_page = false;
                                }
                            }
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

                            if let Some(path) = path {
                                if let Some(buffer) = self.buffers.get_mut(&0) {
                                    let content = buffer.to_string();
                                    if std::fs::write(&path, &content).is_ok() {
                                        buffer.file_path = Some(path);
                                    }
                                }
                            }
                        }
                        ExCommand::WriteQuit => {
                            // Write file then quit
                            let path = self.buffers.get(&0).and_then(|b| b.file_path.clone());
                            if let Some(path) = path {
                                if let Some(buffer) = self.buffers.get(&0) {
                                    let content = buffer.to_string();
                                    let _ = std::fs::write(&path, &content);
                                }
                            }
                            self.command_line.clear();
                            return true;
                        }
                        ExCommand::Set { option } => {
                            match option {
                                SetOption::Number(enabled) => {
                                    self.screen.set_number(enabled);
                                }
                                SetOption::RelativeNumber(enabled) => {
                                    self.screen.set_relative_number(enabled);
                                }
                            }
                        }
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
    fn handle_command(&mut self, cmd_event: CommandEvent) -> bool {
        let CommandEvent { command, context } = cmd_event;

        if let Some(buffer) = self.buffers.get_mut(&context.buffer_id) {
            let result =
                BufferCommandExecutor::execute_on_buffer(buffer, &command, &context);

            match result {
                CommandResult::NeedsRender => {
                    self.render();
                }
                CommandResult::ModeChange(new_mode) => {
                    // Actually change the mode for commands like o, O
                    self.current_mode = new_mode;
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
