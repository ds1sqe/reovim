//! Command handler for translating key events to editor commands

mod count_parser;
mod dispatcher;
mod key_parser;

pub use {count_parser::CountParser, dispatcher::Dispatcher, key_parser::key_to_string};

use {
    crate::{
        bind::{CommandRef, KeyMap, KeyMapInner},
        command::{registry::CommandRegistry, traits::OperatorMotionAction},
        event::{InnerEvent, KeyEvent, Subscribe, VisualTextObjectAction},
        modd::{ModeState, OperatorType, SubMode},
        motion::Motion,
        textobject::{
            Delimiter, SemanticTextObject, SemanticTextObjectSpec, TextObject, TextObjectScope,
            WordTextObject, WordType,
        },
    },
    std::{collections::HashMap, sync::Arc, time::Duration},
    tokio::sync::{broadcast::Receiver, mpsc::Sender, watch},
};

/// Default timeout for which-key popup (500ms)
const WHICH_KEY_TIMEOUT_MS: u64 = 500;

/// Keys that are handled differently when completion popup is visible
const COMPLETION_KEYS: &[&str] = &["Tab", "C-n", "C-p", "C-e"];

/// Leap mode phase tracking (local to `CommandHandler`)
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
enum LeapPhase {
    #[default]
    Inactive,
    WaitingFirstChar,
    WaitingSecondChar,
    ShowingLabels,
}

