use {
    super::*,
    crate::api::{BufferApi, ModeApi},
};

#[test]
fn test_new_creates_valid_runtime() {
    let mut test = TestSessionRuntime::new();
    let _runtime = test.runtime();
    test.assert_mode_name("normal");
    test.assert_mode_depth(1);
}

#[test]
fn test_with_buffer_creates_buffer_and_window() {
    let test = TestSessionRuntime::with_buffer("hello world");
    test.assert_buffer_content("hello world");
    test.assert_window_count(1);
}

#[test]
fn test_mode_operations() {
    let mut test = TestSessionRuntime::new();
    let insert_mode = ModeId::new(ModuleId::new("test"), "insert");

    test.with_runtime(|runtime| {
        runtime.push_mode(insert_mode.clone(), crate::TransitionContext::new());
    });

    test.assert_mode(&insert_mode);
    test.assert_mode_name("insert");
    test.assert_mode_depth(2);

    let changes = test.take_changes();
    assert!(changes.mode_changed);
}

#[cfg_attr(coverage_nightly, coverage(off))]
#[test]
fn test_buffer_operations() {
    use crate::api::BufferApi;

    let mut test = TestSessionRuntime::with_buffer("hello");

    test.with_runtime(|runtime| {
        if let Some(buffer_id) = runtime.active_buffer() {
            runtime.insert_text(buffer_id, Position::new(0, 5), " world");
        }
    });

    test.assert_buffer_content("hello world");

    let changes = test.take_changes();
    assert!(changes.buffer_modified);
}

/// Test cursor manipulation via per-client window state (#471).
///
/// Cursor is now a per-WINDOW property, not per-buffer. Commands update
/// cursor via `runtime.windows_mut().active_mut()?.cursor`.
#[test]
fn test_cursor_operations() {
    let mut test = TestSessionRuntime::with_buffer("hello\nworld");

    // Initial cursor should be at (0, 0)
    test.assert_cursor(0, 0);

    // Manually set cursor via window (simulating what a command would do)
    // Use test.windows (separate field), NOT test.session.windows
    test.windows.active_mut().unwrap().cursor = crate::CursorPosition::new(1, 3);

    // Verify cursor was updated
    test.assert_cursor(1, 3);
}

#[test]
fn test_with_home_mode() {
    let custom_mode = ModeId::new(ModuleId::new("custom"), "special");
    let test = TestSessionRuntime::with_home_mode(custom_mode.clone());
    test.assert_mode(&custom_mode);
}

/// Test cursor position via per-client window state (#471).
///
/// Post-#471: Cursor is per-WINDOW, not per-buffer. Access via
/// `test.windows.active()?.cursor` (read) or
/// `test.windows.active_mut()?.cursor = ...` (write).
///
/// Commands should use `runtime.windows()` to access the
/// client's window state.
#[test]
fn test_cursor_position_via_window() {
    let mut test = TestSessionRuntime::with_buffer("hello\nworld");
    let _buffer_id = test.active_buffer().expect("should have active buffer");

    // Initial position should be (0, 0)
    // Use test.windows (separate field), NOT test.session.windows
    let window = test.windows.active().expect("should have active window");
    assert_eq!((window.cursor.line, window.cursor.column), (0, 0));

    // Set new position via window
    test.windows.active_mut().unwrap().cursor = crate::CursorPosition::new(1, 3);

    // Verify new position
    let window = test.windows.active().expect("should have active window");
    assert_eq!((window.cursor.line, window.cursor.column), (1, 3));

    // Window without buffer still has cursor
    let mut layout = crate::WindowLayout::empty();
    layout.add(crate::Window::new()); // Empty window (no buffer)
    assert!(layout.active().is_some()); // Window exists
    assert_eq!(layout.active().unwrap().buffer_id, None); // But no buffer
}

#[test]
fn test_buffer_line_len_api() {
    use crate::api::BufferApi;

    let mut test = TestSessionRuntime::with_buffer("hello\nworld!");
    let buffer_id = test.active_buffer().expect("should have active buffer");

    test.with_runtime(|runtime| {
        // Check line lengths
        assert_eq!(runtime.buffer_line_len(buffer_id, 0), Some(5)); // "hello"
        assert_eq!(runtime.buffer_line_len(buffer_id, 1), Some(6)); // "world!"

        // Out of bounds line
        assert!(runtime.buffer_line_len(buffer_id, 99).is_none());

        // Non-existent buffer
        let fake_id = BufferId::new();
        assert!(runtime.buffer_line_len(fake_id, 0).is_none());
    });
}

// NOTE: test_buffer_position_vs_cursor_position removed (#471)
// There is now only ONE cursor location: per-client Window.cursor
// The kernel Buffer no longer has a cursor field.

#[test]
fn test_register_api() {
    use crate::api::{RegisterApi, RegisterContent};

    let mut test = TestSessionRuntime::new();

    test.with_runtime(|runtime| {
        // Named registers should be initially empty
        assert!(runtime.get_register(Some('z')).is_none());

        // Set unnamed register
        runtime.set_register(None, RegisterContent::characterwise("hello"));
        let content = runtime.get_register(None).expect("should have content");
        assert_eq!(content.text, "hello");
        assert!(content.is_characterwise());

        // Set named register
        runtime.set_register(Some('a'), RegisterContent::linewise("world\n"));
        let content = runtime
            .get_register(Some('a'))
            .expect("should have content");
        assert_eq!(content.text, "world\n");
        assert!(content.is_linewise());

        // Overwrite unnamed register
        runtime.set_register(None, RegisterContent::linewise("replaced\n"));
        let content = runtime.get_register(None).expect("should have content");
        assert_eq!(content.text, "replaced\n");
        assert!(content.is_linewise());

        // Named register unchanged
        assert_eq!(runtime.get_register(Some('a')).unwrap().text, "world\n");
    });
}

