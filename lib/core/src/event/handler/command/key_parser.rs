//! Key event to string conversion

use crate::event::KeyEvent;
use reovim_sys::event::{KeyCode, KeyModifiers};

/// Convert a key event to its string representation for keymap lookup
#[must_use]
pub fn key_to_string(event: &KeyEvent) -> String {
    let has_ctrl = event.modifiers.contains(KeyModifiers::CONTROL);

    // Handle Ctrl combinations first
    if has_ctrl {
        match event.code {
            KeyCode::Char(c) => return format!("<C-{c}>"),
            KeyCode::Tab => return String::from("<C-Tab>"),
            _ => {}
        }
    }

    match event.code {
        KeyCode::Char(c) => c.to_string(),
        KeyCode::Esc => String::from("Escape"),
        KeyCode::Enter => String::from("Enter"),
        KeyCode::Backspace => String::from("Backspace"),
        KeyCode::Tab => String::from("Tab"),
        KeyCode::Left => String::from("Left"),
        KeyCode::Right => String::from("Right"),
        KeyCode::Up => String::from("Up"),
        KeyCode::Down => String::from("Down"),
        KeyCode::Home => String::from("Home"),
        KeyCode::End => String::from("End"),
        KeyCode::Delete => String::from("Delete"),
        _ => String::new(),
    }
}

// Tests omitted - KeyEvent construction depends on crossterm internals