/// Handler that translates key events to commands based on current mode
pub struct CommandHandler {
    key_event_rx: Option<Receiver<KeyEvent>>,
    keymap: KeyMap,
    /// Watch receiver for mode changes from Runtime
    mode_rx: watch::Receiver<ModeState>,
    /// Local mode state for immediate mode tracking (avoids race conditions)
    local_mode: ModeState,
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
    /// Current leap mode phase
    leap_phase: LeapPhase,
    /// Accumulated leap label (for multi-char labels)
    leap_label: String,
    /// Track when mode was locally changed (avoids race condition with `mode_rx` sync)
    mode_locally_changed: bool,
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
        mode_rx: watch::Receiver<ModeState>,
        completion_active_rx: watch::Receiver<bool>,
        registry: Arc<CommandRegistry>,
    ) -> Self {
        let initial_mode = mode_rx.borrow().clone();
        Self {
            key_event_rx: None,
            keymap: KeyMap::with_defaults(),
            mode_rx,
            local_mode: initial_mode,
            completion_active_rx,
            pending_keys: String::new(),
            count_parser: CountParser::new(),
            dispatcher: Dispatcher::new(tx, 0, 0),
            registry,
            which_key_timeout: Duration::from_millis(WHICH_KEY_TIMEOUT_MS),
            which_key_shown: false,
            leap_phase: LeapPhase::Inactive,
            leap_label: String::new(),
            mode_locally_changed: false,
        }
    }

    /// Get the current mode state
    const fn current_mode(&self) -> &ModeState {
        &self.local_mode
    }

    /// Set local mode state immediately
    const fn set_local_mode(&mut self, mode_state: ModeState) {
        self.local_mode = mode_state;
    }

    /// Check if completion popup is currently active
    fn is_completion_active(&self) -> bool {
        *self.completion_active_rx.borrow()
    }

    /// Check if a key should use completion behavior when popup is visible
    fn is_completion_key(key: &str) -> bool {
        COMPLETION_KEYS.contains(&key)
    }

    const fn get_keymap_for_mode(&self) -> &HashMap<String, KeyMapInner> {
        self.keymap.get_keymap_for_mode(&self.local_mode)
    }

    /// Handle operator-pending mode specially
    /// Returns Some(action) if the key triggers an operator, None otherwise
    /// Returns None with `should_wait=true` if waiting for more keys (text object)
    #[allow(clippy::too_many_lines)]
    fn handle_operator_pending(
        &self,
        key: &str,
        count: Option<usize>,
        pending: &str,
    ) -> (Option<OperatorMotionAction>, bool) {
        let mode = self.current_mode();
        if let SubMode::OperatorPending {
            operator,
            count: op_count,
        } = &mode.sub_mode
        {
            // Calculate total count (operator_count * motion_count)
            let _total_count = op_count.unwrap_or(1) * count.unwrap_or(1);

            // Check for text object completion: pending ends with "i" or "a", key is delimiter or semantic
            if (pending.ends_with('i') || pending.ends_with('a'))
                && key.len() == 1
                && let Some(obj_char) = key.chars().next()
            {
                let scope = if pending.ends_with('i') {
                    TextObjectScope::Inner
                } else {
                    TextObjectScope::Around
                };

                // Try delimiter-based text object first
                if let Some(delimiter) = Delimiter::from_char(obj_char) {
                    let text_object = TextObject::new(scope, delimiter);
                    let action = match operator {
                        OperatorType::Delete => {
                            OperatorMotionAction::DeleteTextObject { text_object }
                        }
                        OperatorType::Yank => OperatorMotionAction::YankTextObject { text_object },
                        OperatorType::Change => {
                            OperatorMotionAction::ChangeTextObject { text_object }
                        }
                    };
                    return (Some(action), false);
                }

                // Try word text object (iw, aw, iW, aW)
                if let Some(word_type) = WordType::from_char(obj_char) {
                    let text_object = WordTextObject::new(scope, word_type);
                    let action = match operator {
                        OperatorType::Delete => {
                            OperatorMotionAction::DeleteWordTextObject { text_object }
                        }
                        OperatorType::Yank => {
                            OperatorMotionAction::YankWordTextObject { text_object }
                        }
                        OperatorType::Change => {
                            OperatorMotionAction::ChangeWordTextObject { text_object }
                        }
                    };
                    return (Some(action), false);
                }

                // Try semantic text object (treesitter-based)
                if let Some(kind) = SemanticTextObject::from_char(obj_char) {
                    let text_object = SemanticTextObjectSpec::new(scope, kind);
                    let action = match operator {
                        OperatorType::Delete => {
                            OperatorMotionAction::DeleteSemanticTextObject { text_object }
                        }
                        OperatorType::Yank => {
                            OperatorMotionAction::YankSemanticTextObject { text_object }
                        }
                        OperatorType::Change => {
                            OperatorMotionAction::ChangeSemanticTextObject { text_object }
                        }
                    };
                    return (Some(action), false);
                }
            }

            // If key is "i" or "a", wait for text object specifier
            if key == "i" || key == "a" {
                return (None, true); // Wait for delimiter or semantic object key
            }

            // Check if key is a motion
            if let Some(motion) = Motion::from_key(key) {
                let total_count = op_count.unwrap_or(1) * count.unwrap_or(1);
                let action = match operator {
                    OperatorType::Delete => OperatorMotionAction::Delete {
                        motion,
                        count: total_count,
                    },
                    OperatorType::Yank => OperatorMotionAction::Yank {
                        motion,
                        count: total_count,
                    },
                    OperatorType::Change => {
                        // In vim, `cw` behaves like `ce` (change to end of word)
                        // Convert WordForward to WordEnd for change operator
                        let change_motion = if motion == Motion::WordForward {
                            Motion::WordEnd
                        } else {
                            motion
                        };
                        OperatorMotionAction::Change {
                            motion: change_motion,
                            count: total_count,
                        }
                    }
                };
                return (Some(action), false);
            }

            // Handle 'd' for dd (delete line), 'y' for yy, 'c' for cc
            match (key, *operator) {
                ("d", OperatorType::Delete)
                | ("y", OperatorType::Yank)
                | ("c", OperatorType::Change) => {
                    // dd/yy/cc: delete/yank/change current line(s)
                    // Use Down motion with count-1 to affect count lines
                    let total_count = op_count.unwrap_or(1) * count.unwrap_or(1);
                    let motion = Motion::Down;
                    let line_count = if total_count == 1 { 0 } else { total_count - 1 };
                    let action = match operator {
                        OperatorType::Delete => OperatorMotionAction::Delete {
                            motion,
                            count: line_count,
                        },
                        OperatorType::Yank => OperatorMotionAction::Yank {
                            motion,
                            count: line_count,
                        },
                        OperatorType::Change => OperatorMotionAction::Change {
                            motion,
                            count: line_count,
                        },
                    };
                    return (Some(action), false);
                }
                _ => {}
            }
        }
        (None, false)
    }

    /// Handle visual mode text object selection (viw, vi(, vif)
    /// Returns Some(action) if the key triggers a text object selection, None otherwise
    /// Returns None with `should_wait=true` if waiting for more keys (i/a pressed)
    fn handle_visual_text_object(
        key: &str,
        pending: &str,
    ) -> (Option<VisualTextObjectAction>, bool) {
        // Check for text object completion: pending ends with "i" or "a", key is delimiter/word/semantic
        if (pending.ends_with('i') || pending.ends_with('a'))
            && key.len() == 1
            && let Some(obj_char) = key.chars().next()
        {
            let scope = if pending.ends_with('i') {
                TextObjectScope::Inner
            } else {
                TextObjectScope::Around
            };

            // Try delimiter-based text object first
            if let Some(delimiter) = Delimiter::from_char(obj_char) {
                let text_object = TextObject::new(scope, delimiter);
                return (Some(VisualTextObjectAction::SelectDelimiter { text_object }), false);
            }

            // Try word text object (iw, aw, iW, aW)
            if let Some(word_type) = WordType::from_char(obj_char) {
                let text_object = WordTextObject::new(scope, word_type);
                return (Some(VisualTextObjectAction::SelectWord { text_object }), false);
            }

            // Try semantic text object (treesitter-based)
            if let Some(kind) = SemanticTextObject::from_char(obj_char) {
                let text_object = SemanticTextObjectSpec::new(scope, kind);
                return (Some(VisualTextObjectAction::SelectSemantic { text_object }), false);
            }
        }

        // If key is "i" or "a", wait for text object specifier
        if key == "i" || key == "a" {
            return (None, true); // Wait for text object key
        }

        (None, false)
    }

    /// Lookup command - assumes key is already pushed to `pending_keys`
    fn lookup_command_no_push(&mut self, key: &str) -> (Option<CommandRef>, bool) {
        let mode = self.current_mode();
        tracing::debug!(?mode, key, pending = %self.pending_keys, "lookup_command_no_push");

        let is_insert = mode.is_insert();
        let completion_active = self.is_completion_active();

        // In insert mode, completion keys should only trigger completion commands
        // when the popup is visible. Otherwise, fall through to default behavior.
        // Note: Tab insertion is now handled in run() via FocusInputEvent.
        if is_insert && Self::is_completion_key(key) && !completion_active && key != "Tab" {
            self.pending_keys.clear();
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
        // Note: Single-character input in insert/command modes is now handled earlier
        // in run() via FocusInputEvent, so this code path is only reached for special keys
        // or invalid sequences in those modes, or for any key in Normal/Visual/Explorer modes.

        // Invalid sequence - clear it to allow starting fresh with the next key
        self.pending_keys.clear();
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
        let bindings = self.keymap.get_bindings_for_prefix(
            &self.local_mode,
            &self.pending_keys,
            &self.registry,
        );

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

    /// Check if mode is one where ESC should only clear pending state (not trigger keymap lookup)
    /// Visual mode is excluded because it has an Escape binding in the keymap to exit to Normal
    /// Telescope mode is excluded because it has Escape bindings for mode switching and closing
    fn is_esc_clearable_mode(mode: &ModeState) -> bool {
        // Editor Normal, Explorer focus, or OperatorPending
        // Telescope has its own Escape handlers via keymap
        (mode.is_normal() && !mode.is_telescope_focus())
            || (mode.is_explorer_focus() && !mode.is_insert())
            || mode.is_operator_pending()
    }

    /// Check if mode is one where Backspace should edit pending keys
    fn is_backspace_editable_mode(mode: &ModeState) -> bool {
        mode.is_normal() || mode.is_visual() || (mode.is_explorer_focus() && mode.is_normal())
    }

    #[allow(clippy::while_let_loop)]
    #[allow(clippy::match_same_arms)]
    #[allow(clippy::too_many_lines)]
    #[allow(clippy::cognitive_complexity)]
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

                                // Sync local mode from Runtime's watch channel
                                // This catches mode changes initiated by Runtime (e.g., explorer focus)
                                // Don't overwrite if in a handler-initiated transient state (OperatorPending, Leap)
                                // or if mode was locally changed and runtime hasn't caught up yet
                                if !self.local_mode.is_operator_pending() && !self.local_mode.is_leap() {
                                    let runtime_mode = self.mode_rx.borrow().clone();
                                    if self.mode_locally_changed {
                                        // Check if runtime has caught up with our local change
                                        if runtime_mode == self.local_mode {
                                            self.mode_locally_changed = false;
                                        }
                                        // Otherwise keep using local_mode until runtime catches up
                                    } else {
                                        // Normal sync from runtime
                                        self.local_mode = runtime_mode;
                                    }
                                }

                                // Hide which-key popup on any key press
                                self.hide_which_key().await;

                                // Check if this is a count digit
                                let mode = self.current_mode();
                                if self.count_parser.is_count_digit(&key_str, mode) {
                                    self.count_parser.accumulate(&key_str);
                                    self.dispatcher
                                        .send_pending_keys(self.pending_display())
                                        .await;
                                    continue;
                                }

                                // Handle Escape in Normal/Visual/Explorer/OperatorPending modes:
                                // - Clear pending keys and count
                                // - In OperatorPending: cancel operator and return to Normal
                                if key_str == "Escape" {
                                    let mode = self.current_mode().clone();
                                    if Self::is_esc_clearable_mode(&mode) {
                                        // Clear pending state
                                        if !self.pending_keys.is_empty()
                                            || self.count_parser.peek().is_some()
                                        {
                                            self.pending_keys.clear();
                                            self.count_parser.clear();
                                            self.dispatcher
                                                .send_pending_keys(self.pending_display())
                                                .await;
                                        }
                                        // If in OperatorPending, cancel and return to Normal
                                        if mode.is_operator_pending() {
                                            let normal = ModeState::normal();
                                            self.set_local_mode(normal.clone());
                                            self.mode_locally_changed = true;
                                            self.dispatcher.update_mode(normal).await;
                                        }
                                        continue;
                                    }
                                }

                                // Handle backspace in Normal/Visual/Explorer modes
                                if key_str == "Backspace" {
                                    let mode = self.current_mode();
                                    if Self::is_backspace_editable_mode(mode) {
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

                                // Handle leap mode specially
                                let mode = self.current_mode();
                                if mode.is_leap() {
                                    // Check for Escape to cancel
                                    if key_str == "Escape" {
                                        self.leap_phase = LeapPhase::Inactive;
                                        self.leap_label.clear();
                                        self.dispatcher.send_leap_cancel().await;
                                        continue;
                                    }

                                    // Handle based on current leap phase
                                    match self.leap_phase {
                                        LeapPhase::Inactive => {
                                            // Just entered leap mode, set phase
                                            self.leap_phase = LeapPhase::WaitingFirstChar;
                                            // The first char will be processed on next iteration
                                            // Actually, let's send this char as first char
                                            if key_str.len() == 1
                                                && let Some(c) = key_str.chars().next()
                                            {
                                                self.leap_phase = LeapPhase::WaitingSecondChar;
                                                self.dispatcher.send_leap_first_char(c).await;
                                            }
                                            continue;
                                        }
                                        LeapPhase::WaitingFirstChar => {
                                            // Process first character
                                            if key_str.len() == 1
                                                && let Some(c) = key_str.chars().next()
                                            {
                                                self.leap_phase = LeapPhase::WaitingSecondChar;
                                                self.dispatcher.send_leap_first_char(c).await;
                                            }
                                            continue;
                                        }
                                        LeapPhase::WaitingSecondChar => {
                                            // Process second character
                                            if key_str.len() == 1
                                                && let Some(c) = key_str.chars().next()
                                            {
                                                self.leap_phase = LeapPhase::ShowingLabels;
                                                self.dispatcher.send_leap_second_char(c).await;
                                            }
                                            continue;
                                        }
                                        LeapPhase::ShowingLabels => {
                                            // Process label selection
                                            // Accumulate label characters for multi-char labels
                                            if key_str.len() == 1 {
                                                self.leap_label.push_str(&key_str);
                                                // Send the label (runtime will handle matching)
                                                self.dispatcher
                                                    .send_leap_select_label(self.leap_label.clone())
                                                    .await;
                                                self.leap_phase = LeapPhase::Inactive;
                                                self.leap_label.clear();
                                            }
                                            continue;
                                        }
                                    }
                                } else if self.leap_phase != LeapPhase::Inactive {
                                    // Mode changed, reset leap state
                                    self.leap_phase = LeapPhase::Inactive;
                                    self.leap_label.clear();
                                }

                                // Handle operator-pending mode (d, y, c + motion or text object)
                                let mode = self.current_mode();
                                if mode.is_operator_pending() {
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
                                        // Change actions enter Insert mode, others return to Normal
                                        let is_change_action = matches!(
                                            &action,
                                            OperatorMotionAction::Change { .. }
                                                | OperatorMotionAction::ChangeTextObject { .. }
                                                | OperatorMotionAction::ChangeWordTextObject { .. }
                                                | OperatorMotionAction::ChangeSemanticTextObject { .. }
                                        );
                                        let new_mode = if is_change_action {
                                            ModeState::insert()
                                        } else {
                                            ModeState::normal()
                                        };
                                        self.set_local_mode(new_mode);
                                        self.mode_locally_changed = true;
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

                                // Handle visual mode text object selection (viw, vi(, vif)
                                let mode = self.current_mode();
                                if mode.is_visual() {
                                    let (action, should_wait) = Self::handle_visual_text_object(
                                        &key_str,
                                        &self.pending_keys,
                                    );
                                    if let Some(action) = action {
                                        tracing::debug!(?action, "Visual text object selection detected");
                                        self.pending_keys.clear();
                                        self.count_parser.take(); // Consume count
                                        self.dispatcher.send_visual_text_object(action).await;
                                        self.dispatcher
                                            .send_pending_keys(self.pending_display())
                                            .await;
                                        continue;
                                    }
                                    if should_wait {
                                        // Waiting for text object specifier (i/a pressed)
                                        self.pending_keys.push_str(&key_str);
                                        self.dispatcher
                                            .send_pending_keys(self.pending_display())
                                            .await;
                                        continue;
                                    }
                                }

                                // Handle single-character input in insert/command/telescope modes
                                // Route via FocusInputEvent instead of creating inline commands
                                let mode = self.current_mode();
                                if (mode.is_insert() || mode.is_command() || mode.is_telescope_focus())
                                    && key_str.len() == 1
                                    && let Some(c) = key_str.chars().next()
                                {
                                    // Single printable character - route through focus system
                                    self.pending_keys.clear();
                                    self.dispatcher.send_focus_insert_char(c).await;
                                    self.dispatcher
                                        .send_pending_keys(self.pending_display())
                                        .await;
                                    continue;
                                }

                                // Handle Backspace in insert/command/telescope modes
                                // Route via FocusInputEvent for focus-based handling
                                let mode = self.current_mode();
                                if (mode.is_insert() || mode.is_command() || mode.is_telescope_focus())
                                    && key_str == "Backspace"
                                {
                                    self.pending_keys.clear();
                                    self.dispatcher.send_focus_delete_backward().await;
                                    self.dispatcher
                                        .send_pending_keys(self.pending_display())
                                        .await;
                                    continue;
                                }

                                // Handle Tab in insert mode when completion is not active
                                // Tab inserts a tab character via FocusInputEvent
                                let mode = self.current_mode();
                                if mode.is_insert()
                                    && key_str == "Tab"
                                    && !self.is_completion_active()
                                {
                                    self.pending_keys.clear();
                                    self.dispatcher.send_focus_insert_char('\t').await;
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

                                if let Some(ref cmd) = cmd {
                                    tracing::debug!(?cmd, count = ?self.count_parser.peek(), "Dispatching command");

                                    // Check for mode change commands and update local mode immediately
                                    // This avoids race conditions with the Runtime's watch channel
                                    if let Some(new_mode) = Dispatcher::mode_for_command(cmd) {
                                        self.set_local_mode(new_mode.clone());
                                        self.mode_locally_changed = true;
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
