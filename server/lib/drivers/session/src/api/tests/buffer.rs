use super::*;

struct StubBufferApi {
    content: Option<String>,
    caps: Option<reovim_kernel::api::v1::BufferCapabilities>,
}

impl BufferApi for StubBufferApi {
    fn active_buffer(&self) -> Option<BufferId> {
        Some(BufferId::new())
    }

    fn set_active_buffer(&mut self, _id: Option<BufferId>) {}

    fn buffer_line(&self, _buffer: BufferId, _line: usize) -> Option<String> {
        None
    }

    fn buffer_line_count(&self, _buffer: BufferId) -> Option<usize> {
        None
    }

    fn buffer_line_len(&self, _buffer: BufferId, _line: usize) -> Option<usize> {
        None
    }

    fn buffer_text_range(
        &self,
        _buffer: BufferId,
        _start: Position,
        _end: Position,
    ) -> Option<String> {
        None
    }

    fn buffer_content(&self, _buffer: BufferId) -> Option<String> {
        self.content.clone()
    }

    fn buffer_file_path(&self, _buffer: BufferId) -> Option<String> {
        None
    }

    fn is_buffer_modified(&self, _buffer: BufferId) -> Option<bool> {
        None
    }

    fn set_buffer_modified(&mut self, _buffer: BufferId, _modified: bool) {}

    fn insert_text(&mut self, _buffer: BufferId, _pos: Position, _text: &str) {}

    fn delete_range(&mut self, _buffer: BufferId, _start: Position, _end: Position) {}

    fn replace_content(&mut self, _buffer: BufferId, _content: &str) {}

    fn create_buffer(&mut self, _name: Option<&str>, _content: &str) -> BufferId {
        BufferId::new()
    }

    fn delete_buffer(&mut self, _buffer: BufferId) -> Result<(), BufferError> {
        Ok(())
    }

    fn rename_buffer(&mut self, _buffer: BufferId, _new_name: &str) {}

    fn buffer_capabilities(
        &self,
        _buffer: BufferId,
    ) -> Option<reovim_kernel::api::v1::BufferCapabilities> {
        self.caps
    }
}

#[test]
fn test_selection_modes() {
    let start = Position::new(0, 0);
    let end = Position::new(0, 5);

    let char_sel = Selection::character(start, end);
    assert!(!char_sel.is_linewise());
    assert_eq!(char_sel.mode, SelectionMode::Character);

    let line_sel = Selection::line(start, end);
    assert!(line_sel.is_linewise());
    assert_eq!(line_sel.mode, SelectionMode::Line);

    let block_sel = Selection::block(start, end);
    assert!(!block_sel.is_linewise());
    assert_eq!(block_sel.mode, SelectionMode::Block);
}

#[test]
fn test_buffer_error_display() {
    let err = BufferError::CannotDeleteLastBuffer;
    assert_eq!(err.to_string(), "cannot delete last buffer");

    let id = BufferId::new();
    let err = BufferError::NotFound(id);
    assert!(err.to_string().contains("buffer not found"));
}

#[test]
fn test_selection_mode_default() {
    let mode = SelectionMode::default();
    assert_eq!(mode, SelectionMode::Character);
}

#[test]
fn test_selection_new() {
    let start = Position::new(1, 2);
    let end = Position::new(3, 4);
    let sel = Selection::new(start, end, SelectionMode::Line);
    assert_eq!(sel.start, start);
    assert_eq!(sel.end, end);
    assert_eq!(sel.mode, SelectionMode::Line);
    assert!(sel.is_linewise());
}

#[test]
fn test_selection_character() {
    let start = Position::new(0, 0);
    let end = Position::new(0, 10);
    let sel = Selection::character(start, end);
    assert_eq!(sel.mode, SelectionMode::Character);
    assert!(!sel.is_linewise());
}

#[test]
fn test_selection_line() {
    let start = Position::new(0, 0);
    let end = Position::new(5, 0);
    let sel = Selection::line(start, end);
    assert_eq!(sel.mode, SelectionMode::Line);
    assert!(sel.is_linewise());
}

#[test]
fn test_selection_block() {
    let start = Position::new(0, 0);
    let end = Position::new(5, 10);
    let sel = Selection::block(start, end);
    assert_eq!(sel.mode, SelectionMode::Block);
    assert!(!sel.is_linewise());
}

#[test]
fn test_selection_equality() {
    let a = Selection::character(Position::new(0, 0), Position::new(0, 5));
    let b = Selection::character(Position::new(0, 0), Position::new(0, 5));
    assert_eq!(a, b);
}

#[test]
fn test_selection_inequality() {
    let a = Selection::character(Position::new(0, 0), Position::new(0, 5));
    let b = Selection::line(Position::new(0, 0), Position::new(0, 5));
    assert_ne!(a, b);
}

#[test]
fn test_selection_clone() {
    let sel = Selection::character(Position::new(1, 2), Position::new(3, 4));
    let cloned = sel.clone();
    assert_eq!(sel, cloned);
}

#[test]
fn test_selection_debug() {
    let sel = Selection::character(Position::new(0, 0), Position::new(0, 5));
    let debug = format!("{sel:?}");
    assert!(debug.contains("Selection"));
}

#[test]
fn test_buffer_error_is_std_error() {
    let err: Box<dyn std::error::Error> = Box::new(BufferError::CannotDeleteLastBuffer);
    assert_eq!(err.to_string(), "cannot delete last buffer");
}

#[test]
fn test_buffer_error_not_found_display() {
    let id = BufferId::new();
    let err = BufferError::NotFound(id);
    let display = err.to_string();
    assert!(display.starts_with("buffer not found"));
}

#[test]
fn test_buffer_error_clone() {
    let err = BufferError::CannotDeleteLastBuffer;
    let cloned = err.clone();
    assert_eq!(err, cloned);
}

#[test]
fn test_buffer_error_eq() {
    assert_eq!(BufferError::CannotDeleteLastBuffer, BufferError::CannotDeleteLastBuffer);
}

#[test]
fn test_buffer_content_if_materializable_returns_content() {
    let api = StubBufferApi {
        content: Some("hello".to_string()),
        caps: Some(reovim_kernel::api::v1::BufferCapabilities::ROPE),
    };

    assert_eq!(api.buffer_content_if_materializable(BufferId::new()), Some("hello".to_string()));
}

#[test]
fn test_buffer_content_if_materializable_skips_virtual() {
    let api = StubBufferApi {
        content: Some("hello".to_string()),
        caps: Some(reovim_kernel::api::v1::BufferCapabilities::VIRTUAL),
    };

    assert_eq!(api.buffer_content_if_materializable(BufferId::new()), None);
}

#[test]
fn test_buffer_content_if_materializable_falls_back_without_caps() {
    let api = StubBufferApi {
        content: Some("hello".to_string()),
        caps: None,
    };

    assert_eq!(api.buffer_content_if_materializable(BufferId::new()), Some("hello".to_string()));
}

#[test]
fn test_selection_mode_clone_copy() {
    let mode = SelectionMode::Block;
    let copied = mode;
    assert_eq!(mode, copied);
}
