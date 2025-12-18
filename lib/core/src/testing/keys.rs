//! Helper functions for creating key events in tests
//!
//! Provides convenient functions to create key events, including
//! a string notation parser for readable test code.

use reovim_sys::event::{KeyCode, KeyEvent, KeyEventKind, KeyEventState, KeyModifiers};

/// Create a simple key press event with no modifiers.
#[must_use]
pub const fn key(code: KeyCode) -> KeyEvent {
    KeyEvent {
        code,
        modifiers: KeyModifiers::NONE,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    }
}

/// Create a key event with modifiers.
#[must_use]
pub const fn key_mod(code: KeyCode, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent {
        code,
        modifiers,
        kind: KeyEventKind::Press,
        state: KeyEventState::NONE,
    }
}

/// Create a Ctrl+key event.
#[must_use]
pub const fn ctrl(c: char) -> KeyEvent {
    key_mod(KeyCode::Char(c), KeyModifiers::CONTROL)
}

/// Create a character key press.
#[must_use]
pub const fn char_key(c: char) -> KeyEvent {
    key(KeyCode::Char(c))
}

/// Convert a string to a sequence of key events.
///
/// Supports both regular characters and special key notation:
/// - Regular characters: `"ihello"` → `[i, h, e, l, l, o]`
/// - Escape: `<Esc>` or `<Escape>`
/// - Enter: `<CR>` or `<Enter>` or `<Return>`
/// - Tab: `<Tab>`
/// - Backspace: `<BS>` or `<Backspace>`
/// - Space: `<Space>` or `<Spc>`
/// - Arrow keys: `<Up>`, `<Down>`, `<Left>`, `<Right>`
/// - Ctrl combinations: `<C-x>` for Ctrl+x
/// - Shift combinations: `<S-x>` for Shift+x
/// - Function keys: `<F1>` through `<F12>`
///
/// # Examples
///
/// ```ignore
/// let events = keys_from_str("ihello<Esc>");
/// // Creates: [i, h, e, l, l, o, Esc]
///
/// let events = keys_from_str(":wq<CR>");
/// // Creates: [:, w, q, Enter]
///
/// let events = keys_from_str("<C-d>");
/// // Creates: [Ctrl+d]
/// ```
#[must_use]
#[allow(clippy::missing_panics_doc)] // unwrap() is guarded by peek()
pub fn keys_from_str(s: &str) -> Vec<KeyEvent> {
    let mut events = Vec::new();
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '<' {
            // Parse special key notation like <Esc>, <CR>, <C-x>
            let mut special = String::new();
            while let Some(&next) = chars.peek() {
                if next == '>' {
                    chars.next();
                    break;
                }
                special.push(chars.next().unwrap());
            }

            if let Some(event) = parse_special_key(&special) {
                events.push(event);
            }
        } else {
            events.push(char_key(c));
        }
    }

    events
}

