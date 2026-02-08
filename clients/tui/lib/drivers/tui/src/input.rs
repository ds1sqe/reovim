//! Terminal input event handling.
//!
//! Provides an async stream of terminal input events with translation
//! to vim-style key notation.

use std::time::Duration;

use {
    crossterm::event::{
        self, Event as CrosstermEvent, KeyCode, KeyEvent as CrosstermKeyEvent, KeyModifiers,
        MouseButton, MouseEvent as CrosstermMouseEvent, MouseEventKind,
    },
    futures::StreamExt,
};

/// Terminal input event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputEvent {
    /// Keyboard event.
    Key(KeyEvent),
    /// Mouse event.
    Mouse(MouseEvent),
    /// Terminal resize event.
    Resize(ResizeEvent),
    /// Focus gained.
    FocusGained,
    /// Focus lost.
    FocusLost,
    /// Paste event (bracketed paste).
    Paste(String),
}

/// Keyboard event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyEvent {
    /// Key code.
    pub code: KeyCode,
    /// Modifier keys held.
    pub modifiers: KeyModifiers,
    /// Vim-style key notation (e.g., "<C-a>", "j", "<Esc>").
    pub vim_notation: String,
}

impl KeyEvent {
    /// Create a new key event from crossterm event.
    #[must_use]
    pub fn from_crossterm(event: CrosstermKeyEvent) -> Self {
        let vim_notation = translate_to_vim_notation(event.code, event.modifiers);
        Self {
            code: event.code,
            modifiers: event.modifiers,
            vim_notation,
        }
    }

    /// Check if this is a control key combination.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn is_ctrl(&self) -> bool {
        self.modifiers.contains(KeyModifiers::CONTROL)
    }

    /// Check if this is an alt key combination.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn is_alt(&self) -> bool {
        self.modifiers.contains(KeyModifiers::ALT)
    }

    /// Check if this is a shift key combination.
    #[must_use]
    #[allow(clippy::missing_const_for_fn)]
    pub fn is_shift(&self) -> bool {
        self.modifiers.contains(KeyModifiers::SHIFT)
    }
}

/// Mouse event.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MouseEvent {
    /// Mouse column (0-indexed).
    pub column: u16,
    /// Mouse row (0-indexed).
    pub row: u16,
    /// Event kind.
    pub kind: MouseEventKind,
}

impl MouseEvent {
    /// Create a new mouse event from crossterm event.
    #[must_use]
    pub const fn from_crossterm(event: CrosstermMouseEvent) -> Self {
        Self {
            column: event.column,
            row: event.row,
            kind: event.kind,
        }
    }

    /// Check if this is a left click.
    #[must_use]
    pub const fn is_left_click(&self) -> bool {
        matches!(self.kind, MouseEventKind::Down(MouseButton::Left))
    }

    /// Check if this is a right click.
    #[must_use]
    pub const fn is_right_click(&self) -> bool {
        matches!(self.kind, MouseEventKind::Down(MouseButton::Right))
    }

    /// Check if this is a scroll event.
    #[must_use]
    pub const fn is_scroll(&self) -> bool {
        matches!(self.kind, MouseEventKind::ScrollUp | MouseEventKind::ScrollDown)
    }
}

/// Terminal resize event.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResizeEvent {
    /// New terminal width.
    pub width: u16,
    /// New terminal height.
    pub height: u16,
}

/// Async input event reader.
///
/// Provides a stream of terminal input events using crossterm's
/// `EventStream` for async compatibility.
pub struct InputReader {
    /// Crossterm event stream.
    stream: crossterm::event::EventStream,
}

impl InputReader {
    /// Create a new input reader.
    #[must_use]
    pub fn new() -> Self {
        Self {
            stream: crossterm::event::EventStream::new(),
        }
    }

