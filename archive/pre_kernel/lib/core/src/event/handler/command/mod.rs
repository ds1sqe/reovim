//! Command handler for translating key events to editor commands

mod count_parser;
mod dispatcher;
mod key_parser;

pub use {count_parser::CountParser, dispatcher::Dispatcher, key_parser::key_to_string};

use {
    crate::{
        bind::{CommandRef, KeyMap, KeyMapInner},
        command::{CommandRegistry, traits::OperatorMotionAction},
        event::{RuntimeEvent, ScopedKeyEvent, Subscribe, VisualTextObjectAction},
        event_bus::EventScope,
        interactor::InteractorRegistry,
        keystroke::{KeyNotationFormat, KeySequence, Keystroke},
        modd::{ModeState, OperatorType, SubMode},
        motion::Motion,
        textobject::{
            Delimiter, SemanticTextObject, SemanticTextObjectSpec, TextObject, TextObjectScope,
            WordTextObject, WordType,
        },
    },
    std::{sync::Arc, time::Duration},
    tokio::sync::{broadcast::Receiver, mpsc::Sender, watch},
};

/// Handler that translates key events to commands based on current mode
pub struct CommandHandler {
    key_event_rx: Option<Receiver<ScopedKeyEvent>>,
    keymap: KeyMap,
    /// Watch receiver for mode changes from Runtime
    mode_rx: watch::Receiver<ModeState>,
    /// Local mode state for immediate mode tracking (avoids race conditions)
    local_mode: ModeState,
    pending_keys: KeySequence,
    count_parser: CountParser,
    dispatcher: Dispatcher,
    /// Track when mode was locally changed (avoids race condition with `mode_rx` sync)
    mode_locally_changed: bool,
    /// Interactor registry for input behavior lookup
    interactor_registry: Arc<InteractorRegistry>,
    /// Current scope for event tracking (set per-key, passed to dispatcher)
    current_scope: Option<EventScope>,
}

impl Subscribe<ScopedKeyEvent> for CommandHandler {
    fn subscribe(&mut self, rx: Receiver<ScopedKeyEvent>) {
        self.key_event_rx = Some(rx);
    }
}

