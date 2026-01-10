//! Action handler methods for window, tab, buffer, and file operations

use crate::{
    command::traits::{BufferAction, FileAction, TabAction, WindowAction},
    modd::{ComponentId, ModeState, SubMode},
    screen::{NavigateDirection, SplitDirection},
};

use super::Runtime;

impl Runtime {
    /// Handle window-related deferred actions. Returns true if editor should quit.
    pub(crate) fn handle_window_action(&mut self, action: &WindowAction) -> bool {
        match action {
            WindowAction::SplitHorizontal { filename } => {
                self.handle_window_split(false, filename.as_ref());
            }
            WindowAction::SplitVertical { filename } => {
                self.handle_window_split(true, filename.as_ref());
            }
            WindowAction::Close { force } => {
                if self.handle_window_close(*force) {
                    return true;
                }
            }
            WindowAction::CloseOthers => {
                self.handle_window_only();
            }
            WindowAction::FocusDirection { direction } => {
                self.handle_window_navigate(*direction);
            }
            WindowAction::Resize { direction, delta } => {
                // Each unit represents 5% of the split ratio
                let ratio_delta = f32::from(*delta) * 0.05;
                self.screen.resize_window(*direction, ratio_delta);
            }
            WindowAction::Equalize => {
                self.handle_window_equalize();
            }
            WindowAction::SwapDirection { direction } => {
                self.screen.swap_window(*direction);
            }
        }

        // Exit window mode if we're in it
        self.exit_window_mode_if_active();

        false
    }

    /// Exit window mode if currently active
    fn exit_window_mode_if_active(&mut self) {
        if matches!(
            self.mode_state.sub_mode,
            SubMode::Interactor(id) if id == ComponentId::WINDOW
        ) {
            self.handle_mode_change(ModeState::normal());
        }
    }

    /// Handle tab-related deferred actions. Returns true if editor should quit.
    pub(crate) fn handle_tab_action(&mut self, action: &TabAction) -> bool {
        match action {
            TabAction::New { filename } => {
                self.handle_tab_new(filename.as_ref());
            }
            TabAction::Close => {
                if self.handle_tab_close() {
                    return true;
                }
            }
            TabAction::Next => {
                self.handle_tab_next();
            }
            TabAction::Prev => {
                self.handle_tab_prev();
            }
            TabAction::Goto { index } => {
                self.screen.goto_tab(*index);
            }
        }
        false
    }

    /// Handle window split command
    ///
    /// Saves the current window's cursor before splitting so the new window
    /// inherits the correct cursor position. See docs/window-buffer.md.
    pub(crate) fn handle_window_split(&mut self, vertical: bool, filename: Option<&String>) {
        let start = std::time::Instant::now();
        tracing::debug!("[SPLIT] handle_window_split START vertical={}", vertical);

        let direction = if vertical {
            SplitDirection::Vertical
        } else {
            SplitDirection::Horizontal
        };

        // If a filename is provided, open it
        if let Some(path) = filename {
            self.open_file(path);
        }

        // Get the current buffer ID (either the newly opened file or existing buffer)
        // Use self.active_buffer_id() since open_file updates it via screen
        let buffer_id = self.active_buffer_id();

        // CENTRALIZED: Save buffer cursor to window before split.
        // split_window() reads window.cursor to copy to new window.
        if self
            .screen
            .save_cursor_to_active_window(&self.buffers)
            .is_none()
        {
            tracing::warn!("[SPLIT] failed to save cursor to active window");
        }

        // Split the window
        if let Some(new_window_id) = self.screen.split_window(direction) {
            // Set the buffer for the new window
            self.screen.set_window_buffer(new_window_id, buffer_id);
            tracing::debug!(
                "[SPLIT] created new_win={} buffer={} at {:?}",
                new_window_id,
                buffer_id,
                start.elapsed()
            );
        }
        tracing::debug!("[SPLIT] handle_window_split DONE at {:?}", start.elapsed());
    }

    /// Handle window close command. Returns true if editor should quit.
    pub(crate) fn handle_window_close(&mut self, _force: bool) -> bool {
        // TODO: Check for unsaved changes if force is false
        self.screen.close_window()
    }

    /// Handle close all other windows (:only)
    pub(crate) fn handle_window_only(&mut self) {
        self.screen.close_other_windows();
    }

    /// Handle window navigation (focus direction)
    pub(crate) fn handle_window_navigate(&mut self, direction: NavigateDirection) {
        let start = std::time::Instant::now();

        // Handle plugin focus case: unfocus plugin and return
        if self.screen.has_plugin_focus() {
            self.screen.focus_editor();
            self.screen.update_window_active_state();
            tracing::debug!("[NAV] Unfocused plugin, focused editor at {:?}", start.elapsed());
            return;
        }

        // Find target window via direction lookup
        let target_id = {
            let Some(tab) = self.screen.tab_manager().active_tab() else {
                return;
            };
            let editor_layout = self.screen.layout().editor_layout();
            let editor_rect = crate::screen::WindowRect::new(
                editor_layout.anchor.x,
                editor_layout.anchor.y,
                editor_layout.width,
                editor_layout.height,
            );
            let layouts = tab.calculate_layouts(editor_rect);

            let Some(id) = crate::screen::layout::split::find_adjacent_window(
                tab.active_window_id,
                direction,
                &layouts,
            ) else {
                tracing::debug!("[NAV] No adjacent window in direction {:?}", direction);
                return;
            };
            id
        };

        // CENTRALIZED: Full cursor sync (save to old, load from new)
        let prev = self
            .screen
            .switch_active_window(target_id, &mut self.buffers);

        tracing::debug!(
            "[NAV] {:?}: win {:?} -> {} at {:?}",
            direction,
            prev,
            target_id,
            start.elapsed()
        );
    }

