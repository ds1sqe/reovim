//! Main event loop for the editor

use crate::buffer::{Buffer, SelectionOps, TextOps};
use crate::event::{
    BufferEvent, CommandHandler, HighlightEvent, InnerEvent, InputEventBroker, TerminateHandler,
};
use crate::modd::{Mod, ModExtension};

use super::Runtime;

impl Runtime {
    /// Initialize and run the editor event loop
    #[allow(clippy::missing_panics_doc)]
    #[allow(clippy::too_many_lines)]
    #[allow(clippy::future_not_send)]
    #[allow(clippy::single_match_else)]
    #[allow(clippy::collapsible_if)]
    #[allow(clippy::match_same_arms)]
    pub async fn init(mut self) {
        let mut buffer = Buffer::empty(0);

        // Load file if provided, otherwise show landing page
        if let Some(ref path) = self.initial_file {
            if let Ok(content) = std::fs::read_to_string(path) {
                buffer.set_content(&content);
            }
            buffer.file_path = Some(path.clone());
        } else {
            // Show landing page when no file is opened
            let landing_content = crate::landing::generate(
                self.screen.width(),
                self.screen.height().saturating_sub(1), // Reserve status line
            );
            buffer.set_content(&landing_content);
            self.showing_landing_page = true;
        }

        self.buffers.insert(0, buffer);
        let input_broker = InputEventBroker::default();

        // Command handler for key-to-command translation
        // Pass mode receiver so CommandHandler can read mode from Runtime (single source of truth)
        let mode_rx = self.subscribe_mode();
        let mut command_hdr =
            CommandHandler::new(self.tx.clone(), mode_rx, self.command_registry.clone());
        let mut terminate_hdr = TerminateHandler::new(self.tx.clone());

        input_broker.key_broker.enlist(&mut command_hdr);
        input_broker.key_broker.enlist(&mut terminate_hdr);

        tokio::spawn(async move { command_hdr.run().await });
        tokio::spawn(async move { terminate_hdr.run().await });
        tokio::spawn(async move { input_broker.subscribe().await });

        // Initial render to show content immediately
        self.render();

        self.run_event_loop().await;

        let _ = self.screen.finalize();
    }

    /// The main event processing loop
    #[allow(clippy::collapsible_if)]
    #[allow(clippy::match_same_arms)]
    #[allow(clippy::future_not_send)]
    async fn run_event_loop(&mut self) {
        loop {
            let next = self.rx.recv().await;
            if let Some(ev) = next {
                if self.handle_event(ev) {
                    break;
                }
            } else {
                self.tx
                    .send(InnerEvent::KillSignal)
                    .await
                    .expect("cannot broadcast kill signal");
                break;
            }
        }
    }

    /// Handle a single event. Returns true if the editor should quit.
    #[allow(clippy::collapsible_if)]
    #[allow(clippy::match_same_arms)]
    fn handle_event(&mut self, ev: InnerEvent) -> bool {
        match ev {
            InnerEvent::BufferEvent(buffer_event) => match buffer_event {
                BufferEvent::SetContent { buffer_id, content } => {
                    if let Some(b) = self.buffers.get_mut(&buffer_id) {
                        b.set_content(&content);
                        self.render();
                    }
                }
            },
            InnerEvent::CommandEvent(cmd_event) => {
                if self.handle_command(cmd_event) {
                    return true;
                }
            }
            InnerEvent::ModeChangeEvent(new_mode) => {
                self.handle_mode_change(new_mode);
            }
            InnerEvent::PendingKeysEvent(keys) => {
                // If pending_keys is being cleared and had content, save as last_command
                if keys.is_empty() && !self.pending_keys.is_empty() {
                    self.last_command = self.pending_keys.clone();
                }
                self.pending_keys = keys;
                self.render();
            }
            InnerEvent::WindowEvent => todo!(),
            InnerEvent::HighlightEvent(hl_event) => match hl_event {
                HighlightEvent::Add {
                    buffer_id,
                    highlights,
                } => {
                    self.highlight_store.add(buffer_id, highlights);
                    self.render();
                }
                HighlightEvent::ClearGroup { buffer_id, group } => {
                    self.highlight_store.clear_group(buffer_id, group);
                    self.render();
                }
                HighlightEvent::ClearAll { buffer_id } => {
                    self.highlight_store.clear_all(buffer_id);
                    self.render();
                }
            },
            InnerEvent::RenderSignal => {
                self.render();
            }
            InnerEvent::KillSignal => {
                return true;
            }
            InnerEvent::WhichKeyShow { prefix, bindings } => {
                self.which_key_panel.show(prefix, bindings);
                self.render();
            }
            InnerEvent::WhichKeyHide => {
                self.which_key_panel.hide();
                self.render();
            }
        }
        false
    }

    /// Handle mode change events
    #[allow(clippy::collapsible_if)]
    fn handle_mode_change(&mut self, new_mode: Mod) {
        // Hide which-key panel on mode change
        self.which_key_panel.hide();

        match &new_mode {
            Mod::Insert(_) => {
                // Clear landing page content when entering insert mode (only once)
                if self.showing_landing_page {
                    if let Some(buffer) = self.buffers.get_mut(&0) {
                        buffer.contents.clear();
                        buffer.cur.x = 0;
                        buffer.cur.y = 0;
                    }
                    self.showing_landing_page = false;
                }
            }
            Mod::Visual(ext) => {
                // Start selection when entering visual mode
                if let Some(buffer) = self.buffers.get_mut(&0) {
                    match ext {
                        ModExtension::Block => buffer.start_block_selection(),
                        _ => buffer.start_selection(),
                    }
                }
            }
            Mod::Normal => {
                // Clear selection when returning to normal mode
                if let Some(buffer) = self.buffers.get_mut(&0) {
                    buffer.clear_selection();
                }
                // Note: command line is cleared in handle_command_line_command
                // after the command is executed, not here (to avoid race condition)
            }
            Mod::Command => {
                // Activate command line when entering command mode
                self.command_line.activate();
            }
        }
        // Use set_mode to broadcast via watch channel
        self.set_mode(new_mode);
        self.render();
    }
}
