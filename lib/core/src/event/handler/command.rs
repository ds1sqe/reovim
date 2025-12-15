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
    pending_count: Option<usize>,
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
            pending_count: None,
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

    /// Lookup command - assumes key is already pushed to pending_keys
    fn lookup_command_no_push(&mut self, key: &str) -> (Option<Command>, bool) {
        let keymap = self.get_keymap_for_mode();

        if let Some(inner) = keymap.get(&self.pending_keys) {
            if inner.command.is_some() {
                let cmd = inner.command.clone();
                self.pending_keys.clear();
                return (cmd, true);
            }
            // Has children, wait for more keys
            return (None, true);
        }

        // No match, clear pending
        self.pending_keys.clear();

        // In insert mode, non-mapped single chars become InsertChar
        if matches!(self.current_mode, Mod::Insert(_)) && key.len() == 1 {
            if let Some(c) = key.chars().next() {
                return (Some(Command::InsertChar(c)), true);
            }
        }

        // In command mode, non-mapped single chars become CommandLineChar
        if matches!(self.current_mode, Mod::Command) && key.len() == 1 {
            if let Some(c) = key.chars().next() {
                return (Some(Command::CommandLineChar(c)), true);
            }
        }

        (None, true)
    }

    async fn update_mode(&mut self, new_mode: Mod) {
        self.current_mode = new_mode.clone();
        let _ = self.inner_tx.send(InnerEvent::ModeChangeEvent(new_mode)).await;
    }

    async fn dispatch_command_with_count(&mut self, cmd: Command) {
        let count = self.take_count();
        let ctx = CommandContext {
            buffer_id: self.current_buffer_id,
            window_id: self.current_window_id,
            count,
        };

        let _ = self
            .inner_tx
            .send(InnerEvent::CommandEvent(CommandEvent {
                command: cmd,
                context: ctx,
            }))
            .await;
    }

    /// Check if a key is a digit that should be accumulated as count
    fn is_count_digit(&self, key: &str) -> bool {
        if !matches!(self.current_mode, Mod::Normal | Mod::Visual(_)) {
            return false;
        }
        if let Some(c) = key.chars().next() {
            if c.is_ascii_digit() {
                // '0' is only a count digit if we already have a count started
                // Otherwise '0' is the line-start command
                if c == '0' {
                    return self.pending_count.is_some();
                }
                return true;
            }
        }
        false
    }

    /// Accumulate a digit into pending_count
    fn accumulate_count(&mut self, key: &str) {
        if let Some(c) = key.chars().next() {
            if let Some(digit) = c.to_digit(10) {
                let current = self.pending_count.unwrap_or(0);
                self.pending_count = Some(current * 10 + digit as usize);
            }
        }
    }

    /// Get and clear the pending count
    fn take_count(&mut self) -> Option<usize> {
        self.pending_count.take()
    }

    /// Build display string for pending keys (including count)
    fn pending_display(&self) -> String {
        let mut display = String::new();
        if let Some(count) = self.pending_count {
            display.push_str(&count.to_string());
        }
        display.push_str(&self.pending_keys);
        display
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

                        // Check if this is a count digit
                        if self.is_count_digit(&key_str) {
                            self.accumulate_count(&key_str);
                            // Send updated display
                            let _ = self
                                .inner_tx
                                .send(InnerEvent::PendingKeysEvent(self.pending_display()))
                                .await;
                            continue;
                        }

                        // Show the key being pressed (before lookup clears it)
                        self.pending_keys.push_str(&key_str);
                        let _ = self
                            .inner_tx
                            .send(InnerEvent::PendingKeysEvent(self.pending_display()))
                            .await;

                        // Now do the lookup (which may clear pending_keys)
                        let (cmd, _) = self.lookup_command_no_push(&key_str);

                        if let Some(cmd) = cmd {
                            // Check for mode change commands
                            match &cmd {
                                Command::EnterNormalMode => {
                                    self.update_mode(Mod::Normal).await;
                                }
                                Command::EnterInsertMode
                                | Command::EnterInsertModeAfter
                                | Command::EnterInsertModeEndOfLine
                                | Command::OpenLineBelow
                                | Command::OpenLineAbove => {
                                    self.update_mode(Mod::Insert(
                                        crate::modd::ModExtension::Normal,
                                    ))
                                    .await;
                                }
                                Command::EnterVisualMode => {
                                    self.update_mode(Mod::Visual(
                                        crate::modd::ModExtension::Normal,
                                    ))
                                    .await;
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
                            self.dispatch_command_with_count(cmd).await;

                            // Clear display after command execution
                            let _ = self
                                .inner_tx
                                .send(InnerEvent::PendingKeysEvent(self.pending_display()))
                                .await;
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
