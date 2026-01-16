//! Visual mode selection handlers for the event loop.
//!
//! Handles saving and restoring visual selections for the `gv` command.

use super::EventLoop;
use crate::server::{AppState, app::LastVisualSelection};

impl<F: reovim_driver_input::InputFallbackHandler<AppState>> EventLoop<F> {
    /// Save the current visual selection if one is active.
    ///
    /// Called before exit-visual commands to preserve the selection for `gv`.
    pub(super) fn save_visual_selection_if_active(&mut self) {
        let Some(buffer_id) = self.app.active_buffer else {
            return;
        };

        let Some(buffer_arc) = self.app.kernel.buffers.get(buffer_id) else {
            return;
        };

        let buffer = buffer_arc.read();
        let selection = buffer.selection();

        // Only save if selection is active
        if !selection.is_active() {
            return;
        }

        // Capture the current mode so we can restore it on gv
        let current_mode = self.app.current_mode().clone();

        let last_selection = LastVisualSelection::new(
            buffer_id,
            selection.anchor,
            buffer.position(),
            selection.mode(),
            current_mode,
        );
        drop(buffer);

        self.app.save_visual_selection(last_selection);
    }

    /// Handle the reselect-last (gv) command.
    ///
    /// Restores the saved visual selection and enters the appropriate visual mode.
    pub(super) fn handle_reselect_last(&mut self) {
        let Some(last_selection) = self.app.last_visual_selection() else {
            // No saved selection - nothing to do
            return;
        };

        // Clone values to avoid borrow issues
        let buffer_id = last_selection.buffer_id;
        let anchor = last_selection.anchor;
        let cursor = last_selection.cursor;
        let mode = last_selection.mode;
        let mode_id = last_selection.mode_id.clone();

        // Verify the buffer still exists and is the active buffer
        // (In Vim, gv only works if you're in the same buffer)
        if self.app.active_buffer != Some(buffer_id) {
            // Different buffer - switch to it first (or ignore)
            // For now, we'll only restore in the same buffer
            return;
        }

        let Some(buffer_arc) = self.app.kernel.buffers.get(buffer_id) else {
            return;
        };

        // Restore the selection
        {
            let mut buffer = buffer_arc.write();
            buffer.selection_mut().start(anchor, mode);
            buffer.set_position(cursor);
        }

        // Transition to the stored visual mode
        // Using the stored mode_id instead of deriving from SelectionMode
        // removes hardcoded "editor" module assumption
        self.app.mode_stack.set(mode_id);
    }
}