// =========================================================================
// TestSessionRuntime assertion tests
// =========================================================================

#[test]
fn test_assert_mode() {
    let mode = ModeId::new(ModuleId::new("test"), "normal");
    let test = TestSessionRuntime::new();
    test.assert_mode(&mode); // Should not panic
}

#[test]
fn test_assert_mode_name() {
    let test = TestSessionRuntime::new();
    test.assert_mode_name("normal"); // Should not panic
}

#[test]
fn test_assert_mode_depth() {
    let test = TestSessionRuntime::new();
    test.assert_mode_depth(1); // Should not panic
}

#[test]
fn test_assert_window_count_empty() {
    let test = TestSessionRuntime::new();
    test.assert_window_count(0); // No windows initially
}

#[test]
fn test_assert_window_count_with_buffer() {
    let test = TestSessionRuntime::with_buffer("hello");
    test.assert_window_count(1);
}

#[test]
fn test_assert_line_count() {
    let test = TestSessionRuntime::with_buffer("line1\nline2\nline3");
    test.assert_line_count(3);
}

#[test]
fn test_assert_buffer_content() {
    let test = TestSessionRuntime::with_buffer("hello world");
    test.assert_buffer_content("hello world");
}

#[test]
fn test_assert_cursor_default() {
    let test = TestSessionRuntime::with_buffer("hello");
    test.assert_cursor(0, 0);
}

#[test]
fn test_current_mode_getter() {
    let test = TestSessionRuntime::new();
    assert_eq!(test.current_mode().name(), "normal");
}

#[test]
fn test_active_buffer_getter() {
    let test = TestSessionRuntime::with_buffer("test");
    assert!(test.active_buffer().is_some());
}

#[test]
fn test_active_buffer_none_when_no_buffer() {
    let test = TestSessionRuntime::new();
    assert!(test.active_buffer().is_none());
}

#[test]
fn test_cursor_position_getter() {
    let test = TestSessionRuntime::with_buffer("hello");
    assert_eq!(test.cursor_position(), Some(Position::new(0, 0)));
}

#[test]
fn test_cursor_position_none_when_no_window() {
    let test = TestSessionRuntime::new();
    assert!(test.cursor_position().is_none());
}

#[test]
fn test_buffer_content_getter() {
    let test = TestSessionRuntime::with_buffer("hello world");
    assert_eq!(test.buffer_content(), Some("hello world".to_string()));
}

#[test]
fn test_buffer_content_none_when_no_buffer() {
    let test = TestSessionRuntime::new();
    assert!(test.buffer_content().is_none());
}

#[test]
fn test_kernel_getter() {
    let test = TestSessionRuntime::new();
    let _kernel = test.kernel(); // Should not panic
}

#[test]
fn test_session_getter() {
    let test = TestSessionRuntime::new();
    let session = test.session();
    assert_eq!(session.id.as_usize(), 1);
}

#[test]
fn test_changes_getter() {
    let test = TestSessionRuntime::new();
    let changes = test.changes();
    assert!(!changes.has_changes());
}

#[test]
fn test_take_changes_resets() {
    let mut test = TestSessionRuntime::new();
    let insert_mode = ModeId::new(ModuleId::new("test"), "insert");

    test.with_runtime(|runtime| {
        runtime.push_mode(insert_mode, crate::TransitionContext::new());
    });

    // Changes should be recorded
    assert!(test.changes().mode_changed);

    // Take should clear
    let changes = test.take_changes();
    assert!(changes.mode_changed);
    assert!(!test.changes().has_changes());
}

#[test]
fn test_runtime_method() {
    let mut test = TestSessionRuntime::new();
    let runtime = test.runtime();
    // Just verify it returns a valid runtime
    assert_eq!(runtime.current_mode().name(), "normal");
}

#[test]
fn test_with_runtime_returns_value() {
    let mut test = TestSessionRuntime::with_buffer("hello");
    let line_count = test.with_runtime(|runtime| {
        let buf = runtime.active_buffer().unwrap();
        runtime.buffer_line_count(buf).unwrap()
    });
    assert_eq!(line_count, 1);
}

#[test]
fn test_default_impl() {
    let test = TestSessionRuntime::default();
    test.assert_mode_name("normal");
    test.assert_mode_depth(1);
}

// =========================================================================
// Buffer manager integration tests
// =========================================================================

#[test]
fn test_buffer_manager_create() {
    let test = TestSessionRuntime::new();
    // Creating a buffer via runtime should work
    let mut test = test;
    let buf_id = test.with_runtime(|runtime| {
        use crate::api::BufferApi;
        runtime.create_buffer(None, "test content")
    });

    let content = test.with_runtime(|runtime| {
        use crate::api::BufferApi;
        runtime.buffer_content(buf_id)
    });
    assert_eq!(content, Some("test content".to_string()));
}

#[test]
fn test_buffer_manager_list() {
    let mut test = TestSessionRuntime::new();

    test.with_runtime(|runtime| {
        use crate::api::BufferApi;
        runtime.create_buffer(None, "buf1");
        runtime.create_buffer(None, "buf2");
    });

    // Kernel should have buffers
    let count = test.kernel().buffers.count();
    assert!(count >= 2);
}

#[test]
fn test_with_home_mode_custom() {
    let custom = ModeId::new(ModuleId::new("custom"), "special-mode");
    let test = TestSessionRuntime::with_home_mode(custom.clone());
    test.assert_mode(&custom);
    test.assert_mode_name("special-mode");
}
