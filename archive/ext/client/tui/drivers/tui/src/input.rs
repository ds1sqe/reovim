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
pub enum PlatformEvent {
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

#[cfg_attr(coverage_nightly, coverage(off))]
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
    pub async fn next_event(&mut self) -> Option<PlatformEvent> {
        match self.stream.next().await {
            Some(Ok(event)) => {
                let input_event = match event {
                    CrosstermEvent::Key(key) => PlatformEvent::Key(KeyEvent::from_crossterm(key)),
                    CrosstermEvent::Mouse(mouse) => {
                        PlatformEvent::Mouse(MouseEvent::from_crossterm(mouse))
                    }
                    CrosstermEvent::Resize(width, height) => {
                        PlatformEvent::Resize(ResizeEvent { width, height })
                    }
                    CrosstermEvent::FocusGained => PlatformEvent::FocusGained,
                    CrosstermEvent::FocusLost => PlatformEvent::FocusLost,
                    CrosstermEvent::Paste(text) => PlatformEvent::Paste(text),
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
    ) -> std::io::Result<Option<PlatformEvent>> {
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

#[cfg_attr(coverage_nightly, coverage(off))]
impl Default for InputReader {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl std::fmt::Debug for InputReader {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("InputReader").finish()
    }
}

/// Translate crossterm key event to vim notation.
#[must_use]
#[cfg_attr(coverage_nightly, coverage(off))]
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
#[path = "input_tests.rs"]
mod tests;
