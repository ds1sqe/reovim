use {
    crate::{
        bind::{KeyMap, KeyMapInner},
        command::{Command, CommandContext},
        event::{inner::CommandEvent, InnerEvent, KeyEvent, Subscribe},
        modd::Mod,
    },
    reovim_sys::event::KeyCode,
    std::collections::HashMap,
    tokio::sync::{broadcast::Receiver, mpsc::Sender},
};

/// Handler that translates key events to commands based on current mode
pub struct CommandHandler {
    key_event_rx: Option<Receiver<KeyEvent>>,
    inner_tx: Sender<InnerEvent>,
    keymap: KeyMap,
    current_mode: Mod,
    pending_keys: String,
    current_buffer_id: usize,
    current_window_id: usize,
}

impl Subscribe<KeyEvent> for CommandHandler {
    fn subscribe(&mut self, rx: Receiver<KeyEvent>) {
        self.key_event_rx = Some(rx);
    }
}

impl CommandHandler {
    pub fn new(tx: Sender<InnerEvent>) -> Self {
        Self {
            key_event_rx: None,
            inner_tx: tx,
            keymap: KeyMap::with_defaults(),
            current_mode: Mod::Normal,
            pending_keys: String::new(),
            current_buffer_id: 0,
            current_window_id: 0,
        }
    }

    fn key_to_string(event: &KeyEvent) -> String {
        match event.code {
            KeyCode::Char(c) => c.to_string(),
            KeyCode::Esc => "Escape".to_string(),
            KeyCode::Enter => "Enter".to_string(),
            KeyCode::Backspace => "Backspace".to_string(),
            KeyCode::Tab => "Tab".to_string(),
            KeyCode::Left => "Left".to_string(),
            KeyCode::Right => "Right".to_string(),
            KeyCode::Up => "Up".to_string(),
            KeyCode::Down => "Down".to_string(),
            KeyCode::Home => "Home".to_string(),
            KeyCode::End => "End".to_string(),
            KeyCode::Delete => "Delete".to_string(),
            _ => String::new(),
        }
    }

    fn get_keymap_for_mode(&self) -> &HashMap<String, KeyMapInner> {
        match self.current_mode {
            Mod::Normal => &self.keymap.nmap,
            Mod::Insert(_) => &self.keymap.imap,
            Mod::Visual(_) => &self.keymap.vmap,
            Mod::Command => &self.keymap.cmap,
        }
    }

    fn lookup_command(&mut self, key: &str) -> Option<Command> {
        self.pending_keys.push_str(key);

        let keymap = self.get_keymap_for_mode();

        if let Some(inner) = keymap.get(&self.pending_keys) {
            if inner.command.is_some() {
                let cmd = inner.command.clone();
                self.pending_keys.clear();
                return cmd;
            }
            // Has children, wait for more keys
            return None;
        }

        // No match, clear pending
        self.pending_keys.clear();

        // In insert mode, non-mapped single chars become InsertChar
        if matches!(self.current_mode, Mod::Insert(_)) && key.len() == 1 {
            if let Some(c) = key.chars().next() {
                return Some(Command::InsertChar(c));
            }
        }

        // In command mode, non-mapped single chars become CommandLineChar
        if matches!(self.current_mode, Mod::Command) && key.len() == 1 {
            if let Some(c) = key.chars().next() {
                return Some(Command::CommandLineChar(c));
            }
        }

        None
    }

    async fn update_mode(&mut self, new_mode: Mod) {
        self.current_mode = new_mode.clone();
        let _ = self.inner_tx.send(InnerEvent::ModeChangeEvent(new_mode)).await;
    }

    async fn dispatch_command(&self, cmd: Command) {
        let ctx = CommandContext {
            buffer_id: self.current_buffer_id,
            window_id: self.current_window_id,
            count: None,
        };

        let _ = self
            .inner_tx
            .send(InnerEvent::CommandEvent(CommandEvent {
                command: cmd,
                context: ctx,
            }))
            .await;
    }

    pub async fn run(mut self) {
        if let Some(rx) = self.key_event_rx.take() {
            let mut rx = rx;
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        let key_str = Self::key_to_string(&event);
                        if key_str.is_empty() {
                            continue;
                        }
                        if let Some(cmd) = self.lookup_command(&key_str) {
                            // Check for mode change commands
                            match &cmd {
                                Command::EnterNormalMode => {
                                    self.update_mode(Mod::Normal).await;
                                }
                                Command::EnterInsertMode | Command::EnterInsertModeAfter => {
                                    self.update_mode(Mod::Insert(
                                        crate::modd::ModExtension::Normal,
                                    )).await;
                                }
                                Command::EnterVisualMode => {
                                    self.update_mode(Mod::Visual(
                                        crate::modd::ModExtension::Normal,
                                    )).await;
                                }
                                Command::EnterCommandMode => {
                                    self.update_mode(Mod::Command).await;
                                }
                                // Command line execute/cancel return to Normal mode
                                Command::CommandLineExecute | Command::CommandLineCancel => {
                                    self.update_mode(Mod::Normal).await;
                                }
                                // Visual delete/yank return to Normal mode
                                Command::VisualDelete | Command::VisualYank => {
                                    self.update_mode(Mod::Normal).await;
                                }
                                _ => {}
                            }
                            self.dispatch_command(cmd).await;
                        }
                    }
                    Err(_) => {
                        break;
                    }
                }
            }
        }
    }
}
