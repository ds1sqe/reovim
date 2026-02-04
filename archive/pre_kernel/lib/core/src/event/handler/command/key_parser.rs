//! Key event to string conversion

use {
    crate::event::KeyEvent,
    reovim_sys::event::{KeyCode, KeyModifiers},
};

/// Convert a key event to its string representation for keymap lookup
#[must_use]
pub fn key_to_string(event: &KeyEvent) -> String {
    let has_ctrl = event.modifiers.contains(KeyModifiers::CONTROL);
    let has_shift = event.modifiers.contains(KeyModifiers::SHIFT);
    let has_alt = event.modifiers.contains(KeyModifiers::ALT);

    // Handle Ctrl+Shift combinations (e.g., C-S-H for window movement)
    if has_ctrl
        && has_shift
        && let KeyCode::Char(c) = event.code
    {
        return format!("<C-S-{}>", c.to_ascii_uppercase());
    }

    // Handle Ctrl combinations
    if has_ctrl {
        match event.code {
            KeyCode::Char(c) => return format!("<C-{c}>"),
            KeyCode::Tab => return String::from("<C-Tab>"),
            _ => {}
        }
    }

    // Handle Alt combinations
    if has_alt {
        match event.code {
            KeyCode::Char(' ') => return String::from("<A-Space>"),
            KeyCode::Char(c) => return format!("<A-{c}>"),
            KeyCode::Enter => return String::from("<A-Enter>"),
            KeyCode::Tab => return String::from("<A-Tab>"),
            _ => {}
        }
    }

    // Handle Shift+Tab
    if has_shift && event.code == KeyCode::BackTab {
        return String::from("S-Tab");
    }

    match event.code {
        // Handle raw escape byte (some terminals send '\x1b' as Char instead of Esc)
        KeyCode::Char('\x1b') | KeyCode::Esc => String::from("Escape"),
        KeyCode::Char(c) => c.to_string(),
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
