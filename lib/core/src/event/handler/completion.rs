//! Completion handler for auto-triggering completion on typing

use crate::{
    completion::trigger::{TriggerConfig, TriggerDetector, TriggerResult},
    event::{key::KeyCode, CompletionEvent, InnerEvent, KeyEvent, Subscribe},
    modd::Mod,
};
use tokio::sync::{broadcast::Receiver, mpsc::Sender, watch};

/// Handler that auto-triggers completion when typing in Insert mode
pub struct CompletionHandler {
    key_event_rx: Option<Receiver<KeyEvent>>,
    event_tx: Sender<InnerEvent>,
    mode_rx: watch::Receiver<Mod>,
    trigger_detector: TriggerDetector,
    /// Current prefix length (tracked from typing)
    prefix_len: usize,
}

impl Subscribe<KeyEvent> for CompletionHandler {
    fn subscribe(&mut self, rx: Receiver<KeyEvent>) {
        self.key_event_rx = Some(rx);
    }
}

impl CompletionHandler {
    /// Create a new completion handler
    #[must_use]
    pub fn new(
        tx: Sender<InnerEvent>,
        mode_rx: watch::Receiver<Mod>,
        config: TriggerConfig,
    ) -> Self {
        Self {
            key_event_rx: None,
            event_tx: tx,
            mode_rx,
            trigger_detector: TriggerDetector::new(config),
            prefix_len: 0,
        }
    }

    /// Create with default trigger config
    #[must_use]
    pub fn with_defaults(tx: Sender<InnerEvent>, mode_rx: watch::Receiver<Mod>) -> Self {
        Self::new(tx, mode_rx, TriggerConfig::default())
    }

    /// Get the current mode from the watch channel
    fn current_mode(&self) -> Mod {
        self.mode_rx.borrow().clone()
    }

    /// Check if we're in insert mode
    fn is_insert_mode(&self) -> bool {
        matches!(self.current_mode(), Mod::Insert(_))
    }

    /// Send a completion trigger event
    async fn trigger_completion(&self) {
        let _ = self
            .event_tx
            .send(InnerEvent::CompletionEvent(CompletionEvent::Trigger {
                buffer_id: 0, // TODO: support multiple buffers
            }))
            .await;
    }

    /// Send a completion dismiss event
    async fn dismiss_completion(&self) {
        let _ = self
            .event_tx
            .send(InnerEvent::CompletionEvent(CompletionEvent::Dismiss))
            .await;
    }

    /// Send a filter update event
    async fn update_filter(&self, new_prefix: String) {
        let _ = self
            .event_tx
            .send(InnerEvent::CompletionEvent(CompletionEvent::UpdateFilter {
                new_prefix,
            }))
            .await;
    }

    /// Check if a character is a word character (can be part of an identifier)
    fn is_word_char(ch: char) -> bool {
        ch.is_alphanumeric() || ch == '_'
    }

    /// Handle a key event
    async fn handle_key(&mut self, event: &KeyEvent) {
        // Only process in insert mode
        if !self.is_insert_mode() {
            self.prefix_len = 0;
            self.trigger_detector.clear();
            return;
        }

        match event.code {
            KeyCode::Char(ch) => {
                if Self::is_word_char(ch) {
                    // Continue building prefix
                    self.prefix_len += 1;

                    // Check trigger conditions
                    let result = self.trigger_detector.on_char_typed(ch, self.prefix_len);
                    if result == TriggerResult::TriggerNow {
                        self.trigger_completion().await;
                    }
                    // TriggerAfterDebounce and NoTrigger are handled by the timer below
                } else {
                    // Non-word character typed, reset prefix
                    self.prefix_len = 0;
                    self.trigger_detector.clear();
                    self.dismiss_completion().await;
                }
            }
            KeyCode::Backspace => {
                if self.prefix_len > 0 {
                    self.prefix_len -= 1;
                    if self.prefix_len == 0 {
                        self.trigger_detector.clear();
                        self.dismiss_completion().await;
                    } else {
                        // Update filter with shorter prefix (let runtime calculate actual prefix)
                        self.update_filter(String::new()).await;
                    }
                }
            }
            KeyCode::Esc | KeyCode::Enter => {
                // Leaving insert mode or confirming, reset
                self.prefix_len = 0;
                self.trigger_detector.clear();
            }
            _ => {
                // Other keys (arrows, etc.) - don't affect prefix tracking
            }
        }
    }

    /// Run the completion handler event loop
    #[allow(clippy::while_let_loop)]
    pub async fn run(mut self) {
        let Some(rx) = self.key_event_rx.take() else {
            return;
        };

        let mut rx = rx;
        let debounce_duration = self.trigger_detector.debounce_duration();

        loop {
            tokio::select! {
                // Handle key events
                result = rx.recv() => {
                    match result {
                        Ok(event) => {
                            self.handle_key(&event).await;
                        }
                        Err(_) => {
                            break;
                        }
                    }
                }

                // Check debounce timer
                () = tokio::time::sleep(debounce_duration), if self.trigger_detector.is_pending() => {
                    if self.trigger_detector.should_trigger_now() {
                        self.trigger_completion().await;
                        self.trigger_detector.clear();
                    }
                }
            }
        }
    }
}
