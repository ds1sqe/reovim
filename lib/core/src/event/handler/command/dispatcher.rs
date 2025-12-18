//! Command dispatch and mode change handling

use {
    crate::{
        bind::CommandRef,
        command::{CommandContext, traits::OperatorMotionAction},
        event::{
            InnerEvent,
            inner::{CommandEvent, VisualTextObjectAction, WhichKeyBinding},
        },
        modd::{ModeState, OperatorType},
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
            "enter_normal_mode" => Some(ModeState::normal()),
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
            // Commands that return to Normal mode
            "command_line_execute" | "command_line_cancel" | "visual_delete" | "visual_yank" => {
                Some(ModeState::normal())
            }
            // Explorer mode transitions are handled in runtime via DeferredAction
            // toggle_explorer, explorer_close, explorer_focus_editor etc.
            _ => None,
        }
    }

    /// Send operator + motion action to runtime
    pub async fn send_operator_motion(&self, action: OperatorMotionAction) {
        // Also send mode change back to normal
        let _ = self
            .inner_tx
            .send(InnerEvent::ModeChangeEvent(ModeState::normal()))
            .await;
        let _ = self
            .inner_tx
            .send(InnerEvent::OperatorMotionEvent(action))
            .await;
    }

    /// Send event to show which-key popup
    pub async fn send_which_key_show(&self, prefix: String, bindings: Vec<WhichKeyBinding>) {
        let _ = self
            .inner_tx
            .send(InnerEvent::WhichKeyShow { prefix, bindings })
            .await;
    }

    /// Send event to hide which-key popup
    pub async fn send_which_key_hide(&self) {
        let _ = self.inner_tx.send(InnerEvent::WhichKeyHide).await;
    }

    /// Send leap first character event
    pub async fn send_leap_first_char(&self, c: char) {
        let _ = self
            .inner_tx
            .send(InnerEvent::LeapEvent(crate::event::LeapEvent::FirstChar { char: c }))
            .await;
    }

    /// Send leap second character event
    pub async fn send_leap_second_char(&self, c: char) {
        let _ = self
            .inner_tx
            .send(InnerEvent::LeapEvent(crate::event::LeapEvent::SecondChar { char: c }))
            .await;
    }

    /// Send leap label selection event
    pub async fn send_leap_select_label(&self, label: String) {
        let _ = self
            .inner_tx
            .send(InnerEvent::LeapEvent(crate::event::LeapEvent::SelectLabel { label }))
            .await;
    }

    /// Send leap cancel event
    pub async fn send_leap_cancel(&self) {
        let _ = self
            .inner_tx
            .send(InnerEvent::LeapEvent(crate::event::LeapEvent::Cancel))
            .await;
    }

    /// Send visual text object selection event
    pub async fn send_visual_text_object(&self, action: VisualTextObjectAction) {
        let _ = self
            .inner_tx
            .send(InnerEvent::VisualTextObjectEvent(action))
            .await;
    }
}
