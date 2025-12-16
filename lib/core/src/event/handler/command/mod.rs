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
            registry::CommandRegistry,
            traits::OperatorMotionAction,
            CommandTrait,
        },
        event::{InnerEvent, KeyEvent, Subscribe},
        modd::{Mod, OperatorType},
        motion::Motion,
        textobject::{Delimiter, TextObject, TextObjectScope},
    },
    std::{collections::HashMap, sync::Arc, time::Duration},
    tokio::sync::{broadcast::Receiver, mpsc::Sender, watch},
};

/// Default timeout for which-key popup (500ms)
const WHICH_KEY_TIMEOUT_MS: u64 = 500;

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
        completion_active_rx: watch::Receiver<bool>,
        registry: Arc<CommandRegistry>,
    ) -> Self {
        Self {
            key_event_rx: None,
            keymap: KeyMap::with_defaults(),
            mode_rx,
            completion_active_rx,
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
            Mod::OperatorPending { .. } => &self.keymap.operator_pending,
            Mod::Telescope => &self.keymap.telescope,
        }
    }

    /// Handle operator-pending mode specially
    /// Returns Some(action) if the key triggers an operator, None otherwise
    /// Returns None with `should_wait=true` if waiting for more keys (text object)
    fn handle_operator_pending(
        &self,
        key: &str,
        count: Option<usize>,
        pending: &str,
    ) -> (Option<OperatorMotionAction>, bool) {
        let mode = self.current_mode();
        if let Mod::OperatorPending { operator, count: op_count } = mode {
            // Calculate total count (operator_count * motion_count)
            let _total_count = op_count.unwrap_or(1) * count.unwrap_or(1);

            // Check for text object completion: pending ends with "i" or "a", key is delimiter
            if (pending.ends_with('i') || pending.ends_with('a'))
                && key.len() == 1
                && let Some(delim_char) = key.chars().next()
                && let Some(delimiter) = Delimiter::from_char(delim_char)
            {
                let scope = if pending.ends_with('i') {
                    TextObjectScope::Inner
                } else {
                    TextObjectScope::Around
                };
                let text_object = TextObject::new(scope, delimiter);
                let action = match operator {
                    OperatorType::Delete => OperatorMotionAction::DeleteTextObject { text_object },
                    OperatorType::Yank => OperatorMotionAction::YankTextObject { text_object },
                    OperatorType::Change => OperatorMotionAction::ChangeTextObject { text_object },
                };
                return (Some(action), false);
            }

            // If key is "i" or "a", wait for delimiter
            if key == "i" || key == "a" {
                return (None, true); // Wait for delimiter
            }

            // Check if key is a motion
            if let Some(motion) = Motion::from_key(key) {
                let total_count = op_count.unwrap_or(1) * count.unwrap_or(1);
                let action = match operator {
                    OperatorType::Delete => OperatorMotionAction::Delete { motion, count: total_count },
                    OperatorType::Yank => OperatorMotionAction::Yank { motion, count: total_count },
                    OperatorType::Change => OperatorMotionAction::Change { motion, count: total_count },
                };
                return (Some(action), false);
            }

            // Handle 'd' for dd (delete line), 'y' for yy, 'c' for cc
            match (key, &operator) {
                ("d", OperatorType::Delete) | ("y", OperatorType::Yank) | ("c", OperatorType::Change) => {
                    // dd/yy/cc: delete/yank/change current line(s)
                    // Use Down motion with count-1 to affect count lines
                    let total_count = op_count.unwrap_or(1) * count.unwrap_or(1);
                    let motion = Motion::Down;
                    let line_count = if total_count == 1 { 0 } else { total_count - 1 };
                    let action = match operator {
                        OperatorType::Delete => OperatorMotionAction::Delete { motion, count: line_count },
                        OperatorType::Yank => OperatorMotionAction::Yank { motion, count: line_count },
                        OperatorType::Change => OperatorMotionAction::Change { motion, count: line_count },
                    };
                    return (Some(action), false);
                }
                _ => {}
            }
        }
        (None, false)
    }

    /// Lookup command - assumes key is already pushed to `pending_keys`
    fn lookup_command_no_push(&mut self, key: &str) -> (Option<CommandRef>, bool) {
        let mode = self.current_mode();
        tracing::debug!(?mode, key, pending = %self.pending_keys, "lookup_command_no_push");

        let is_insert = matches!(mode, Mod::Insert(_));
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

        // Check for exact match
        if let Some(inner) = keymap.get(&self.pending_keys) {
            if inner.command.is_some() {
                let cmd = inner.command.clone();
                self.pending_keys.clear();
                return (cmd, true);
            }
            // Has children, wait for more keys
            return (None, true);
        }

        // Check if current pending_keys is a valid prefix for any binding
        let is_valid_prefix = keymap
            .keys()
            .any(|k| k.starts_with(&self.pending_keys) && k != &self.pending_keys);

        if is_valid_prefix {
            // Valid prefix, wait for more keys (which-key will trigger on timeout)
            return (None, true);
        }

        // No match and not a valid prefix
        // In insert/command/explorer-input modes, handle single chars specially
        // In these modes, clear pending and process the char immediately
        if is_insert
            && key.len() == 1
            && let Some(c) = key.chars().next()
        {
            self.pending_keys.clear();
            let cmd: Arc<dyn CommandTrait> = Arc::new(InsertCharCommand::new(c));
            return (Some(CommandRef::Inline(cmd)), true);
        }

        if matches!(mode, Mod::Command)
            && key.len() == 1
            && let Some(c) = key.chars().next()
        {
            self.pending_keys.clear();
            let cmd: Arc<dyn CommandTrait> = Arc::new(CommandLineCharCommand::new(c));
            return (Some(CommandRef::Inline(cmd)), true);
        }

        if matches!(mode, Mod::ExplorerInput)
            && key.len() == 1
            && let Some(c) = key.chars().next()
        {
            self.pending_keys.clear();
            let cmd: Arc<dyn CommandTrait> = Arc::new(ExplorerInputCharCommand::new(c));
            return (Some(CommandRef::Inline(cmd)), true);
        }

        // In Normal/Visual/Explorer modes, keep pending_keys for backspace correction
        // User can press Escape to clear, or backspace to remove last key
        (None, true)
    }

    /// Build display string for pending keys (including count)
    fn pending_display(&self) -> String {
        let mut display = String::new();
        if let Some(count) = self.count_parser.peek() {
            display.push_str(&count.to_string());
        }
        // Replace invisible/special characters with visible representations
        for c in self.pending_keys.chars() {
            match c {
                ' ' => display.push_str("SPC "),
                _ => display.push(c),
            }
        }
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

    /// Check if which-key should be shown
    /// Returns true when:
    /// - `pending_keys` is empty (show all bindings)
    /// - `pending_keys` is a valid prefix (show prefix-specific bindings)
    fn should_show_which_key(&self) -> bool {
        // Always show which-key when pending_keys is empty (show all bindings)
        if self.pending_keys.is_empty() {
            return true;
        }

        // Check if there are any bindings that start with this prefix
        let keymap = self.get_keymap_for_mode();
        keymap
            .keys()
            .any(|k| k.starts_with(&self.pending_keys) && k != &self.pending_keys)
    }

    #[allow(clippy::while_let_loop)]
    #[allow(clippy::match_same_arms)]
    #[allow(clippy::too_many_lines)]
    pub async fn run(mut self) {
        if let Some(rx) = self.key_event_rx.take() {
            let mut rx = rx;
            loop {
                // Calculate timeout duration based on pending keys state
                let timeout = if self.should_show_which_key() && !self.which_key_shown {
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

                                tracing::trace!(key = %key_str, "Key pressed");

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

                                // Handle Escape to clear pending_keys in Normal/Visual modes
                                if key_str == "Escape" && !self.pending_keys.is_empty() {
                                    let mode = self.current_mode();
                                    if matches!(mode, Mod::Normal | Mod::Visual(_) | Mod::Explorer) {
                                        self.pending_keys.clear();
                                        self.count_parser.clear();
                                        self.dispatcher
                                            .send_pending_keys(self.pending_display())
                                            .await;
                                        continue;
                                    }
                                }

                                // Handle backspace in Normal/Visual/Explorer modes
                                if key_str == "Backspace" {
                                    let mode = self.current_mode();
                                    if matches!(mode, Mod::Normal | Mod::Visual(_) | Mod::Explorer) {
                                        if !self.pending_keys.is_empty() {
                                            // Remove last character/key from pending
                                            // Handle multi-char keys like "<C-x>" properly
                                            if self.pending_keys.ends_with('>') {
                                                // Remove entire <...> sequence
                                                if let Some(start) = self.pending_keys.rfind('<') {
                                                    self.pending_keys.truncate(start);
                                                } else {
                                                    self.pending_keys.pop();
                                                }
                                            } else {
                                                self.pending_keys.pop();
                                            }
                                            self.dispatcher
                                                .send_pending_keys(self.pending_display())
                                                .await;
                                        }
                                        // In Normal/Visual/Explorer, ignore backspace (don't add to pending)
                                        continue;
                                    }
                                }

                                // Handle operator-pending mode (d, y, c + motion or text object)
                                let mode = self.current_mode();
                                if matches!(mode, Mod::OperatorPending { .. }) {
                                    let count = self.count_parser.peek();
                                    let (action, should_wait) = self.handle_operator_pending(
                                        &key_str,
                                        count,
                                        &self.pending_keys,
                                    );
                                    if let Some(action) = action {
                                        tracing::debug!(?action, "Operator-pending action detected");
                                        self.pending_keys.clear();
                                        self.count_parser.take(); // Consume count
                                        self.dispatcher.send_operator_motion(action).await;
                                        self.dispatcher
                                            .send_pending_keys(self.pending_display())
                                            .await;
                                        continue;
                                    }
                                    if should_wait {
                                        // Waiting for text object delimiter (i/a pressed)
                                        self.pending_keys.push_str(&key_str);
                                        self.dispatcher
                                            .send_pending_keys(self.pending_display())
                                            .await;
                                        continue;
                                    }
                                }

                                // Show the key being pressed (before lookup clears it)
                                self.pending_keys.push_str(&key_str);
                                self.dispatcher
                                    .send_pending_keys(self.pending_display())
                                    .await;

                                // Now do the lookup (which may clear pending_keys)
                                let (cmd, _) = self.lookup_command_no_push(&key_str);

                                if let Some(ref cmd) = cmd {
                                    tracing::debug!(?cmd, count = ?self.count_parser.peek(), "Dispatching command");

                                    // Check for mode change commands and notify Runtime
                                    if let Some(new_mode) = Dispatcher::mode_for_command(cmd) {
                                        self.dispatcher.update_mode(new_mode).await;
                                    }

                                    // Dispatch the command with count
                                    let count = self.count_parser.take();
                                    self.dispatcher.dispatch(cmd.clone(), count).await;

                                    // Clear display after command execution
                                    self.dispatcher
                                        .send_pending_keys(self.pending_display())
                                        .await;
                                }
                            }
                            Err(e) => {
                                tracing::debug!(error = %e, "Key event channel closed");
                                break;
                            }
                        }
                    }
                    // Timeout elapsed - show which-key popup
                    () = tokio::time::sleep(timeout) => {
                        if self.should_show_which_key() && !self.which_key_shown {
                            self.show_which_key().await;
                        }
                    }
                }
            }
        }
    }
}
