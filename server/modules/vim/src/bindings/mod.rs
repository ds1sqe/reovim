//! Vim keybindings.
//!
//! This module contains all Vim-style keybindings organized by mode:
//! - `normal` - Normal mode (hjkl, operators, etc.)
//! - `insert` - Insert mode (Escape, etc.)
//! - `visual` - Visual mode (selection commands)
//! - `operator_modes` - Dedicated operator modes: delete, yank, change (Epic #415)
//! - `commandline` - Command-line mode (: commands)
//! - `window` - Window mode (`<C-w>` commands) (Epic #438)

pub mod commandline;
pub mod insert;
pub mod normal;
pub mod operator_modes;
pub mod visual;
pub mod window;

use reovim_kernel::api::v1::KeybindingRegistration;

/// Returns all Vim keybindings.
///
/// This collects bindings from all modes into a single vector.
#[must_use]
pub fn all() -> Vec<KeybindingRegistration> {
    let mut bindings = Vec::new();
    bindings.extend(normal::bindings());
    bindings.extend(insert::bindings());
    bindings.extend(visual::bindings());
    bindings.extend(operator_modes::all_operator_bindings());
    bindings.extend(commandline::bindings());
    bindings.extend(window::bindings());
    bindings
}
