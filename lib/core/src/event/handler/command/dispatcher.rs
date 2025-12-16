//! Command dispatch and mode change handling

use crate::bind::CommandRef;
use crate::command::CommandContext;
use crate::event::{inner::CommandEvent, InnerEvent};
use crate::modd::{Mod, ModExtension};
use tokio::sync::mpsc::Sender;

/// Handles command dispatch and mode transitions
pub struct Dispatcher {
    inner_tx: Sender<InnerEvent>,
    current_buffer_id: usize,
    current_window_id: usize,
}

impl Dispatcher {
    #[must_use]
    pub const fn new(
        tx: Sender<InnerEvent>,
        buffer_id: usize,
        window_id: usize,
    ) -> Self {
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
    pub async fn update_mode(&self, new_mode: Mod) {
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
    pub fn mode_for_command(cmd: &CommandRef) -> Option<Mod> {
        let name = match cmd {
            CommandRef::Registered(id) => id.as_str(),
            CommandRef::Inline(cmd) => cmd.name(),
        };

        match name {
            "enter_normal_mode" => Some(Mod::Normal),
            "enter_insert_mode" | "enter_insert_mode_after" | "enter_insert_mode_eol"
            | "open_line_below" | "open_line_above" => Some(Mod::Insert(ModExtension::Normal)),
            "enter_visual_mode" => Some(Mod::Visual(ModExtension::Normal)),
            "enter_command_mode" => Some(Mod::Command),
            // Commands that return to Normal mode
            "command_line_execute" | "command_line_cancel" | "visual_delete" | "visual_yank" => {
                Some(Mod::Normal)
            }
            _ => None,
        }
    }
}