impl CommandHandler {
    #[must_use]
    pub fn new(
        tx: Sender<RuntimeEvent>,
        mode_rx: watch::Receiver<ModeState>,
        keymap: KeyMap,
        command_registry: Arc<CommandRegistry>,
        interactor_registry: Arc<InteractorRegistry>,
    ) -> Self {
        let initial_mode = mode_rx.borrow().clone();
        Self {
            key_event_rx: None,
            keymap,
            mode_rx,
            local_mode: initial_mode,
            pending_keys: KeySequence::new(),
            count_parser: CountParser::new(),
            dispatcher: Dispatcher::new(tx, 0, 0, command_registry),
            mode_locally_changed: false,
            interactor_registry,
            current_scope: None,
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

    /// Lookup a binding with fallback support
    /// Also checks runtime mode if different from local mode (handles race conditions
    /// where focus change hasn't propagated to `local_mode` yet)
    fn lookup_binding(&self, keys: &KeySequence) -> Option<&KeyMapInner> {
        // First try local mode
        if let Some(inner) = self.keymap.lookup_binding(&self.local_mode, keys) {
            return Some(inner);
        }

        // If local mode failed and runtime mode is different, try runtime mode
        // This is a defensive fallback for edge cases where local_mode hasn't synced yet
        {
            let runtime_mode = self.mode_rx.borrow();
            if runtime_mode.interactor_id != self.local_mode.interactor_id {
                return self.keymap.lookup_binding(&runtime_mode, keys);
            }
        }

        None
    }

    /// Check if keys are a valid prefix (with fallback support)
    fn is_valid_prefix(&self, keys: &KeySequence) -> bool {
        self.keymap.is_valid_prefix(&self.local_mode, keys)
    }

    /// Handle operator-pending mode specially
    /// Returns Some(action) if the key triggers an operator, None otherwise
    /// Returns None with `should_wait=true` if waiting for more keys (text object)
    #[allow(clippy::too_many_lines)]
    fn handle_operator_pending(
        &self,
        key: &str,
        count: Option<usize>,
        pending: &KeySequence,
    ) -> (Option<OperatorMotionAction>, bool) {
        use crate::keystroke::Key;

        let mode = self.current_mode();
        if let SubMode::OperatorPending {
            operator,
            count: op_count,
        } = &mode.sub_mode
        {
            // Calculate total count (operator_count * motion_count)
            let _total_count = op_count.unwrap_or(1) * count.unwrap_or(1);

            // Check if pending ends with 'i' or 'a' (for text object scope)
            let pending_ends_with_i = pending
                .last()
                .is_some_and(|k| k.key == Key::Char('i') && k.modifiers.is_empty());
            let pending_ends_with_a = pending
                .last()
                .is_some_and(|k| k.key == Key::Char('a') && k.modifiers.is_empty());

            // Check for text object completion: pending ends with "i" or "a", key is delimiter or semantic
            if (pending_ends_with_i || pending_ends_with_a)
                && key.len() == 1
                && let Some(obj_char) = key.chars().next()
            {
                let scope = if pending_ends_with_i {
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
        pending: &KeySequence,
    ) -> (Option<VisualTextObjectAction>, bool) {
        use crate::keystroke::Key;

        // Check if pending ends with 'i' or 'a' (for text object scope)
        let pending_ends_with_i = pending
            .last()
            .is_some_and(|k| k.key == Key::Char('i') && k.modifiers.is_empty());
        let pending_ends_with_a = pending
            .last()
            .is_some_and(|k| k.key == Key::Char('a') && k.modifiers.is_empty());

        // Check for text object completion: pending ends with "i" or "a", key is delimiter/word/semantic
        if (pending_ends_with_i || pending_ends_with_a)
            && key.len() == 1
            && let Some(obj_char) = key.chars().next()
        {
            let scope = if pending_ends_with_i {
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
        tracing::info!(
            "Key lookup: interactor_id={}, key={}, pending={}",
            self.local_mode.interactor_id.0,
            key,
            self.pending_keys
        );

        // Check for exact match (with fallback to DefaultNormal for plugin windows)
        if let Some(inner) = self.lookup_binding(&self.pending_keys.clone()) {
            tracing::info!(
                "Found binding for key={}, has_command={}",
                &self.pending_keys,
                inner.command.is_some()
            );
            if inner.command.is_some() {
                let cmd = inner.command.clone();
                self.pending_keys.clear();
                return (cmd, true);
            }
            // Has children, wait for more keys
            return (None, true);
        }

        tracing::info!("No exact binding found for key={}", &self.pending_keys);

        // Check if current pending_keys is a valid prefix for any binding
        // (with fallback to DefaultNormal for plugin windows)
        if self.is_valid_prefix(&self.pending_keys.clone()) {
            // Valid prefix, wait for more keys (which-key will trigger on timeout)
            return (None, true);
        }

        // No match and not a valid prefix
        // Note: Single-character input in insert/command modes is now handled earlier
        // in run() via TextInputEvent, so this code path is only reached for special keys
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
        // Use KeySequence's render method for status line format
        display.push_str(&self.pending_keys.render(KeyNotationFormat::StatusLine));
        display
    }

    /// Check if mode is one where ESC should only clear pending state (not trigger keymap lookup)
    /// Visual mode is excluded because it has an Escape binding in the keymap to exit to Normal
    /// Non-editor interactors handle their own Escape via keymap
    fn is_esc_clearable_mode(mode: &ModeState) -> bool {
        let is_editor = mode.interactor_id.0 == "editor";
        // Editor Normal mode or OperatorPending - ESC clears pending
        // Non-editor interactors and visual mode have their own ESC handlers
        (mode.is_normal() && is_editor) || mode.is_operator_pending()
    }

    /// Check if mode is one where Backspace should edit pending keys
    const fn is_backspace_editable_mode(mode: &ModeState) -> bool {
        // Normal or Visual mode in editor - Backspace edits pending keys
        mode.is_normal() || mode.is_visual()
    }

    #[allow(clippy::while_let_loop)]
    #[allow(clippy::match_same_arms)]
    #[allow(clippy::too_many_lines)]
    #[allow(clippy::cognitive_complexity)]
    pub async fn run(mut self) {
        if let Some(rx) = self.key_event_rx.take() {
            let mut rx = rx;
            loop {
                // Use long timeout - no time-sensitive features
                let timeout = Duration::from_secs(3600);

                tokio::select! {
                    // Key event received
                    result = rx.recv() => {
                        match result {
                            Ok(mut scoped_event) => {
                                // Extract key and scope from ScopedKeyEvent
                                let event = scoped_event.key;
                                // Store scope for this key - will be consumed by dispatcher or
                                // decremented at end of processing if not dispatched
                                self.current_scope = scoped_event.take_scope();

                                let key_start = std::time::Instant::now();
                                let key_str = key_to_string(&event);
                                if key_str.is_empty() {
                                    // Decrement scope - no dispatch for empty key
                                    if let Some(scope) = self.current_scope.take() {
                                        scope.decrement();
                                    }
                                    continue;
                                }
                                // Convert KeyEvent to Keystroke for pending_keys
                                let keystroke = Keystroke::from(&event);

                                tracing::trace!("[RTT] CommandHandler: key={} received at {:?}", key_str, key_start);

                                // Sync local mode from Runtime's watch channel
                                // This catches mode changes initiated by Runtime (e.g., explorer focus, plugin mode changes)
                                // Allow syncing even from OperatorPending if runtime mode is different
                                // (e.g., when plugin changes from OperatorPending to Interactor)
                                let runtime_mode = self.mode_rx.borrow().clone();
                                let should_sync = runtime_mode != self.local_mode;

                                // Clear stale flag when runtime caught up
                                if !should_sync && self.mode_locally_changed {
                                    self.mode_locally_changed = false;
                                }

                                if should_sync {
                                    // If interactor changed, always sync (focus change initiated by runtime)
                                    let interactor_changed = runtime_mode.interactor_id != self.local_mode.interactor_id;

                                    if interactor_changed {
                                        // Focus change - always sync and clear mode_locally_changed flag
                                        self.local_mode = runtime_mode;
                                        self.mode_locally_changed = false;
                                    } else if self.mode_locally_changed {
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

                                // Check if this is a count digit
                                let mode = self.current_mode();
                                if self.count_parser.is_count_digit(&key_str, mode) {
                                    self.count_parser.accumulate(&key_str);
                                    self.dispatcher
                                        .send_pending_keys(self.pending_display())
                                        .await;
                                    // Decrement scope - no dispatch for count digit
                                    if let Some(scope) = self.current_scope.take() {
                                        scope.decrement();
                                    }
                                    continue;
                                }

                                // Handle Escape in Normal/Visual/Explorer/OperatorPending modes:
                                // - First check if there's a keymap binding for Escape
                                // - If yes, let normal key handling process it
                                // - If no, clear pending keys and count
                                // - In OperatorPending: cancel operator and return to Normal
                                if key_str == "Escape" {
                                    let mode = self.current_mode().clone();
                                    if Self::is_esc_clearable_mode(&mode) {
                                        // Check if there's a binding for just Escape (e.g., which-key close)
                                        let escape_seq = KeySequence::from_vec(vec![keystroke.clone()]);
                                        let has_escape_binding = self.lookup_binding(&escape_seq)
                                            .is_some_and(|inner| inner.command.is_some());

                                        // If no Escape binding, do the special handling
                                        if !has_escape_binding {
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
                                                self.dispatcher.update_mode(normal, self.current_scope.take()).await;
                                            } else {
                                                // Decrement scope - no dispatch for non-op-pending Escape
                                                if let Some(scope) = self.current_scope.take() {
                                                    scope.decrement();
                                                }
                                            }
                                            continue;
                                        }
                                        // If there IS a binding, fall through to normal key handling
                                    }
                                }

                                // NOTE: Backspace handling moved to AFTER the normal keybinding lookup.
                                // This ensures that plugins (like explorer) that bind Backspace get
                                // their bindings checked first, before applying editor-specific behavior.
                                // The editor-specific "edit pending keys" is now a FALLBACK in the
                                // "no command found" section at line ~710.

                                // Handle operator-pending mode (d, y, c + motion or text object)
                                // BUT: First check if there's a registered command for this key
                                // This allows plugin commands (like jump) to override built-in motions
                                let mode = self.current_mode();
                                if mode.is_operator_pending() {
                                    // Peek at command binding to see if a plugin command is registered
                                    let (cmd_peek, _) = self.lookup_command_no_push(&key_str);
                                    let has_registered_command = cmd_peek.is_some();

                                    // Only use built-in motion handling if NO plugin command is registered
                                    if !has_registered_command {
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
                                            self.dispatcher.send_operator_motion(action, self.current_scope.take()).await;
                                            self.dispatcher
                                                .send_pending_keys(self.pending_display())
                                                .await;
                                            continue;
                                        }
                                        if should_wait {
                                            // Waiting for text object delimiter (i/a pressed)
                                            self.pending_keys.push(keystroke.clone());
                                            self.dispatcher
                                                .send_pending_keys(self.pending_display())
                                                .await;
                                            // Decrement scope - no dispatch while waiting for text object
                                            if let Some(scope) = self.current_scope.take() {
                                                scope.decrement();
                                            }
                                            continue;
                                        }
                                    }
                                    // If has_registered_command, fall through to normal command dispatch
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
                                        self.dispatcher.send_visual_text_object(action, self.current_scope.take()).await;
                                        self.dispatcher
                                            .send_pending_keys(self.pending_display())
                                            .await;
                                        continue;
                                    }
                                    if should_wait {
                                        // Waiting for text object specifier (i/a pressed)
                                        self.pending_keys.push(keystroke.clone());
                                        self.dispatcher
                                            .send_pending_keys(self.pending_display())
                                            .await;
                                        // Decrement scope - no dispatch while waiting for text object
                                        if let Some(scope) = self.current_scope.take() {
                                            scope.decrement();
                                        }
                                        continue;
                                    }
                                }

                                // Handle single-character input in modes that accept char input
                                // Route via TextInputEvent instead of creating inline commands
                                let mode = self.current_mode();
                                if mode.accepts_char_input_with(&self.interactor_registry)
                                    && key_str.len() == 1
                                    && let Some(c) = key_str.chars().next()
                                {
                                    // Single printable character - route through focus system
                                    self.pending_keys.clear();
                                    self.dispatcher.send_focus_insert_char(c, self.current_scope.take()).await;
                                    self.dispatcher
                                        .send_pending_keys(self.pending_display())
                                        .await;
                                    continue;
                                }

                                // Handle Backspace in modes that accept char input
                                // Route via TextInputEvent for text input handling
                                let mode = self.current_mode();
                                if mode.accepts_char_input_with(&self.interactor_registry)
                                    && key_str == "Backspace"
                                {
                                    self.pending_keys.clear();
                                    self.dispatcher.send_focus_delete_backward(self.current_scope.take()).await;
                                    self.dispatcher
                                        .send_pending_keys(self.pending_display())
                                        .await;
                                    continue;
                                }

                                // Show the key being pressed (before lookup clears it)
                                self.pending_keys.push(keystroke);
                                self.dispatcher
                                    .send_pending_keys(self.pending_display())
                                    .await;

                                // Now do the lookup (which may clear pending_keys)
                                let (cmd, _) = self.lookup_command_no_push(&key_str);

                                if let Some(ref cmd) = cmd {
                                    tracing::debug!(?cmd, count = ?self.count_parser.peek(), "Dispatching command");

                                    // Check for mode change commands and update local mode immediately
                                    // This avoids race conditions with the Runtime's watch channel
                                    if let Some(new_mode) = self.dispatcher.mode_for_command(cmd) {
                                        tracing::debug!(
                                            "[MODE] Immediate mode update: {:?} -> {:?}",
                                            self.current_mode().sub_mode,
                                            new_mode.sub_mode
                                        );
                                        self.set_local_mode(new_mode.clone());
                                        self.mode_locally_changed = true;
                                        // Mode change doesn't need scope - dispatch gets the scope
                                        self.dispatcher.update_mode(new_mode, None).await;
                                    }

                                    // Dispatch the command with count (consumes scope)
                                    let count = self.count_parser.take();
                                    self.dispatcher.dispatch(cmd.clone(), count, self.current_scope.take()).await;

                                    // Clear display after command execution
                                    self.dispatcher
                                        .send_pending_keys(self.pending_display())
                                        .await;
                                } else {
                                    // No keybinding found - handle fallbacks
                                    let mode = self.current_mode();

                                    // Backspace fallback: Edit pending keys in editor Normal/Visual mode
                                    // This is checked AFTER keybinding lookup, so plugin bindings take precedence
                                    if key_str == "Backspace" && Self::is_backspace_editable_mode(mode) {
                                        // Only in editor mode, remove last keystroke from pending
                                        let is_editor = mode.interactor_id.0 == "editor";
                                        if is_editor && !self.pending_keys.is_empty() {
                                            self.pending_keys.pop();
                                            self.dispatcher
                                                .send_pending_keys(self.pending_display())
                                                .await;
                                        }
                                        // For non-editor modes with no binding, just ignore the Backspace
                                    } else if mode.is_insert() && key_str == "Tab" {
                                        // Tab fallback in insert mode
                                        self.pending_keys.clear();
                                        self.dispatcher.send_focus_insert_char('\t', self.current_scope.take()).await;
                                        self.dispatcher
                                            .send_pending_keys(self.pending_display())
                                            .await;
                                    }
                                }

                                // Decrement scope if it wasn't consumed by a dispatch
                                if let Some(scope) = self.current_scope.take() {
                                    scope.decrement();
                                }
                            }
                            Err(e) => {
                                tracing::debug!(error = %e, "Key event channel closed");
                                break;
                            }
                        }
                    }
                    // Timeout elapsed - no action needed
                    () = tokio::time::sleep(timeout) => {}
                }
            }
        }
    }
}
