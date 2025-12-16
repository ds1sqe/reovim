//! Command handler for translating key events to editor commands

mod count_parser;
mod dispatcher;
mod key_parser;

pub use count_parser::CountParser;
pub use dispatcher::Dispatcher;
pub use key_parser::key_to_string;

use {
    crate::{
        bind::{CommandRef, KeyMap, KeyMapInner},
        command::{
            builtin::{CommandLineCharCommand, InsertCharCommand},
            registry::CommandRegistry,
            CommandTrait,
        },
        event::{InnerEvent, KeyEvent, Subscribe},
        modd::Mod,
    },
    std::{collections::HashMap, sync::Arc, time::Duration},
    tokio::sync::{broadcast::Receiver, mpsc::Sender, watch},
};

/// Default timeout for which-key popup (500ms)
const WHICH_KEY_TIMEOUT_MS: u64 = 500;

/// Handler that translates key events to commands based on current mode
pub struct CommandHandler {
    key_event_rx: Option<Receiver<KeyEvent>>,
    keymap: KeyMap,
    /// Watch receiver for mode changes from Runtime (single source of truth)
    mode_rx: watch::Receiver<Mod>,
    pending_keys: String,
    count_parser: CountParser,
    dispatcher: Dispatcher,
    /// Command registry for looking up command descriptions
    registry: Arc<CommandRegistry>,
    /// Timeout duration for which-key popup
    which_key_timeout: Duration,
    /// Whether the which-key popup is currently shown
    which_key_shown: bool,
}

impl Subscribe<KeyEvent> for CommandHandler {
    fn subscribe(&mut self, rx: Receiver<KeyEvent>) {
        self.key_event_rx = Some(rx);
    }
}

impl CommandHandler {
    #[must_use]
    pub fn new(
        tx: Sender<InnerEvent>,
        mode_rx: watch::Receiver<Mod>,
        registry: Arc<CommandRegistry>,
    ) -> Self {
        Self {
            key_event_rx: None,
            keymap: KeyMap::with_defaults(),
            mode_rx,
            pending_keys: String::new(),
            count_parser: CountParser::new(),
            dispatcher: Dispatcher::new(tx, 0, 0),
            registry,
            which_key_timeout: Duration::from_millis(WHICH_KEY_TIMEOUT_MS),
            which_key_shown: false,
        }
    }

    /// Get the current mode from the watch channel
    fn current_mode(&self) -> Mod {
        self.mode_rx.borrow().clone()
    }

    fn get_keymap_for_mode(&self) -> &HashMap<String, KeyMapInner> {
        match self.current_mode() {
            Mod::Normal => &self.keymap.normal,
            Mod::Insert(_) => &self.keymap.insert,
            Mod::Visual(_) => &self.keymap.visual,
            Mod::Command => &self.keymap.command,
        }
    }

    /// Lookup command - assumes key is already pushed to `pending_keys`
    fn lookup_command_no_push(&mut self, key: &str) -> (Option<CommandRef>, bool) {
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

        // In insert mode, non-mapped single chars become InsertChar (inline command)
        if matches!(self.current_mode(), Mod::Insert(_))
            && key.len() == 1
            && let Some(c) = key.chars().next()
        {
            let cmd: Arc<dyn CommandTrait> = Arc::new(InsertCharCommand::new(c));
            return (Some(CommandRef::Inline(cmd)), true);
        }

        // In command mode, non-mapped single chars become CommandLineChar (inline command)
        if matches!(self.current_mode(), Mod::Command)
            && key.len() == 1
            && let Some(c) = key.chars().next()
        {
            let cmd: Arc<dyn CommandTrait> = Arc::new(CommandLineCharCommand::new(c));
            return (Some(CommandRef::Inline(cmd)), true);
        }

        (None, true)
    }

    /// Build display string for pending keys (including count)
    fn pending_display(&self) -> String {
        let mut display = String::new();
        if let Some(count) = self.count_parser.peek() {
            display.push_str(&count.to_string());
        }
        display.push_str(&self.pending_keys);
        display
    }

    /// Show the which-key popup with available bindings
    async fn show_which_key(&mut self) {
        let mode = self.current_mode();
        let bindings = self
            .keymap
            .get_bindings_for_prefix(&mode, &self.pending_keys, &self.registry);

        if !bindings.is_empty() {
            self.dispatcher
                .send_which_key_show(self.pending_keys.clone(), bindings)
                .await;
            self.which_key_shown = true;
        }
    }

    /// Hide the which-key popup
    async fn hide_which_key(&mut self) {
        if self.which_key_shown {
            self.dispatcher.send_which_key_hide().await;
            self.which_key_shown = false;
        }
    }

    /// Check if we have a pending prefix that could trigger which-key
    fn has_pending_prefix(&self) -> bool {
        if self.pending_keys.is_empty() {
            return false;
        }

        // Check if there are any bindings that start with this prefix
        let keymap = self.get_keymap_for_mode();
        keymap.keys().any(|k| k.starts_with(&self.pending_keys) && k != &self.pending_keys)
    }

    #[allow(clippy::while_let_loop)]
    #[allow(clippy::match_same_arms)]
    pub async fn run(mut self) {
        if let Some(rx) = self.key_event_rx.take() {
            let mut rx = rx;
            loop {
                // Calculate timeout duration based on pending keys state
                let timeout = if self.has_pending_prefix() && !self.which_key_shown {
                    self.which_key_timeout
                } else {
                    // No pending prefix or already showing - use long timeout
                    Duration::from_secs(3600)
                };

                tokio::select! {
                    // Key event received
                    result = rx.recv() => {
                        match result {
                            Ok(event) => {
                                let key_str = key_to_string(&event);
                                if key_str.is_empty() {
                                    continue;
                                }

                                // Hide which-key popup on any key press
                                self.hide_which_key().await;

                                // Check if this is a count digit
                                let mode = self.current_mode();
                                if self.count_parser.is_count_digit(&key_str, &mode) {
                                    self.count_parser.accumulate(&key_str);
                                    self.dispatcher
                                        .send_pending_keys(self.pending_display())
                                        .await;
                                    continue;
                                }

                                // Show the key being pressed (before lookup clears it)
                                self.pending_keys.push_str(&key_str);
                                self.dispatcher
                                    .send_pending_keys(self.pending_display())
                                    .await;

                                // Now do the lookup (which may clear pending_keys)
                                let (cmd, _) = self.lookup_command_no_push(&key_str);

                                if let Some(cmd) = cmd {
                                    // Check for mode change commands and notify Runtime
                                    if let Some(new_mode) = Dispatcher::mode_for_command(&cmd) {
                                        self.dispatcher.update_mode(new_mode).await;
                                    }

                                    // Dispatch the command with count
                                    let count = self.count_parser.take();
                                    self.dispatcher.dispatch(cmd, count).await;

                                    // Clear display after command execution
                                    self.dispatcher
                                        .send_pending_keys(self.pending_display())
                                        .await;
                                }
                            }
                            Err(_) => {
                                break;
                            }
                        }
                    }
                    // Timeout elapsed - show which-key popup
                    () = tokio::time::sleep(timeout) => {
                        if self.has_pending_prefix() && !self.which_key_shown {
                            self.show_which_key().await;
                        }
                    }
                }
            }
        }
    }
}