    /// Get the next input event.
    ///
    /// Returns `None` if the stream ends.
    pub async fn next_event(&mut self) -> Option<InputEvent> {
        match self.stream.next().await {
            Some(Ok(event)) => {
                let input_event = match event {
                    CrosstermEvent::Key(key) => InputEvent::Key(KeyEvent::from_crossterm(key)),
                    CrosstermEvent::Mouse(mouse) => {
                        InputEvent::Mouse(MouseEvent::from_crossterm(mouse))
                    }
                    CrosstermEvent::Resize(width, height) => {
                        InputEvent::Resize(ResizeEvent { width, height })
                    }
                    CrosstermEvent::FocusGained => InputEvent::FocusGained,
                    CrosstermEvent::FocusLost => InputEvent::FocusLost,
                    CrosstermEvent::Paste(text) => InputEvent::Paste(text),
                };
                tracing::trace!(?input_event, "Input event received");
                Some(input_event)
            }
            Some(Err(e)) => {
                tracing::warn!(?e, "Input stream error");
                None
            }
            None => {
                tracing::debug!("Input stream ended");
                None
            }
        }
    }

    /// Try to get an event with timeout.
    ///
    /// Returns `Ok(None)` if timeout expires, `Ok(Some(event))` if event received.
    ///
    /// # Errors
    ///
    /// This function never actually returns errors in the current implementation
    /// as all timeout and stream-end conditions return `Ok(None)`.
    pub async fn next_event_timeout(
        &mut self,
        timeout: Duration,
    ) -> std::io::Result<Option<InputEvent>> {
        match tokio::time::timeout(timeout, self.next_event()).await {
            Ok(Some(event)) => Ok(Some(event)),
            Ok(None) | Err(_) => Ok(None), // Stream ended or timeout
        }
    }

    /// Poll for available events without blocking.
    ///
    /// # Errors
    ///
    /// Returns an error if polling fails.
    pub fn poll(&self) -> std::io::Result<bool> {
        event::poll(Duration::ZERO)
    }
}

impl Default for InputReader {
    fn default() -> Self {
        Self::new()
    }
}

impl std::fmt::Debug for InputReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InputReader").finish()
    }
}

