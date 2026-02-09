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
mod tests {
    use {super::*, crossterm::event::KeyEventKind};

    fn make_key(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
        KeyEvent {
            code,
            modifiers,
            kind: KeyEventKind::Press,
            state: crossterm::event::KeyEventState::NONE,
        }
    }

    #[test]
    fn test_simple_char() {
        let key = make_key(KeyCode::Char('a'), KeyModifiers::NONE);
        assert_eq!(InputHandler::key_to_notation(&key), Some("a".to_string()));
    }

    #[test]
    fn test_uppercase_char() {
        let key = make_key(KeyCode::Char('A'), KeyModifiers::SHIFT);
        assert_eq!(InputHandler::key_to_notation(&key), Some("A".to_string()));
    }

    #[test]
    fn test_ctrl_char() {
        let key = make_key(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(InputHandler::key_to_notation(&key), Some("<C-c>".to_string()));
    }

    #[test]
    fn test_alt_char() {
        let key = make_key(KeyCode::Char('x'), KeyModifiers::ALT);
        assert_eq!(InputHandler::key_to_notation(&key), Some("<M-x>".to_string()));
    }

    #[test]
    fn test_escape() {
        let key = make_key(KeyCode::Esc, KeyModifiers::NONE);
        assert_eq!(InputHandler::key_to_notation(&key), Some("<Esc>".to_string()));
    }

    #[test]
    fn test_enter() {
        let key = make_key(KeyCode::Enter, KeyModifiers::NONE);
        assert_eq!(InputHandler::key_to_notation(&key), Some("<CR>".to_string()));
    }

    #[test]
    fn test_arrow_keys() {
        let key = make_key(KeyCode::Up, KeyModifiers::NONE);
        assert_eq!(InputHandler::key_to_notation(&key), Some("<Up>".to_string()));

        let key = make_key(KeyCode::Down, KeyModifiers::CONTROL);
        assert_eq!(InputHandler::key_to_notation(&key), Some("<C-Down>".to_string()));
    }

    #[test]
    fn test_function_keys() {
        let key = make_key(KeyCode::F(1), KeyModifiers::NONE);
        assert_eq!(InputHandler::key_to_notation(&key), Some("<F1>".to_string()));

        let key = make_key(KeyCode::F(12), KeyModifiers::SHIFT);
        assert_eq!(InputHandler::key_to_notation(&key), Some("<S-F12>".to_string()));
    }

    #[test]
    fn test_space() {
        let key = make_key(KeyCode::Char(' '), KeyModifiers::NONE);
        assert_eq!(InputHandler::key_to_notation(&key), Some("<Space>".to_string()));
    }

    #[test]
    fn test_tab() {
        let key = make_key(KeyCode::Tab, KeyModifiers::NONE);
        assert_eq!(InputHandler::key_to_notation(&key), Some("<Tab>".to_string()));
    }

    #[test]
    fn test_shift_tab() {
        let key = make_key(KeyCode::Tab, KeyModifiers::SHIFT);
        assert_eq!(InputHandler::key_to_notation(&key), Some("<S-Tab>".to_string()));
    }

    #[test]
    fn test_special_keys() {
        assert_eq!(
            InputHandler::key_to_notation(&make_key(KeyCode::Backspace, KeyModifiers::NONE)),
            Some("<BS>".to_string())
        );
        assert_eq!(
            InputHandler::key_to_notation(&make_key(KeyCode::Delete, KeyModifiers::NONE)),
            Some("<Del>".to_string())
        );
        assert_eq!(
            InputHandler::key_to_notation(&make_key(KeyCode::Insert, KeyModifiers::NONE)),
            Some("<Insert>".to_string())
        );
        assert_eq!(
            InputHandler::key_to_notation(&make_key(KeyCode::Home, KeyModifiers::NONE)),
            Some("<Home>".to_string())
        );
        assert_eq!(
            InputHandler::key_to_notation(&make_key(KeyCode::End, KeyModifiers::NONE)),
            Some("<End>".to_string())
        );
        assert_eq!(
            InputHandler::key_to_notation(&make_key(KeyCode::PageUp, KeyModifiers::NONE)),
            Some("<PageUp>".to_string())
        );
        assert_eq!(
            InputHandler::key_to_notation(&make_key(KeyCode::PageDown, KeyModifiers::NONE)),
            Some("<PageDown>".to_string())
        );
    }

    #[test]
    fn test_left_right_arrows() {
        assert_eq!(
            InputHandler::key_to_notation(&make_key(KeyCode::Left, KeyModifiers::NONE)),
            Some("<Left>".to_string())
        );
        assert_eq!(
            InputHandler::key_to_notation(&make_key(KeyCode::Right, KeyModifiers::NONE)),
            Some("<Right>".to_string())
        );
    }

    #[test]
    fn test_ctrl_alt_char() {
        let key = make_key(KeyCode::Char('x'), KeyModifiers::CONTROL | KeyModifiers::ALT);
        assert_eq!(InputHandler::key_to_notation(&key), Some("<C-M-x>".to_string()));
    }

    #[test]
    fn test_alt_shift_lowercase() {
        let key = make_key(KeyCode::Char('a'), KeyModifiers::ALT | KeyModifiers::SHIFT);
        assert_eq!(InputHandler::key_to_notation(&key), Some("<M-A>".to_string()));
    }

    #[test]
    fn test_less_than() {
        let key = make_key(KeyCode::Char('<'), KeyModifiers::NONE);
        assert_eq!(InputHandler::key_to_notation(&key), Some("<lt>".to_string()));
    }

    #[test]
    #[cfg_attr(coverage_nightly, coverage(off))]
    fn test_unknown_key_code() {
        let key = make_key(KeyCode::Null, KeyModifiers::NONE);
        assert_eq!(InputHandler::key_to_notation(&key), None);
    }

    #[test]
    fn test_event_to_keys_non_key_event() {
        let event = Event::Resize(80, 24);
        assert_eq!(InputHandler::event_to_keys(&event), None);
    }

    #[test]
    fn test_event_to_keys_key_event() {
        let event = Event::Key(make_key(KeyCode::Char('a'), KeyModifiers::NONE));
        assert_eq!(InputHandler::event_to_keys(&event), Some("a".to_string()));
    }
}
