use crate::{
    buffer::{Buffer, TextOps},
    event_bus::{BufferClosed, FileOpened},
};

use super::Runtime;

impl Runtime {
    /// Create a new empty buffer and return its ID
    pub fn create_buffer(&mut self) -> usize {
        let id = self.next_buffer_id;
        self.next_buffer_id += 1;
        let buffer = Buffer::empty(id);
        self.buffers.insert(id, buffer);
        id
    }

    /// Create a new buffer from a file and return its ID
    /// Returns None if the file cannot be read
    pub fn create_buffer_from_file(&mut self, path: &str) -> Option<usize> {
        tracing::debug!(path, "create_buffer_from_file: attempting to read");
        match std::fs::read_to_string(path) {
            Ok(content) => {
                let id = self.next_buffer_id;
                self.next_buffer_id += 1;
                let mut buffer = Buffer::empty(id);
                buffer.set_content(&content);
                buffer.file_path = Some(path.to_string());

                // Try to create syntax provider from factory (if available)
                if let Some(factory) = self.plugin_state.syntax_factory()
                    && let Some(mut syntax) = factory.create_syntax(id, path, &content)
                {
                    // Do initial parse
                    syntax.parse(&content);
                    buffer.attach_syntax(syntax);
                    tracing::debug!(id, path, "create_buffer_from_file: attached syntax provider");
                }

                // Try to create decoration provider from factory (if available)
                if let Some(factory) = self.plugin_state.decoration_factory()
                    && let Some(mut decorator) = factory.create_provider(path, &content)
                {
                    // Initial refresh to parse content
                    decorator.refresh(&content);
                    buffer.attach_decoration_provider(decorator);
                    tracing::debug!(
                        id,
                        path,
                        "create_buffer_from_file: attached decoration provider"
                    );
                }

                // Start background saturator if syntax/decoration providers exist
                // Saturator sends render signals which are low-priority events
                if buffer.has_syntax() || buffer.has_decoration_provider() {
                    buffer.start_saturator(self.lo_tx.clone());
                    tracing::debug!(id, path, "create_buffer_from_file: started saturator");
                }

                self.buffers.insert(id, buffer);

                // Emit FileOpened event for plugins that need to know about new files
                // Use emit_event to propagate scope for deterministic completion tracking
                self.emit_event(FileOpened {
                    buffer_id: id,
                    path: path.to_string(),
                });
                tracing::debug!(id, path, "create_buffer_from_file: emitted FileOpened event");

                tracing::debug!(id, path, "create_buffer_from_file: success");
                Some(id)
            }
            Err(e) => {
                tracing::debug!(path, error = %e, "create_buffer_from_file: failed to read file");
                None
            }
        }
    }

    /// Switch to a different buffer by ID
    pub fn switch_buffer(&mut self, buffer_id: usize) {
        if self.buffers.contains_key(&buffer_id) {
            self.screen.set_editor_buffer(buffer_id);
        }
    }

    /// Close a buffer by ID
    /// Returns true if buffer was closed, false if it was the last buffer
    pub fn close_buffer(&mut self, buffer_id: usize) -> bool {
        // Don't close the last buffer
        if self.buffers.len() <= 1 {
            return false;
        }

        if self.buffers.remove(&buffer_id).is_some() {
            // Clean up highlights
            self.highlight_store.clear_all(buffer_id);

            // Emit BufferClosed event for plugins to clean up their state
            // Use emit_event to propagate scope for deterministic completion tracking
            self.emit_event(BufferClosed { buffer_id });

            // If we closed the active buffer, switch to another one
            if self.active_buffer_id() == buffer_id
                && let Some(&new_id) = self.buffers.keys().next()
            {
                self.screen.set_editor_buffer(new_id);
            }
            true
        } else {
            false
        }
    }

    /// Get the previous buffer ID (wraps around)
    #[must_use]
    pub fn prev_buffer_id(&self) -> Option<usize> {
        let ids: Vec<usize> = self.buffers.keys().copied().collect();
        if ids.len() <= 1 {
            return None;
        }
        let current_pos = ids.iter().position(|&id| id == self.active_buffer_id())?;
        let prev_pos = if current_pos == 0 {
            ids.len() - 1
        } else {
            current_pos - 1
        };
        Some(ids[prev_pos])
    }

    /// Get the next buffer ID (wraps around)
    #[must_use]
    pub fn next_buffer_id(&self) -> Option<usize> {
        let ids: Vec<usize> = self.buffers.keys().copied().collect();
        if ids.len() <= 1 {
            return None;
        }
        let current_pos = ids.iter().position(|&id| id == self.active_buffer_id())?;
        let next_pos = (current_pos + 1) % ids.len();
        Some(ids[next_pos])
    }

    /// Get the currently active buffer ID (derived from active window)
    ///
    /// This is the single source of truth for the active buffer, derived from
    /// the screen's active window. Returns 0 if no active window exists.
    #[must_use]
    pub fn active_buffer_id(&self) -> usize {
        self.screen.active_buffer_id().unwrap_or(0)
    }

    /// Get the currently active buffer
    #[must_use]
    pub fn active_buffer(&self) -> Option<&Buffer> {
        self.buffers.get(&self.active_buffer_id())
    }

    /// Get the currently active buffer mutably
    pub fn active_buffer_mut(&mut self) -> Option<&mut Buffer> {
        self.buffers.get_mut(&self.active_buffer_id())
    }
}