/// Translate crossterm key event to vim notation.
#[must_use]
fn translate_to_vim_notation(code: KeyCode, modifiers: KeyModifiers) -> String {
    let mut parts = Vec::new();

    // Build modifier prefix
    if modifiers.contains(KeyModifiers::CONTROL) {
        parts.push("C");
    }
    if modifiers.contains(KeyModifiers::ALT) {
        parts.push("A");
    }
    if modifiers.contains(KeyModifiers::SHIFT) {
        // Shift is implicit for uppercase letters
        match code {
            KeyCode::Char(c) if c.is_ascii_alphabetic() => {}
            _ => parts.push("S"),
        }
    }

    // Get key name
    let key_name = match code {
        KeyCode::Char(' ') => "Space".to_string(),
        KeyCode::Char('\t') | KeyCode::Tab => "Tab".to_string(),
        KeyCode::Char(c) => {
            if modifiers.contains(KeyModifiers::SHIFT) && c.is_ascii_alphabetic() {
                c.to_uppercase().to_string()
            } else {
                c.to_string()
            }
        }
        KeyCode::Enter => "CR".to_string(),
        KeyCode::Esc => "Esc".to_string(),
        KeyCode::Backspace => "BS".to_string(),
        KeyCode::Delete => "Del".to_string(),
        KeyCode::Left => "Left".to_string(),
        KeyCode::Right => "Right".to_string(),
        KeyCode::Up => "Up".to_string(),
        KeyCode::Down => "Down".to_string(),
        KeyCode::Home => "Home".to_string(),
        KeyCode::End => "End".to_string(),
        KeyCode::PageUp => "PageUp".to_string(),
        KeyCode::PageDown => "PageDown".to_string(),
        KeyCode::Insert => "Insert".to_string(),
        KeyCode::F(n) => format!("F{n}"),
        _ => return String::new(),
    };

    // Build vim notation
    if parts.is_empty() {
        // Single character without modifiers
        if key_name.len() == 1 {
            key_name
        } else {
            format!("<{key_name}>")
        }
    } else {
        // Has modifiers
        parts.push(&key_name);
        format!("<{}>", parts.join("-"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vim_notation_simple() {
        assert_eq!(translate_to_vim_notation(KeyCode::Char('j'), KeyModifiers::NONE), "j");
        assert_eq!(translate_to_vim_notation(KeyCode::Char('J'), KeyModifiers::SHIFT), "J");
    }

    #[test]
    fn test_vim_notation_ctrl() {
        assert_eq!(translate_to_vim_notation(KeyCode::Char('a'), KeyModifiers::CONTROL), "<C-a>");
        assert_eq!(translate_to_vim_notation(KeyCode::Char('w'), KeyModifiers::CONTROL), "<C-w>");
    }

    #[test]
    fn test_vim_notation_special() {
        assert_eq!(translate_to_vim_notation(KeyCode::Esc, KeyModifiers::NONE), "<Esc>");
        assert_eq!(translate_to_vim_notation(KeyCode::Enter, KeyModifiers::NONE), "<CR>");
        assert_eq!(translate_to_vim_notation(KeyCode::Backspace, KeyModifiers::NONE), "<BS>");
    }

    #[test]
    fn test_vim_notation_function_keys() {
        assert_eq!(translate_to_vim_notation(KeyCode::F(1), KeyModifiers::NONE), "<F1>");
        assert_eq!(translate_to_vim_notation(KeyCode::F(12), KeyModifiers::NONE), "<F12>");
    }

    #[test]
    fn test_key_event_modifiers() {
        let key = KeyEvent {
            code: KeyCode::Char('a'),
            modifiers: KeyModifiers::CONTROL | KeyModifiers::ALT,
            vim_notation: "<C-A-a>".to_string(),
        };
        assert!(key.is_ctrl());
        assert!(key.is_alt());
        assert!(!key.is_shift());
    }

    #[test]
    fn test_mouse_event_click() {
        let event = MouseEvent {
            column: 10,
            row: 5,
            kind: MouseEventKind::Down(MouseButton::Left),
        };
        assert!(event.is_left_click());
        assert!(!event.is_right_click());
        assert!(!event.is_scroll());
    }

    #[test]
    fn test_resize_event() {
        let event = ResizeEvent {
            width: 120,
            height: 40,
        };
        assert_eq!(event.width, 120);
        assert_eq!(event.height, 40);
    }

    #[test]
    fn test_vim_notation_space() {
        assert_eq!(translate_to_vim_notation(KeyCode::Char(' '), KeyModifiers::NONE), "<Space>");
    }

    #[test]
    fn test_vim_notation_tab() {
        assert_eq!(translate_to_vim_notation(KeyCode::Tab, KeyModifiers::NONE), "<Tab>");
        assert_eq!(translate_to_vim_notation(KeyCode::Char('\t'), KeyModifiers::NONE), "<Tab>");
    }

    #[test]
    fn test_vim_notation_arrows() {
        assert_eq!(translate_to_vim_notation(KeyCode::Left, KeyModifiers::NONE), "<Left>");
        assert_eq!(translate_to_vim_notation(KeyCode::Right, KeyModifiers::NONE), "<Right>");
        assert_eq!(translate_to_vim_notation(KeyCode::Up, KeyModifiers::NONE), "<Up>");
        assert_eq!(translate_to_vim_notation(KeyCode::Down, KeyModifiers::NONE), "<Down>");
    }

    #[test]
    fn test_vim_notation_home_end() {
        assert_eq!(translate_to_vim_notation(KeyCode::Home, KeyModifiers::NONE), "<Home>");
        assert_eq!(translate_to_vim_notation(KeyCode::End, KeyModifiers::NONE), "<End>");
    }

    #[test]
    fn test_vim_notation_page_up_down() {
        assert_eq!(translate_to_vim_notation(KeyCode::PageUp, KeyModifiers::NONE), "<PageUp>");
        assert_eq!(translate_to_vim_notation(KeyCode::PageDown, KeyModifiers::NONE), "<PageDown>");
    }

    #[test]
    fn test_vim_notation_delete_insert() {
        assert_eq!(translate_to_vim_notation(KeyCode::Delete, KeyModifiers::NONE), "<Del>");
        assert_eq!(translate_to_vim_notation(KeyCode::Insert, KeyModifiers::NONE), "<Insert>");
    }

    #[test]
    fn test_vim_notation_alt() {
        assert_eq!(translate_to_vim_notation(KeyCode::Char('a'), KeyModifiers::ALT), "<A-a>");
        assert_eq!(translate_to_vim_notation(KeyCode::Char('z'), KeyModifiers::ALT), "<A-z>");
    }

    #[test]
    fn test_vim_notation_shift() {
        assert_eq!(translate_to_vim_notation(KeyCode::Char('a'), KeyModifiers::SHIFT), "A");
        assert_eq!(translate_to_vim_notation(KeyCode::Enter, KeyModifiers::SHIFT), "<S-CR>");
    }

    #[test]
    fn test_vim_notation_ctrl_alt() {
        let mods = KeyModifiers::CONTROL | KeyModifiers::ALT;
        assert_eq!(translate_to_vim_notation(KeyCode::Char('a'), mods), "<C-A-a>");
    }

    #[test]
    fn test_vim_notation_ctrl_shift() {
        let mods = KeyModifiers::CONTROL | KeyModifiers::SHIFT;
        assert_eq!(translate_to_vim_notation(KeyCode::Char('a'), mods), "<C-A>");
    }

    #[test]
    fn test_vim_notation_all_modifiers() {
        let mods = KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SHIFT;
        let notation = translate_to_vim_notation(KeyCode::Char('a'), mods);
        assert!(notation.contains('C'));
        assert!(notation.contains('A'));
    }

    #[test]
    fn test_key_event_from_crossterm() {
        let ct_event = CrosstermKeyEvent::new(KeyCode::Char('j'), KeyModifiers::NONE);
        let key = KeyEvent::from_crossterm(ct_event);
        assert_eq!(key.code, KeyCode::Char('j'));
        assert_eq!(key.modifiers, KeyModifiers::NONE);
        assert_eq!(key.vim_notation, "j");
    }

    #[test]
    fn test_key_event_clone() {
        let key = KeyEvent {
            code: KeyCode::Char('x'),
            modifiers: KeyModifiers::NONE,
            vim_notation: "x".to_string(),
        };
        let cloned = key.clone();
        assert_eq!(key, cloned);
    }

    #[test]
    fn test_key_event_debug() {
        let key = KeyEvent {
            code: KeyCode::Char('a'),
            modifiers: KeyModifiers::NONE,
            vim_notation: "a".to_string(),
        };
        let debug = format!("{key:?}");
        assert!(debug.contains("KeyEvent"));
    }

    #[test]
    fn test_mouse_event_right_click() {
        let event = MouseEvent {
            column: 5,
            row: 10,
            kind: MouseEventKind::Down(MouseButton::Right),
        };
        assert!(event.is_right_click());
        assert!(!event.is_left_click());
        assert!(!event.is_scroll());
    }

    #[test]
    fn test_mouse_event_scroll_up() {
        let event = MouseEvent {
            column: 0,
            row: 0,
            kind: MouseEventKind::ScrollUp,
        };
        assert!(event.is_scroll());
        assert!(!event.is_left_click());
        assert!(!event.is_right_click());
    }

    #[test]
    fn test_mouse_event_scroll_down() {
        let event = MouseEvent {
            column: 0,
            row: 0,
            kind: MouseEventKind::ScrollDown,
        };
        assert!(event.is_scroll());
    }

    #[test]
    fn test_mouse_event_clone() {
        let event = MouseEvent {
            column: 1,
            row: 2,
            kind: MouseEventKind::Down(MouseButton::Left),
        };
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn test_mouse_event_debug() {
        let event = MouseEvent {
            column: 0,
            row: 0,
            kind: MouseEventKind::Down(MouseButton::Left),
        };
        let debug = format!("{event:?}");
        assert!(debug.contains("MouseEvent"));
    }

    #[test]
    fn test_resize_event_clone() {
        let event = ResizeEvent {
            width: 80,
            height: 24,
        };
        let cloned = event;
        assert_eq!(event, cloned);
    }

    #[test]
    fn test_resize_event_debug() {
        let event = ResizeEvent {
            width: 80,
            height: 24,
        };
        let debug = format!("{event:?}");
        assert!(debug.contains("ResizeEvent"));
    }

    #[test]
    fn test_input_event_clone() {
        let event = InputEvent::Key(KeyEvent {
            code: KeyCode::Char('a'),
            modifiers: KeyModifiers::NONE,
            vim_notation: "a".to_string(),
        });
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn test_input_event_debug() {
        let event = InputEvent::FocusGained;
        let debug = format!("{event:?}");
        assert!(debug.contains("FocusGained"));
    }

    #[test]
    fn test_input_event_focus_lost() {
        let event = InputEvent::FocusLost;
        assert!(matches!(event, InputEvent::FocusLost));
    }

    #[test]
    fn test_input_event_paste() {
        let event = InputEvent::Paste("hello".to_string());
        if let InputEvent::Paste(text) = event {
            assert_eq!(text, "hello");
        } else {
            panic!("Expected Paste event");
        }
    }

    // InputReader::new() and InputReader::default() require a TTY and cannot be unit tested
    // They are tested in integration tests instead

    #[test]
    fn test_vim_notation_unknown_keycode() {
        // KeyCode variants not in our match arm return empty string
        let notation = translate_to_vim_notation(KeyCode::Null, KeyModifiers::NONE);
        assert_eq!(notation, "");
    }

    #[test]
    fn test_vim_notation_shift_non_alpha() {
        // Shift + non-alpha key should include S- prefix
        assert_eq!(translate_to_vim_notation(KeyCode::Char(' '), KeyModifiers::SHIFT), "<S-Space>");
        assert_eq!(translate_to_vim_notation(KeyCode::Tab, KeyModifiers::SHIFT), "<S-Tab>");
    }

    #[test]
    fn test_vim_notation_ctrl_special_keys() {
        assert_eq!(translate_to_vim_notation(KeyCode::Left, KeyModifiers::CONTROL), "<C-Left>");
        assert_eq!(translate_to_vim_notation(KeyCode::Home, KeyModifiers::CONTROL), "<C-Home>");
    }

    #[test]
    fn test_vim_notation_alt_special_keys() {
        assert_eq!(translate_to_vim_notation(KeyCode::Up, KeyModifiers::ALT), "<A-Up>");
        assert_eq!(translate_to_vim_notation(KeyCode::F(5), KeyModifiers::ALT), "<A-F5>");
    }

    #[test]
    fn test_vim_notation_shift_special_keys() {
        assert_eq!(translate_to_vim_notation(KeyCode::Delete, KeyModifiers::SHIFT), "<S-Del>");
        assert_eq!(translate_to_vim_notation(KeyCode::Insert, KeyModifiers::SHIFT), "<S-Insert>");
        assert_eq!(translate_to_vim_notation(KeyCode::PageUp, KeyModifiers::SHIFT), "<S-PageUp>");
        assert_eq!(
            translate_to_vim_notation(KeyCode::PageDown, KeyModifiers::SHIFT),
            "<S-PageDown>"
        );
    }

    #[test]
    fn test_vim_notation_ctrl_alt_shift() {
        let mods = KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SHIFT;
        // Non-alpha char gets S- prefix
        assert_eq!(translate_to_vim_notation(KeyCode::Enter, mods), "<C-A-S-CR>");
    }

    #[test]
    fn test_vim_notation_shift_function_key() {
        assert_eq!(translate_to_vim_notation(KeyCode::F(1), KeyModifiers::SHIFT), "<S-F1>");
    }

    #[test]
    fn test_vim_notation_ctrl_shift_enter() {
        let mods = KeyModifiers::CONTROL | KeyModifiers::SHIFT;
        assert_eq!(translate_to_vim_notation(KeyCode::Enter, mods), "<C-S-CR>");
    }

    #[test]
    fn test_mouse_event_from_crossterm() {
        let ct_event = CrosstermMouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: 15,
            row: 20,
            modifiers: KeyModifiers::NONE,
        };
        let event = MouseEvent::from_crossterm(ct_event);
        assert_eq!(event.column, 15);
        assert_eq!(event.row, 20);
        assert!(event.is_left_click());
    }

    #[test]
    fn test_mouse_event_middle_click() {
        let event = MouseEvent {
            column: 0,
            row: 0,
            kind: MouseEventKind::Down(MouseButton::Middle),
        };
        assert!(!event.is_left_click());
        assert!(!event.is_right_click());
        assert!(!event.is_scroll());
    }

    #[test]
    fn test_mouse_event_drag() {
        let event = MouseEvent {
            column: 5,
            row: 5,
            kind: MouseEventKind::Drag(MouseButton::Left),
        };
        assert!(!event.is_left_click());
        assert!(!event.is_right_click());
        assert!(!event.is_scroll());
    }

    #[test]
    fn test_mouse_event_moved() {
        let event = MouseEvent {
            column: 10,
            row: 10,
            kind: MouseEventKind::Moved,
        };
        assert!(!event.is_left_click());
        assert!(!event.is_right_click());
        assert!(!event.is_scroll());
    }

    #[test]
    fn test_input_event_variants_debug() {
        let mouse = InputEvent::Mouse(MouseEvent {
            column: 0,
            row: 0,
            kind: MouseEventKind::Down(MouseButton::Left),
        });
        let debug = format!("{mouse:?}");
        assert!(debug.contains("Mouse"));

        let resize = InputEvent::Resize(ResizeEvent {
            width: 80,
            height: 24,
        });
        let debug = format!("{resize:?}");
        assert!(debug.contains("Resize"));
    }

    #[test]
    fn test_input_event_variants_eq() {
        let e1 = InputEvent::FocusGained;
        let e2 = InputEvent::FocusGained;
        assert_eq!(e1, e2);

        let e3 = InputEvent::FocusLost;
        assert_ne!(e1, e3);
    }

    #[test]
    fn test_input_event_resize() {
        let event = InputEvent::Resize(ResizeEvent {
            width: 100,
            height: 50,
        });
        if let InputEvent::Resize(r) = event {
            assert_eq!(r.width, 100);
            assert_eq!(r.height, 50);
        } else {
            panic!("Expected Resize event");
        }
    }

    #[test]
    fn test_input_event_mouse() {
        let event = InputEvent::Mouse(MouseEvent {
            column: 42,
            row: 10,
            kind: MouseEventKind::ScrollUp,
        });
        if let InputEvent::Mouse(m) = &event {
            assert_eq!(m.column, 42);
            assert!(m.is_scroll());
        } else {
            panic!("Expected Mouse event");
        }
        // Clone and equality
        let cloned = event.clone();
        assert_eq!(event, cloned);
    }

    #[test]
    fn test_key_event_is_shift() {
        let key = KeyEvent {
            code: KeyCode::Char('A'),
            modifiers: KeyModifiers::SHIFT,
            vim_notation: "A".to_string(),
        };
        assert!(key.is_shift());
        assert!(!key.is_ctrl());
        assert!(!key.is_alt());
    }

    #[test]
    fn test_key_event_from_crossterm_special() {
        let ct_event = CrosstermKeyEvent::new(KeyCode::Esc, KeyModifiers::NONE);
        let key = KeyEvent::from_crossterm(ct_event);
        assert_eq!(key.vim_notation, "<Esc>");

        let ct_event = CrosstermKeyEvent::new(KeyCode::Enter, KeyModifiers::CONTROL);
        let key = KeyEvent::from_crossterm(ct_event);
        assert_eq!(key.vim_notation, "<C-CR>");
    }

    #[test]
    fn test_vim_notation_backspace_with_modifiers() {
        assert_eq!(translate_to_vim_notation(KeyCode::Backspace, KeyModifiers::CONTROL), "<C-BS>");
        assert_eq!(translate_to_vim_notation(KeyCode::Backspace, KeyModifiers::ALT), "<A-BS>");
    }

    #[test]
    fn test_vim_notation_esc_with_modifiers() {
        assert_eq!(translate_to_vim_notation(KeyCode::Esc, KeyModifiers::CONTROL), "<C-Esc>");
    }

    #[test]
    fn test_vim_notation_char_tab() {
        // '\t' as Char should map to Tab
        assert_eq!(
            translate_to_vim_notation(KeyCode::Char('\t'), KeyModifiers::CONTROL),
            "<C-Tab>"
        );
    }

    #[test]
    fn test_vim_notation_space_with_ctrl() {
        assert_eq!(
            translate_to_vim_notation(KeyCode::Char(' '), KeyModifiers::CONTROL),
            "<C-Space>"
        );
    }

    #[test]
    fn test_vim_notation_arrows_with_shift() {
        assert_eq!(translate_to_vim_notation(KeyCode::Left, KeyModifiers::SHIFT), "<S-Left>");
        assert_eq!(translate_to_vim_notation(KeyCode::Right, KeyModifiers::SHIFT), "<S-Right>");
        assert_eq!(translate_to_vim_notation(KeyCode::Down, KeyModifiers::SHIFT), "<S-Down>");
    }

    #[test]
    fn test_vim_notation_end_home_with_ctrl() {
        assert_eq!(translate_to_vim_notation(KeyCode::End, KeyModifiers::CONTROL), "<C-End>");
    }

    // InputReader::new(), InputReader::default(), and Debug impl require a real
    // terminal (crossterm's EventStream panics without one). They are covered
    // by integration tests instead.
}
