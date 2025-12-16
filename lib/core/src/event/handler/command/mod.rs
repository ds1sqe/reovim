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
            builtin::{CommandLineCharCommand, ExplorerInputCharCommand, InsertCharCommand},
            CommandTrait,
        },
        event::{InnerEvent, KeyEvent, Subscribe},
        modd::Mod,
    },
    std::{collections::HashMap, sync::Arc},
    tokio::sync::{broadcast::Receiver, mpsc::Sender, watch},
};

/// Keys that are handled differently when completion popup is visible
const COMPLETION_KEYS: &[&str] = &["Tab", "C-n", "C-p", "C-e"];

/// Handler that translates key events to commands based on current mode
pub struct CommandHandler {
    key_event_rx: Option<Receiver<KeyEvent>>,
    keymap: KeyMap,
    /// Watch receiver for mode changes from Runtime (single source of truth)
    mode_rx: watch::Receiver<Mod>,
    /// Watch receiver for completion active state
    completion_active_rx: watch::Receiver<bool>,
    pending_keys: String,
    count_parser: CountParser,
    dispatcher: Dispatcher,
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
        completion_active_rx: watch::Receiver<bool>,
    ) -> Self {
        Self {
            key_event_rx: None,
            keymap: KeyMap::with_defaults(),
            mode_rx,
            completion_active_rx,
            pending_keys: String::new(),
            count_parser: CountParser::new(),
            dispatcher: Dispatcher::new(tx, 0, 0),
        }
    }

    /// Get the current mode from the watch channel
    fn current_mode(&self) -> Mod {
        self.mode_rx.borrow().clone()
    }

    /// Check if completion popup is currently active
    fn is_completion_active(&self) -> bool {
        *self.completion_active_rx.borrow()
    }

    /// Check if a key should use completion behavior when popup is visible
    fn is_completion_key(key: &str) -> bool {
        COMPLETION_KEYS.contains(&key)
    }

    fn get_keymap_for_mode(&self) -> &HashMap<String, KeyMapInner> {
        match self.current_mode() {
            Mod::Normal => &self.keymap.normal,
            Mod::Insert(_) => &self.keymap.insert,
            Mod::Visual(_) => &self.keymap.visual,
            Mod::Command => &self.keymap.command,
            Mod::Explorer => &self.keymap.explorer,
            Mod::ExplorerInput => &self.keymap.explorer_input,
        }
    }

    /// Lookup command - assumes key is already pushed to `pending_keys`
    fn lookup_command_no_push(&mut self, key: &str) -> (Option<CommandRef>, bool) {
        let is_insert = matches!(self.current_mode(), Mod::Insert(_));
        let completion_active = self.is_completion_active();

        // In insert mode, completion keys should only trigger completion commands
        // when the popup is visible. Otherwise, fall through to default behavior.
        if is_insert && Self::is_completion_key(key) && !completion_active {
            self.pending_keys.clear();

            // Tab inserts a tab character when completion is not active
            if key == "Tab" {
                let cmd: Arc<dyn CommandTrait> = Arc::new(InsertCharCommand::new('\t'));
                return (Some(CommandRef::Inline(cmd)), true);
            }

            // C-n, C-p, C-e do nothing when completion is not active
            return (None, true);
        }

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
        if is_insert
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

        // In explorer input mode, non-mapped single chars become ExplorerInputChar (inline command)
        if matches!(self.current_mode(), Mod::ExplorerInput)
            && key.len() == 1
            && let Some(c) = key.chars().next()
        {
            let cmd: Arc<dyn CommandTrait> = Arc::new(ExplorerInputCharCommand::new(c));
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

    #[allow(clippy::while_let_loop)]
    #[allow(clippy::match_same_arms)]
    pub async fn run(mut self) {
        if let Some(rx) = self.key_event_rx.take() {
            let mut rx = rx;
            loop {
                match rx.recv().await {
                    Ok(event) => {
                        let key_str = key_to_string(&event);
                        if key_str.is_empty() {
                            continue;
                        }

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
        }
    }
}
