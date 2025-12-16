//! Text manipulation operations for buffers

/// Text manipulation operations for Buffer
pub trait TextOps {
    /// Set buffer content from string
    fn set_content(&mut self, content: &str);
    /// Convert buffer content to string
    fn content_to_string(&self) -> String;
    /// Insert a character at cursor position
    fn insert_char(&mut self, c: char);
    /// Insert a newline at cursor position (Enter key)
    fn insert_newline(&mut self);
    /// Delete character before cursor (backspace)
    fn delete_char_backward(&mut self);
    /// Delete character at cursor position (delete)
    fn delete_char_forward(&mut self);
    /// Delete entire current line
    fn delete_line(&mut self);
}
