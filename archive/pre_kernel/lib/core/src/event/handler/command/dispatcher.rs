//! Command dispatch and mode change handling

use {
    crate::{
        bind::CommandRef,
        command::{CommandContext, CommandRegistry, traits::OperatorMotionAction},
        event::{
            RuntimeEvent,
            inner::{CommandEvent, VisualTextObjectAction},
        },
        event_bus::EventScope,
        modd::ModeState,
    },
    std::sync::Arc,
    tokio::sync::mpsc::Sender,
};

/// Handles command dispatch and mode transitions
pub struct Dispatcher {
    inner_tx: Sender<RuntimeEvent>,
    current_buffer_id: usize,
    current_window_id: usize,
    command_registry: Arc<CommandRegistry>,
}

impl Dispatcher {
    #[must_use]
    pub const fn new(
        tx: Sender<RuntimeEvent>,
        buffer_id: usize,
        window_id: usize,
        command_registry: Arc<CommandRegistry>,
    ) -> Self {
        Self {
            inner_tx: tx,
            current_buffer_id: buffer_id,
            current_window_id: window_id,
            command_registry,
        }
    }

    /// Get the sender for sending events
    #[must_use]
    #[allow(dead_code)]
    pub const fn sender(&self) -> &Sender<RuntimeEvent> {
        &self.inner_tx
    }

    /// Helper to send event with optional scope (consumes scope)
    async fn send_with_scope(&self, event: RuntimeEvent, scope: Option<EventScope>) {
        let event = match scope {
            Some(s) => event.with_scope(s),
            None => event,
        };
        let _ = self.inner_tx.send(event).await;
    }

    /// Update the current mode and notify listeners (consumes scope)
    pub async fn update_mode(&self, new_mode: ModeState, scope: Option<EventScope>) {
        self.send_with_scope(RuntimeEvent::mode_change(new_mode), scope)
            .await;
    }

    /// Dispatch a command with the given count (consumes scope)
    pub async fn dispatch(&self, cmd: CommandRef, count: Option<usize>, scope: Option<EventScope>) {
        let start = std::time::Instant::now();
        let ctx = CommandContext {
            buffer_id: self.current_buffer_id,
            window_id: self.current_window_id,
            count,
        };

        self.send_with_scope(
            RuntimeEvent::command(CommandEvent {
                command: cmd.clone(),
                context: ctx,
            }),
            scope,
        )
        .await;
        tracing::trace!(
            "[RTT] Dispatcher.dispatch: cmd={:?} send took {:?}",
            match &cmd {
                CommandRef::Registered(id) => id.as_str(),
                CommandRef::Inline(c) => c.name(),
            },
            start.elapsed()
        );
    }

    /// Send pending keys display update (no scope - pure UI event)
    pub async fn send_pending_keys(&self, display: String) {
        let _ = self
            .inner_tx
            .send(RuntimeEvent::pending_keys(display))
            .await;
    }

    /// Determine the new mode after a command, if any
    ///
    /// Queries the command's `resulting_mode()` method via the command registry.
    #[must_use]
    pub fn mode_for_command(&self, cmd: &CommandRef) -> Option<ModeState> {
        match cmd {
            CommandRef::Registered(id) => self
                .command_registry
                .get(id)
                .and_then(|c| c.resulting_mode()),
            CommandRef::Inline(cmd) => cmd.resulting_mode(),
        }
    }

    /// Send operator + motion action to runtime (consumes scope)
    pub async fn send_operator_motion(
        &self,
        action: OperatorMotionAction,
        scope: Option<EventScope>,
    ) {
        // Change actions enter Insert mode, others return to Normal
        let new_mode = match &action {
            OperatorMotionAction::Change { .. }
            | OperatorMotionAction::ChangeTextObject { .. }
            | OperatorMotionAction::ChangeWordTextObject { .. }
            | OperatorMotionAction::ChangeSemanticTextObject { .. } => ModeState::insert(),
            _ => ModeState::normal(),
        };
        // Mode change doesn't need scope - operator_motion is the primary event
        self.send_with_scope(RuntimeEvent::mode_change(new_mode), None)
            .await;
        self.send_with_scope(RuntimeEvent::operator_motion(action), scope)
            .await;
    }

    /// Send visual text object selection event (consumes scope)
    pub async fn send_visual_text_object(
        &self,
        action: VisualTextObjectAction,
        scope: Option<EventScope>,
    ) {
        self.send_with_scope(RuntimeEvent::visual_text_object(action), scope)
            .await;
    }

    /// Send focus input event for character insertion (consumes scope)
    pub async fn send_focus_insert_char(&self, c: char, scope: Option<EventScope>) {
        self.send_with_scope(
            RuntimeEvent::text_input(crate::event::TextInputEvent::InsertChar(c)),
            scope,
        )
        .await;
    }

    /// Send focus input event for character deletion (backspace) (consumes scope)
    pub async fn send_focus_delete_backward(&self, scope: Option<EventScope>) {
        self.send_with_scope(
            RuntimeEvent::text_input(crate::event::TextInputEvent::DeleteCharBackward),
            scope,
        )
        .await;
    }
}
