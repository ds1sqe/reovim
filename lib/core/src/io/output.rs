//! Output abstraction for terminal rendering
//!
//! Provides:
//! - `MockOutput` for capturing rendered output in tests
//! - `DualOutput` for server mode (writes to terminal AND capture buffer)
//! - `strip_ansi_codes` for removing ANSI escape sequences

use std::{
    io::{self, Write},
    sync::{Arc, Mutex},
};

/// Mock output that captures all written bytes.
///
/// This is used in tests to capture the output of `Screen::render()`
/// and make assertions about the display content.
#[derive(Clone, Debug)]
pub struct MockOutput {
    buffer: Arc<Mutex<Vec<u8>>>,
}

impl MockOutput {
    /// Create a new mock output buffer.
    #[must_use]
    pub fn new() -> Self {
        Self {
            buffer: Arc::new(Mutex::new(Vec::new())),
        }
    }

    /// Get all captured output as a string.
    ///
    /// Invalid UTF-8 sequences are replaced with the replacement character.
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned.
    #[must_use]
    pub fn to_string_lossy(&self) -> String {
        let buf = self.buffer.lock().unwrap();
        String::from_utf8_lossy(&buf).to_string()
    }

    /// Get raw captured bytes.
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        self.buffer.lock().unwrap().clone()
    }

    /// Clear captured output.
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned.
    pub fn clear(&self) {
        self.buffer.lock().unwrap().clear();
    }

    /// Check if output contains a string (after stripping ANSI codes).
    #[must_use]
    pub fn contains(&self, needle: &str) -> bool {
        self.strip_ansi().contains(needle)
    }

    /// Get output with ANSI escape sequences stripped.
    ///
    /// This removes all ANSI escape sequences (colors, cursor movement, etc.)
    /// leaving only the text content.
    #[must_use]
    pub fn strip_ansi(&self) -> String {
        let raw = self.to_string_lossy();
        strip_ansi_codes(&raw)
    }

    /// Get the number of bytes captured.
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned.
    #[must_use]
    pub fn len(&self) -> usize {
        self.buffer.lock().unwrap().len()
    }

    /// Check if the buffer is empty.
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.buffer.lock().unwrap().is_empty()
    }
}

impl Default for MockOutput {
    fn default() -> Self {
        Self::new()
    }
}

impl Write for MockOutput {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        self.buffer.lock().unwrap().extend_from_slice(buf);
        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

/// Dual output that writes to both a primary output (terminal) and a capture buffer.
///
/// Used in server mode to render to terminal while also capturing output for RPC clients.
/// The capture buffer is shared via `Arc<Mutex<>>` so it can be read by the server.
///
/// **Important**: Writes are buffered and only sent to the terminal on `flush()`.
/// This prevents flickering caused by many small writes.
pub struct DualOutput {
    /// Primary output (typically stdout)
    primary: Option<Box<dyn Write + Send>>,
    /// Capture buffer for RPC clients
    capture: Arc<Mutex<Vec<u8>>>,
    /// Write buffer for batching writes to terminal (reduces flicker)
    write_buffer: Vec<u8>,
}

impl DualOutput {
    /// Create a dual output with terminal and capture buffer
    #[must_use]
    pub fn new(primary: Box<dyn Write + Send>) -> Self {
        Self {
            primary: Some(primary),
            capture: Arc::new(Mutex::new(Vec::new())),
            write_buffer: Vec::with_capacity(64 * 1024), // 64KB buffer
        }
    }

    /// Create a headless dual output (capture only, no terminal)
    #[must_use]
    pub fn headless() -> Self {
        Self {
            primary: None,
            capture: Arc::new(Mutex::new(Vec::new())),
            write_buffer: Vec::new(),
        }
    }

    /// Get a handle to the capture buffer for reading
    #[must_use]
    pub fn capture_handle(&self) -> CaptureHandle {
        CaptureHandle {
            buffer: Arc::clone(&self.capture),
        }
    }

    /// Clear the capture buffer
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned.
    pub fn clear_capture(&self) {
        self.capture.lock().unwrap().clear();
    }
}

impl Write for DualOutput {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        // Write to capture buffer
        self.capture.lock().unwrap().extend_from_slice(buf);

        // Buffer writes for terminal (flushed on flush())
        if self.primary.is_some() {
            self.write_buffer.extend_from_slice(buf);
        }

