//! Terminal input handling for TUI client.
//!
//! Reads terminal events and converts them to vim-style key notation.

use std::time::Duration;

use crossterm::event::{self, Event, KeyCode, KeyEvent, KeyModifiers};

/// Input handler for terminal events.
pub struct InputHandler;

impl InputHandler {
    /// Read next terminal event with timeout.
    ///
    /// Returns `None` if no event available within timeout.
    ///
    /// # Errors
    ///
    /// Returns error if reading fails.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn poll_event(timeout: Duration) -> std::io::Result<Option<Event>> {
        if event::poll(timeout)? {
            Ok(Some(event::read()?))
        } else {
            Ok(None)
        }
    }

    /// Read next terminal event (blocking).
    ///
    /// # Errors
    ///
    /// Returns error if reading fails.
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn read_event() -> std::io::Result<Event> {
        event::read()
    }

    /// Convert a key event to vim-style notation.
    ///
    /// Returns `None` for events that shouldn't be sent (like pure modifier presses).
    #[must_use]
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn key_to_notation(key: &KeyEvent) -> Option<String> {
        let KeyEvent {
            code, modifiers, ..
        } = key;

        // Handle modifiers
        let ctrl = modifiers.contains(KeyModifiers::CONTROL);
        let alt = modifiers.contains(KeyModifiers::ALT);
        let shift = modifiers.contains(KeyModifiers::SHIFT);

        match code {
            // Special keys
            KeyCode::Esc => Some("<Esc>".to_string()),
            KeyCode::Enter => Some("<CR>".to_string()),
            KeyCode::Tab => {
                if shift {
                    Some("<S-Tab>".to_string())
                } else {
                    Some("<Tab>".to_string())
                }
            }
            KeyCode::Backspace => Some("<BS>".to_string()),
            KeyCode::Delete => Some("<Del>".to_string()),
            KeyCode::Insert => Some("<Insert>".to_string()),
            KeyCode::Home => Some("<Home>".to_string()),
            KeyCode::End => Some("<End>".to_string()),
            KeyCode::PageUp => Some("<PageUp>".to_string()),
            KeyCode::PageDown => Some("<PageDown>".to_string()),

            // Arrow keys
            KeyCode::Up => Some(format_with_modifiers("Up", ctrl, alt, shift)),
            KeyCode::Down => Some(format_with_modifiers("Down", ctrl, alt, shift)),
            KeyCode::Left => Some(format_with_modifiers("Left", ctrl, alt, shift)),
            KeyCode::Right => Some(format_with_modifiers("Right", ctrl, alt, shift)),

            // Function keys
            KeyCode::F(n) => Some(format_with_modifiers(&format!("F{n}"), ctrl, alt, shift)),

            // Character keys
            KeyCode::Char(c) => {
                let c = *c;

                // Ctrl+letter
                if ctrl && !alt {
                    return Some(format!("<C-{}>", c.to_ascii_lowercase()));
                }

                // Alt+key
                if alt && !ctrl {
                    if shift && c.is_ascii_lowercase() {
                        return Some(format!("<M-{}>", c.to_ascii_uppercase()));
                    }
                    return Some(format!("<M-{c}>"));
                }

                // Ctrl+Alt+key
                if ctrl && alt {
                    return Some(format!("<C-M-{}>", c.to_ascii_lowercase()));
                }

                // Space
                if c == ' ' {
                    return Some("<Space>".to_string());
                }

                // Less than / Greater than (need escaping in some contexts)
                if c == '<' {
                    return Some("<lt>".to_string());
                }

                // Regular character (shift is implicit in uppercase)
                Some(c.to_string())
            }

            // Ignore other keys
            _ => None,
        }
    }

    /// Convert terminal event to key notation if applicable.
    #[must_use]
    pub fn event_to_keys(event: &Event) -> Option<String> {
        match event {
            Event::Key(key) => Self::key_to_notation(key),
            // Resize handled separately, mouse/focus not yet supported
            _ => None,
        }
    }
}

/// Format a key with modifiers.
#[cfg_attr(coverage_nightly, coverage(off))]
fn format_with_modifiers(key: &str, ctrl: bool, alt: bool, shift: bool) -> String {
    let mut result = String::with_capacity(key.len() + 8);
    result.push('<');

    if ctrl {
        result.push_str("C-");
    }
    if alt {
        result.push_str("M-");
    }
    if shift {
        result.push_str("S-");
    }

    result.push_str(key);
    result.push('>');
    result
}

#[cfg(test)]
#[path = "input_tests.rs"]
mod tests;