    /// Handle window equalize
    pub(crate) fn handle_window_equalize(&mut self) {
        self.screen.equalize_windows();
    }

    // === Tab Management Handlers ===

    /// Handle new tab command
    pub(crate) fn handle_tab_new(&mut self, filename: Option<&String>) {
        // If a filename is provided, open it
        if let Some(path) = filename {
            self.open_file(path);
        }

        // Get the current buffer ID
        // Use self.active_buffer_id() since open_file updates it via screen
        let buffer_id = self.active_buffer_id();

        let tab_id = self.screen.new_tab(buffer_id);
        tracing::info!(tab_id = tab_id, buffer_id = buffer_id, "New tab created");
    }

    /// Handle tab close command. Returns true if editor should quit.
    pub(crate) fn handle_tab_close(&mut self) -> bool {
        self.screen.close_tab()
    }

    /// Handle next tab command (gt)
    pub(crate) fn handle_tab_next(&mut self) {
        self.screen.next_tab();
    }

    /// Handle previous tab command (gT)
    pub(crate) fn handle_tab_prev(&mut self) {
        self.screen.prev_tab();
    }

    // === Buffer Navigation Handlers ===

    /// Handle buffer-related deferred actions
    pub(crate) fn handle_buffer_action(&mut self, action: &BufferAction) {
        match action {
            BufferAction::Prev => {
                if let Some(prev_id) = self.prev_buffer_id() {
                    self.switch_buffer(prev_id);
                    // Update window's buffer
                    if let Some(window_id) = self.screen.active_window_id() {
                        self.screen.set_window_buffer(window_id, prev_id);
                    }
                }
            }
            BufferAction::Next => {
                if let Some(next_id) = self.next_buffer_id() {
                    self.switch_buffer(next_id);
                    if let Some(window_id) = self.screen.active_window_id() {
                        self.screen.set_window_buffer(window_id, next_id);
                    }
                }
            }
            BufferAction::Delete { force: _ } => {
                // TODO: Check modified state if !force
                let buffer_id = self.active_buffer_id();
                self.close_buffer(buffer_id);
                // Update window to show the new active buffer
                if let Some(window_id) = self.screen.active_window_id() {
                    self.screen
                        .set_window_buffer(window_id, self.active_buffer_id());
                }
            }
        }
        self.request_render();
    }

    // ========================================================================
    // File Operations
    // ========================================================================

    /// Handle file operations from plugins
    ///
    /// This provides a way for plugins to request file operations that need
    /// Runtime access (e.g., opening files into buffers, creating files).
    pub(crate) fn handle_file_action(&mut self, action: &FileAction) {
        match action {
            FileAction::Open { path } => {
                self.open_file(path);
                self.screen.set_editor_buffer(self.active_buffer_id());
                // Switch focus to editor if a plugin has focus
                if self.screen.has_plugin_focus() {
                    self.screen.focus_editor();
                    self.set_mode(ModeState::normal());
                }
            }
            FileAction::Create { path, is_dir } => {
                let result = if *is_dir {
                    std::fs::create_dir_all(path)
                } else {
                    // Create parent directories if needed, then create file
                    if let Some(parent) = std::path::Path::new(path).parent() {
                        let _ = std::fs::create_dir_all(parent);
                    }
                    std::fs::File::create(path).map(|_| ())
                };
                match result {
                    Ok(()) => {
                        tracing::info!(path = %path, is_dir = is_dir, "File/directory created");
                    }
                    Err(e) => {
                        tracing::error!(path = %path, error = %e, "Failed to create file/directory");
                    }
                }
            }
            FileAction::Delete { path } => {
                let path = std::path::Path::new(path);
                let result = if path.is_dir() {
                    std::fs::remove_dir_all(path)
                } else {
                    std::fs::remove_file(path)
                };
                match result {
                    Ok(()) => {
                        tracing::info!(path = %path.display(), "File/directory deleted");
                    }
                    Err(e) => {
                        tracing::error!(path = %path.display(), error = %e, "Failed to delete file/directory");
                    }
                }
            }
            FileAction::Rename { from, to } => match std::fs::rename(from, to) {
                Ok(()) => {
                    tracing::info!(from = %from, to = %to, "File/directory renamed");
                }
                Err(e) => {
                    tracing::error!(from = %from, to = %to, error = %e, "Failed to rename");
                }
            },
        }
        self.request_render();
    }
}
