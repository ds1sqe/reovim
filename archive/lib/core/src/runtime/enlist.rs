//! Handler registration (enlist) for interactors
//!
//! This module implements the Bevy/Zed-inspired enlist pattern where
//! handlers are registered at initialization time, enabling fully
//! generic dispatch at runtime without hardcoded match arms.
//!
//! Handler functions are public so they can be registered by plugins
//! (specifically `UIComponentsPlugin`).

use {
    super::Runtime,
    crate::{
        buffer::TextOps,
        event_bus::{BufferModification, BufferModified},
        screen::Position,
    },
};

/// Editor focus input handler
///
/// Routes input to command line or buffer based on current mode.
pub fn handle_editor_input(
    rt: &mut Runtime,
    char: Option<char>,
    delete: bool,
    _clear_landing: bool, // Deprecated - now checks rt.showing_landing_page directly
) {
    // Clear landing page when inserting first character
    if rt.showing_landing_page && char.is_some() {
        if let Some(buffer) = rt.buffers.get_mut(&0) {
            buffer.contents.clear();
            buffer.cur = Position::default();
        }
        rt.showing_landing_page = false;
        rt.landing_state = None;
    }

    if rt.mode_state.is_command() {
        // Command line input
        if let Some(c) = char {
            rt.command_line.insert_char(c);
        }
        if delete {
            rt.command_line.delete_char();
        }
    } else if rt.mode_state.is_insert() {
        // Buffer input
        let buffer_id = rt.active_buffer_id();

        // Track event data to emit after releasing mutable borrow
        let mut insert_event: Option<(usize, usize, char)> = None;

        if let Some(buffer) = rt.buffers.get_mut(&buffer_id) {
            if let Some(c) = char {
                // Get position before insertion for event
                let start = (buffer.cur.y as usize, buffer.cur.x as usize);
                buffer.insert_char(c);
                insert_event = Some((start.0, start.1, c));
            }
            if delete {
                buffer.delete_char_backward();
            }
        }

        // Emit BufferModified event after releasing mutable borrow
        // Use emit_event to propagate scope for deterministic completion tracking
        if let Some((start_y, start_x, c)) = insert_event {
            rt.emit_event(BufferModified {
                buffer_id,
                modification: BufferModification::Insert {
                    start: (start_y, start_x),
                    text: c.to_string(),
                },
            });
        }
    }
}

/// Command line focus input handler
///
/// Handles input for `:` command mode.
pub fn handle_command_line_input(rt: &mut Runtime, char: Option<char>, delete: bool, _clear: bool) {
    if let Some(c) = char {
        rt.command_line.insert_char(c);
    }
    if delete {
        rt.command_line.delete_char();
    }
}