        Ok(buf.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        // Write buffered content to terminal all at once
        if let Some(ref mut primary) = self.primary {
            if !self.write_buffer.is_empty() {
                primary.write_all(&self.write_buffer)?;
                self.write_buffer.clear();
            }
            primary.flush()?;
        }
        Ok(())
    }
}

/// Handle to read captured output from a `DualOutput`
#[derive(Clone)]
pub struct CaptureHandle {
    buffer: Arc<Mutex<Vec<u8>>>,
}

impl CaptureHandle {
    /// Get captured output as raw bytes
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        self.buffer.lock().unwrap().clone()
    }

    /// Get captured output as a string (lossy conversion)
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned.
    #[must_use]
    pub fn to_string_lossy(&self) -> String {
        let buf = self.buffer.lock().unwrap();
        String::from_utf8_lossy(&buf).to_string()
    }

    /// Get captured output with ANSI codes stripped
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned.
    #[must_use]
    pub fn strip_ansi(&self) -> String {
        strip_ansi_codes(&self.to_string_lossy())
    }

    /// Check if the capture buffer is empty
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.buffer.lock().unwrap().is_empty()
    }

    /// Get the length of captured bytes
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned.
    #[must_use]
    pub fn len(&self) -> usize {
        self.buffer.lock().unwrap().len()
    }

    /// Clear the capture buffer
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned.
    pub fn clear(&self) {
        self.buffer.lock().unwrap().clear();
    }

    /// Take a snapshot of the buffer and clear it atomically
    ///
    /// This returns the current buffer contents and clears the buffer,
    /// ensuring each read gets only the latest frame without accumulation.
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned.
    #[must_use]
    pub fn take_snapshot(&self) -> Vec<u8> {
        let mut guard = self.buffer.lock().unwrap();
        std::mem::take(&mut *guard)
    }

    /// Take a snapshot as string (lossy conversion) and clear
    ///
    /// # Panics
    ///
    /// Panics if the mutex is poisoned.
    #[must_use]
    pub fn take_snapshot_lossy(&self) -> String {
        String::from_utf8_lossy(&self.take_snapshot()).to_string()
    }
}

/// Strip ANSI escape sequences from a string.
///
/// Handles:
/// - CSI sequences: `\x1b[...m` (colors, styles)
/// - CSI sequences: `\x1b[...H` (cursor positioning)
/// - CSI sequences: `\x1b[...J`, `\x1b[...K` (clear screen/line)
/// - Other CSI sequences: `\x1b[...X` where X is any letter
#[must_use]
pub fn strip_ansi_codes(s: &str) -> String {
    let mut result = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\x1b' {
            // Check for CSI sequence (ESC [)
            if chars.peek() == Some(&'[') {
                chars.next(); // consume '['
                // Skip until we find a letter (the terminator)
                while let Some(&next) = chars.peek() {
                    chars.next();
                    if next.is_ascii_alphabetic() {
                        break;
                    }
                }
            } else {
                // Other escape sequence - skip next char
                chars.next();
            }
        } else {
            result.push(c);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mock_output_captures_bytes() {
        let mut output = MockOutput::new();
        output.write_all(b"hello").unwrap();
        output.write_all(b" world").unwrap();

        assert_eq!(output.to_string_lossy(), "hello world");
        assert_eq!(output.len(), 11);
    }

    #[test]
    fn test_mock_output_clone_shares_buffer() {
        let mut output1 = MockOutput::new();
        let output2 = output1.clone();

        output1.write_all(b"test").unwrap();
        assert_eq!(output2.to_string_lossy(), "test");
    }

    #[test]
    fn test_mock_output_clear() {
        let mut output = MockOutput::new();
        output.write_all(b"hello").unwrap();
        output.clear();
        assert!(output.is_empty());
    }

    #[test]
    fn test_strip_ansi_removes_color_codes() {
        let input = "\x1b[31mred\x1b[0m normal \x1b[1;32mbold green\x1b[0m";
        let stripped = strip_ansi_codes(input);
        assert_eq!(stripped, "red normal bold green");
    }

    #[test]
    fn test_strip_ansi_removes_cursor_codes() {
        let input = "\x1b[1;1Hstart\x1b[10;20Hend";
        let stripped = strip_ansi_codes(input);
        assert_eq!(stripped, "startend");
    }

    #[test]
    fn test_strip_ansi_removes_clear_codes() {
        let input = "\x1b[2Jcleared\x1b[Kline";
        let stripped = strip_ansi_codes(input);
        assert_eq!(stripped, "clearedline");
    }

    #[test]
    fn test_mock_output_contains() {
        let mut output = MockOutput::new();
        output.write_all(b"\x1b[31mhello\x1b[0m world").unwrap();

        assert!(output.contains("hello"));
        assert!(output.contains("world"));
        assert!(output.contains("hello world"));
        assert!(!output.contains("goodbye"));
    }
}
