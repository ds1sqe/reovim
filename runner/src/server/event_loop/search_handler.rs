//! Search functionality for the event loop.
//!
//! Handles /, ?, n, N, *, # search commands and search input mode.

use {
    reovim_driver_input::{KeyCode, KeyEvent},
    reovim_kernel::api::v1::MotionEngine,
};

use {
    super::EventLoop,
    crate::server::{AppState, app::SearchDirection},
};

impl<F: reovim_driver_input::InputFallbackHandler<AppState>> EventLoop<F> {
    /// Handle a key event when in search input mode (typing pattern).
    ///
    /// Keys are buffered until Enter executes the search or Escape cancels.
    pub(super) fn handle_search_input(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Escape => {
                // Cancel search input
                self.app.search.cancel_input();
                self.app.clear_pending_keys();
            }
            KeyCode::Enter => {
                // Complete search input and execute
                if let Some((pattern, direction)) = self.app.search.complete_input() {
                    self.execute_search(&pattern, direction);
                }
                self.app.clear_pending_keys();
            }
            KeyCode::Backspace => {
                // Delete last character from input buffer
                self.app.search.input_buffer.pop();
            }
            KeyCode::Char(c) => {
                // Add character to input buffer
                self.app.search.input_buffer.push(c);
            }
            _ => {
                // Ignore other keys in search input mode
            }
        }
    }

    /// Handle a search action from search commands.
    pub(super) fn handle_search_action(&mut self, action: &reovim_driver_command::SearchAction) {
        use reovim_driver_command::SearchAction;

        match *action {
            SearchAction::EnterSearchMode { direction } => {
                let dir = match direction {
                    reovim_driver_command::SearchDirection::Forward => SearchDirection::Forward,
                    reovim_driver_command::SearchDirection::Backward => SearchDirection::Backward,
                };
                self.app.search.start_input(dir);
                self.last_error = None;
            }
            SearchAction::Next => {
                // Go to next match in same direction
                if let Some(pattern) = self.app.search.pattern.clone() {
                    let direction = self.app.search.direction;
                    self.execute_search(&pattern, direction);
                } else {
                    self.set_error("No previous search pattern");
                }
            }
            SearchAction::Previous => {
                // Go to previous match (reverse direction)
                if let Some(pattern) = self.app.search.pattern.clone() {
                    let direction = match self.app.search.direction {
                        SearchDirection::Forward => SearchDirection::Backward,
                        SearchDirection::Backward => SearchDirection::Forward,
                    };
                    self.execute_search(&pattern, direction);
                } else {
                    self.set_error("No previous search pattern");
                }
            }
            SearchAction::WordUnderCursor { direction } => {
                self.execute_word_search(direction);
            }
            SearchAction::ClearHighlight => {
                self.app.search.clear_highlight();
                self.last_error = None;
            }
        }
    }

    /// Execute search with pattern and direction.
    pub(super) fn execute_search(&mut self, pattern: &str, direction: SearchDirection) {
        use crate::search::{Direction, SearchEngine};

        let Some(buffer_id) = self.app.active_buffer else {
            self.set_error("No active buffer");
            return;
        };

        let Some(buffer_arc) = self.app.kernel.buffers.get(buffer_id) else {
            self.set_error("Buffer not found");
            return;
        };

        let search_dir = match direction {
            SearchDirection::Forward => Direction::Forward,
            SearchDirection::Backward => Direction::Backward,
        };

        let buffer = buffer_arc.read();
        let cursor_pos = buffer.cursor().position;

        match SearchEngine::find_next(&buffer, cursor_pos, pattern, search_dir, true) {
            Ok(Some(m)) => {
                let wrapped = match search_dir {
                    Direction::Forward => m.start < cursor_pos,
                    Direction::Backward => m.start > cursor_pos,
                };
                drop(buffer);
                buffer_arc.write().set_position(m.start);
                self.app.search.highlight_active = true;
                self.last_error = None;

                if wrapped {
                    let msg = match search_dir {
                        Direction::Forward => "search hit BOTTOM, continuing at TOP",
                        Direction::Backward => "search hit TOP, continuing at BOTTOM",
                    };
                    tracing::info!(msg);
                }
            }
            Ok(None) => {
                self.set_error("Pattern not found");
            }
            Err(e) => {
                self.set_error(e.to_string());
            }
        }
    }

    /// Execute word search (* or #).
    pub(super) fn execute_word_search(
        &mut self,
        direction: reovim_driver_command::SearchDirection,
    ) {
        use crate::search::SearchEngine;

        let Some(buffer_id) = self.app.active_buffer else {
            self.set_error("No active buffer");
            return;
        };

        let Some(buffer_arc) = self.app.kernel.buffers.get(buffer_id) else {
            self.set_error("Buffer not found");
            return;
        };

        let buffer = buffer_arc.read();
        let cursor_pos = buffer.cursor().position;

        let Some(word_pattern) = SearchEngine::word_at_cursor(&buffer, cursor_pos) else {
            self.set_error("No word under cursor");
            return;
        };
        drop(buffer);

        // Store pattern and direction
        self.app.search.pattern = Some(word_pattern.clone());
        self.app.search.direction = match direction {
            reovim_driver_command::SearchDirection::Forward => SearchDirection::Forward,
            reovim_driver_command::SearchDirection::Backward => SearchDirection::Backward,
        };

        // Execute search
        self.execute_search(&word_pattern, self.app.search.direction);
    }

    /// Execute a repeat find motion (; or ,).
    pub(super) fn execute_repeat_find(&mut self, reverse: bool) {
        let Some(last_find) = self.app.last_find else {
            // No previous find to repeat
            return;
        };

        let Some(buffer_id) = self.app.active_buffer else {
            self.set_error("No active buffer");
            return;
        };

        let Some(buffer_arc) = self.app.kernel.buffers.get(buffer_id) else {
            self.set_error("Buffer not found");
            return;
        };

        // Get motion (same or reversed direction)
        let motion = if reverse {
            last_find.reverse_motion()
        } else {
            last_find.repeat_motion()
        };

        // Calculate and apply motion
        let buffer = buffer_arc.read();
        let target = MotionEngine::calculate(&buffer, buffer.cursor(), motion, 1);
        drop(buffer);

        if let Some(pos) = target {
            buffer_arc.write().set_position(pos);
        }
        // Note: ; and , do NOT update last_find (per Vim behavior)

        self.app.clear_pending_keys();
    }
}
