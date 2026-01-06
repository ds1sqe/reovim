//! Command dispatch and mode change handling

use {
    crate::{
        bind::CommandRef,
        command::{CommandContext, traits::OperatorMotionAction},
        event::{
            InnerEvent,
            inner::{CommandEvent, VisualTextObjectAction},
        },
        modd::{ComponentId, ModeState, OperatorType, SubMode},
    },
    tokio::sync::mpsc::Sender,
};

/// Handles command dispatch and mode transitions
pub struct Dispatcher {
    inner_tx: Sender<InnerEvent>,
    current_buffer_id: usize,
    current_window_id: usize,
}

impl Dispatcher {
    #[must_use]
    pub const fn new(tx: Sender<InnerEvent>, buffer_id: usize, window_id: usize) -> Self {
        Self {
            inner_tx: tx,
            current_buffer_id: buffer_id,
            current_window_id: window_id,
        }
    }

    /// Get the sender for sending events
    #[must_use]
    #[allow(dead_code)]
    pub const fn sender(&self) -> &Sender<InnerEvent> {
        &self.inner_tx
    }

    /// Update the current mode and notify listeners
    pub async fn update_mode(&self, new_mode: ModeState) {
        let _ = self
            .inner_tx
            .send(InnerEvent::ModeChangeEvent(new_mode))
            .await;
    }

    /// Dispatch a command with the given count
    pub async fn dispatch(&self, cmd: CommandRef, count: Option<usize>) {
        let start = std::time::Instant::now();
        let ctx = CommandContext {
            buffer_id: self.current_buffer_id,
            window_id: self.current_window_id,
            count,
        };

        let _ = self
            .inner_tx
            .send(InnerEvent::CommandEvent(CommandEvent {
                command: cmd.clone(),
                context: ctx,
            }))
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

    /// Send pending keys display update
    pub async fn send_pending_keys(&self, display: String) {
        let _ = self
            .inner_tx
            .send(InnerEvent::PendingKeysEvent(display))
            .await;
    }

    /// Determine the new mode after a command, if any
    ///
    /// Uses command names to detect mode-changing commands.
    #[must_use]
    pub fn mode_for_command(cmd: &CommandRef) -> Option<ModeState> {
        let name = match cmd {
            CommandRef::Registered(id) => id.as_str(),
            CommandRef::Inline(cmd) => cmd.name(),
        };

        match name {
            // Commands that return to Normal mode (Editor focus)
            // NOTE: visual_delete and visual_yank are NOT here because they need
            // to read the selection BEFORE mode changes to Normal (which clears selection).
            // They handle mode transition via CommandResult::ClipboardWrite.
            "enter_normal_mode" | "command_line_execute" | "command_line_cancel" => {
                Some(ModeState::normal())
            }
            "enter_insert_mode"
            | "enter_insert_mode_after"
            | "enter_insert_mode_eol"
            | "open_line_below"
            | "open_line_above" => Some(ModeState::insert()),
            "enter_visual_mode" => Some(ModeState::visual()),
            "enter_visual_block_mode" => Some(ModeState::visual_block()),
            "enter_command_mode" => Some(ModeState::command()),
            // Operator-pending mode
            "enter_delete_operator" => {
                Some(ModeState::operator_pending(OperatorType::Delete, None))
            }
            "enter_yank_operator" => Some(ModeState::operator_pending(OperatorType::Yank, None)),
            "enter_change_operator" => {
                Some(ModeState::operator_pending(OperatorType::Change, None))
            }
            // Window mode (Ctrl-W)
            "enter_window_mode" => {
                Some(ModeState::new().with_sub(SubMode::Interactor(ComponentId::WINDOW)))
            }
            // Window mode actions -> Normal mode
            "window_mode_focus_left"
            | "window_mode_focus_down"
            | "window_mode_focus_up"
            | "window_mode_focus_right"
            | "window_mode_move_left"
            | "window_mode_move_down"
            | "window_mode_move_up"
            | "window_mode_move_right"
            | "window_mode_swap_left"
            | "window_mode_swap_down"
            | "window_mode_swap_up"
            | "window_mode_swap_right"
            | "window_mode_split_h"
            | "window_mode_split_v"
            | "window_mode_close"
            | "window_mode_only"
            | "window_mode_equalize" => Some(ModeState::normal()),
            // Plugin mode transitions (telescope, explorer) are handled via DeferredAction
            // toggle_explorer, explorer_close, explorer_focus_editor etc.
            _ => None,
        }
    }

    /// Send operator + motion action to runtime
    pub async fn send_operator_motion(&self, action: OperatorMotionAction) {
        // Change actions enter Insert mode, others return to Normal
        let new_mode = match &action {
            OperatorMotionAction::Change { .. }
            | OperatorMotionAction::ChangeTextObject { .. }
            | OperatorMotionAction::ChangeWordTextObject { .. }
            | OperatorMotionAction::ChangeSemanticTextObject { .. } => ModeState::insert(),
            _ => ModeState::normal(),
        };
        let _ = self
            .inner_tx
            .send(InnerEvent::ModeChangeEvent(new_mode))
            .await;
        let _ = self
            .inner_tx
            .send(InnerEvent::OperatorMotionEvent(action))
            .await;
    }

    /// Send visual text object selection event
    pub async fn send_visual_text_object(&self, action: VisualTextObjectAction) {
        let _ = self
            .inner_tx
            .send(InnerEvent::VisualTextObjectEvent(action))
            .await;
    }

    /// Send focus input event for character insertion
    pub async fn send_focus_insert_char(&self, c: char) {
        let _ = self
            .inner_tx
            .send(InnerEvent::TextInputEvent(crate::event::TextInputEvent::InsertChar(c)))
            .await;
    }

    /// Send focus input event for character deletion (backspace)
    pub async fn send_focus_delete_backward(&self) {
        let _ = self
            .inner_tx
            .send(InnerEvent::TextInputEvent(crate::event::TextInputEvent::DeleteCharBackward))
            .await;
    }
}