/// Parse a special key notation string.
fn parse_special_key(s: &str) -> Option<KeyEvent> {
    let upper = s.to_uppercase();

    // Check for modifier prefixes
    if let Some(rest) = upper.strip_prefix("C-") {
        // Ctrl combination
        let c = rest.chars().next()?;
        return Some(ctrl(c.to_ascii_lowercase()));
    }

    if let Some(rest) = upper.strip_prefix("S-") {
        // Shift combination
        let c = rest.chars().next()?;
        return Some(key_mod(KeyCode::Char(c), KeyModifiers::SHIFT));
    }

    if let Some(rest) = upper.strip_prefix("A-").or_else(|| upper.strip_prefix("M-")) {
        // Alt/Meta combination
        let c = rest.chars().next()?;
        return Some(key_mod(
            KeyCode::Char(c.to_ascii_lowercase()),
            KeyModifiers::ALT,
        ));
    }

    // Function keys
    if let Some(rest) = upper.strip_prefix('F')
        && let Ok(n) = rest.parse::<u8>()
        && (1..=12).contains(&n)
    {
        return Some(key(KeyCode::F(n)));
    }

    // Named keys
    match upper.as_str() {
        "ESC" | "ESCAPE" => Some(key(KeyCode::Esc)),
        "CR" | "ENTER" | "RETURN" => Some(key(KeyCode::Enter)),
        "TAB" => Some(key(KeyCode::Tab)),
        "BS" | "BACKSPACE" => Some(key(KeyCode::Backspace)),
        "SPACE" | "SPC" => Some(char_key(' ')),
        "UP" => Some(key(KeyCode::Up)),
        "DOWN" => Some(key(KeyCode::Down)),
        "LEFT" => Some(key(KeyCode::Left)),
        "RIGHT" => Some(key(KeyCode::Right)),
        "HOME" => Some(key(KeyCode::Home)),
        "END" => Some(key(KeyCode::End)),
        "PAGEUP" | "PGUP" => Some(key(KeyCode::PageUp)),
        "PAGEDOWN" | "PGDN" => Some(key(KeyCode::PageDown)),
        "INSERT" | "INS" => Some(key(KeyCode::Insert)),
        "DELETE" | "DEL" => Some(key(KeyCode::Delete)),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_char_key() {
        let event = char_key('a');
        assert_eq!(event.code, KeyCode::Char('a'));
        assert_eq!(event.modifiers, KeyModifiers::NONE);
    }

    #[test]
    fn test_ctrl_key() {
        let event = ctrl('c');
        assert_eq!(event.code, KeyCode::Char('c'));
        assert!(event.modifiers.contains(KeyModifiers::CONTROL));
    }

    #[test]
    fn test_keys_from_str_basic() {
        let events = keys_from_str("abc");
        assert_eq!(events.len(), 3);
        assert_eq!(events[0].code, KeyCode::Char('a'));
        assert_eq!(events[1].code, KeyCode::Char('b'));
        assert_eq!(events[2].code, KeyCode::Char('c'));
    }

    #[test]
    fn test_keys_from_str_with_escape() {
        let events = keys_from_str("ihello<Esc>");
        assert_eq!(events.len(), 7);
        assert_eq!(events[0].code, KeyCode::Char('i'));
        assert_eq!(events[5].code, KeyCode::Char('o'));
        assert_eq!(events[6].code, KeyCode::Esc);
    }

    #[test]
    fn test_keys_from_str_with_enter() {
        let events = keys_from_str(":wq<CR>");
        assert_eq!(events.len(), 4);
        assert_eq!(events[0].code, KeyCode::Char(':'));
        assert_eq!(events[3].code, KeyCode::Enter);
    }

    #[test]
    fn test_keys_from_str_ctrl() {
        let events = keys_from_str("<C-d>");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].code, KeyCode::Char('d'));
        assert!(events[0].modifiers.contains(KeyModifiers::CONTROL));
    }

    #[test]
    fn test_keys_from_str_arrow_keys() {
        let events = keys_from_str("jj<Down><Up>");
        assert_eq!(events.len(), 4);
        assert_eq!(events[0].code, KeyCode::Char('j'));
        assert_eq!(events[1].code, KeyCode::Char('j'));
        assert_eq!(events[2].code, KeyCode::Down);
        assert_eq!(events[3].code, KeyCode::Up);
    }

    #[test]
    fn test_keys_from_str_function_keys() {
        let events = keys_from_str("<F1><F12>");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].code, KeyCode::F(1));
        assert_eq!(events[1].code, KeyCode::F(12));
    }

    #[test]
    fn test_keys_from_str_mixed() {
        let events = keys_from_str("dd:q!<CR>");
        assert_eq!(events.len(), 6);
        assert_eq!(events[0].code, KeyCode::Char('d'));
        assert_eq!(events[1].code, KeyCode::Char('d'));
        assert_eq!(events[2].code, KeyCode::Char(':'));
        assert_eq!(events[3].code, KeyCode::Char('q'));
        assert_eq!(events[4].code, KeyCode::Char('!'));
        assert_eq!(events[5].code, KeyCode::Enter);
    }
}
