use super::Runtime;

impl Runtime {
    /// Get a snapshot of the current mode state for RPC
    #[must_use]
    pub fn mode_snapshot(&self) -> crate::rpc::ModeSnapshot {
        crate::rpc::ModeSnapshot::from_mode(&self.mode_state, &self.display_registry)
    }

    /// Get a snapshot of cursor position for a buffer
    #[must_use]
    pub fn cursor_snapshot(&self, buffer_id: usize) -> Option<crate::rpc::CursorSnapshot> {
        self.buffers
            .get(&buffer_id)
            .map(|buf| crate::rpc::CursorSnapshot::from(&buf.cur))
    }

    /// Get a snapshot of buffer metadata
    #[must_use]
    pub fn buffer_snapshot(&self, buffer_id: usize) -> Option<crate::rpc::BufferSnapshot> {
        self.buffers
            .get(&buffer_id)
            .map(crate::rpc::BufferSnapshot::from)
    }

    /// Get a list of all buffer snapshots
    #[must_use]
    pub fn buffer_list_snapshot(&self) -> Vec<crate::rpc::BufferSnapshot> {
        self.buffers
            .values()
            .map(crate::rpc::BufferSnapshot::from)
            .collect()
    }

    /// Get a snapshot of selection state for a buffer
    #[must_use]
    pub fn selection_snapshot(&self, buffer_id: usize) -> Option<crate::rpc::SelectionSnapshot> {
        self.buffers.get(&buffer_id).map(|buf| {
            let mut snapshot = crate::rpc::SelectionSnapshot::from(&buf.selection);
            // Fill in the actual cursor position
            snapshot.cursor = crate::rpc::CursorSnapshot::from(&buf.cur);
            snapshot
        })
    }

    /// Get a snapshot of screen dimensions and state
    #[must_use]
    pub fn screen_snapshot(&self) -> crate::rpc::ScreenSnapshot {
        crate::rpc::ScreenSnapshot {
            width: self.screen.width(),
            height: self.screen.height(),
            active_buffer_id: self.active_buffer_id(),
            active_window_id: self.screen.active_window_id(),
            window_count: self.screen.window_count(),
        }
    }

    /// Get a snapshot of all windows and their states
    #[must_use]
    pub fn windows_snapshot(&self) -> Vec<crate::rpc::WindowSnapshot> {
        self.screen
            .windows()
            .iter()
            .map(|w| crate::rpc::WindowSnapshot {
                id: w.id,
                buffer_id: w.buffer_id().unwrap_or(0),
                buffer_anchor_x: w.buffer_anchor().map_or(0, |a| a.x),
                buffer_anchor_y: w.buffer_anchor().map_or(0, |a| a.y),
                is_active: w.is_active,
                cursor_x: w.cursor.x,
                cursor_y: w.cursor.y,
            })
            .collect()
    }

    /// Get the content of a buffer as a string
    #[must_use]
    pub fn buffer_content(&self, buffer_id: usize) -> Option<String> {
        self.buffers.get(&buffer_id).map(|buf| {
            buf.contents
                .iter()
                .map(|line| line.inner.as_str())
                .collect::<Vec<_>>()
                .join("\n")
        })
    }

    /// Get a visual snapshot of the screen for debugging/AI understanding
    ///
    /// Returns None if the frame renderer is not enabled.
    #[must_use]
    pub fn visual_snapshot(&self) -> Option<crate::visual::VisualSnapshot> {
        let buffer = self.screen.frame_buffer()?;

        // Build cell grid
        let mut cells = Vec::with_capacity(buffer.height() as usize);
        for y in 0..buffer.height() {
            let mut row = Vec::with_capacity(buffer.width() as usize);
            if let Some(buffer_row) = buffer.row(y) {
                for cell in buffer_row {
                    row.push(crate::rpc::CellSnapshot::from(cell));
                }
            }
            cells.push(row);
        }

        // Get cursor info
        let cursor =
            self.buffers
                .get(&self.active_buffer_id())
                .map(|buf| crate::visual::CursorInfo {
                    x: buf.cur.x,
                    y: buf.cur.y,
                    layer: "editor".to_string(),
                });

        // Get layer info - just base layers for now
        // Plugin windows provide their own z-order during rendering
        let layers = vec![
            crate::visual::LayerInfo {
                name: "base".to_string(),
                z_order: 0,
                visible: true,
                bounds: crate::visual::BoundsInfo::new(0, 0, buffer.width(), buffer.height()),
            },
            crate::visual::LayerInfo {
                name: "editor".to_string(),
                z_order: 2,
                visible: true,
                bounds: crate::visual::BoundsInfo::new(
                    0,
                    1,
                    buffer.width(),
                    buffer.height().saturating_sub(2),
                ),
            },
        ];

        // Build plain text
        let plain_text = buffer.to_ascii();

        Some(crate::visual::VisualSnapshot {
            width: buffer.width(),
            height: buffer.height(),
            cells,
            cursor,
            layers,
            plain_text,
        })
    }
}
