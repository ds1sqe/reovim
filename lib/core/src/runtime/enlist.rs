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
        event::{InnerEvent, TelescopeEvent},
        screen::Position,
    },
};

/// Telescope focus input handler
///
/// Modifies telescope query and triggers async filtering via `UpdateQuery` event.
pub fn handle_telescope_input(rt: &mut Runtime, char: Option<char>, delete: bool, _clear: bool) {
    if let Some(c) = char {
        rt.telescope_state.insert_char(c);
    }
    if delete {
        rt.telescope_state.delete_char();
    }
    // Trigger async filtering
    let query = rt.telescope_state.query.clone();
    drop(
        rt.tx
            .try_send(InnerEvent::TelescopeEvent(TelescopeEvent::UpdateQuery { query })),
    );
}

/// Editor focus input handler
///
/// Routes input to command line or buffer based on current mode.
pub fn handle_editor_input(
    rt: &mut Runtime,
    char: Option<char>,
    delete: bool,
    clear_landing: bool,
) {
    // Clear landing page if requested
    if clear_landing && rt.showing_landing_page {
        if let Some(buffer) = rt.buffers.get_mut(&0) {
            buffer.contents.clear();
            buffer.cur = Position::default();
        }
        rt.showing_landing_page = false;
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
        if let Some(buffer) = rt.buffers.get_mut(&rt.active_buffer_id) {
            if let Some(c) = char {
                buffer.insert_char(c);
            }
            if delete {
                buffer.delete_char_backward();
            }
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
