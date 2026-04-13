use super::*;

use std::{ops::Range, sync::Arc};

use reovim_driver_syntax::{
    Annotation, HighlightCategory, SyntaxDriver, SyntaxDriverFactory, SyntaxEdit,
};

/// A minimal test driver for unit tests.
struct TestDriver {
    language: String,
    parsed: bool,
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl TestDriver {
    fn new(language: &str) -> Self {
        Self {
            language: language.to_string(),
            parsed: false,
        }
    }
}

#[cfg_attr(coverage_nightly, coverage(off))]
impl SyntaxDriver for TestDriver {
    fn language(&self) -> &str {
        &self.language
    }

    fn parse(&mut self, _content: &str) {
        self.parsed = true;
    }

    fn update(&mut self, _content: &str, _edit: &SyntaxEdit) {
        // No-op
    }

    fn highlights(&self, byte_range: Range<usize>) -> Vec<Annotation> {
        if self.parsed {
            vec![Annotation::new(
                byte_range.start,
                byte_range.end,
                HighlightCategory::new("comment"),
            )]
        } else {
            Vec::new()
        }
    }

    fn is_parsed(&self) -> bool {
        self.parsed
    }
}

/// A minimal test factory.
struct TestFactory;

#[cfg_attr(coverage_nightly, coverage(off))]
impl SyntaxDriverFactory for TestFactory {
    fn create(&self, language_id: &str) -> Option<Box<dyn SyntaxDriver>> {
        if language_id == "rust" {
            Some(Box::new(TestDriver::new("rust")))
        } else {
            None
        }
    }

    fn supported_languages(&self) -> Vec<&str> {
        vec!["rust"]
    }

    fn supports(&self, language_id: &str) -> bool {
        language_id == "rust"
    }
}

fn buffer_id(n: usize) -> BufferId {
    BufferId::from_raw(n)
}

// ========================================================================
// SyntaxStreamState tests
// ========================================================================

#[test]
fn test_stream_state_new() {
    let state = SyntaxStreamState::new();
    assert_eq!(state.subscriber_count(), 0);
    assert!(!state.has_subscribers());
}

#[test]
fn test_stream_state_session_extension() {
    let state = SyntaxStreamState::create();
    assert_eq!(state.subscriber_count(), 0);
}

#[test]
fn test_subscribe() {
    let mut state = SyntaxStreamState::new();
    assert_eq!(state.subscriber_count(), 0);

    let _rx1 = state.subscribe();
    assert_eq!(state.subscriber_count(), 1);
    assert!(state.has_subscribers());

    let _rx2 = state.subscribe();
    assert_eq!(state.subscriber_count(), 2);
}

#[test]
fn test_broadcast() {
    let mut state = SyntaxStreamState::new();
    let mut rx = state.subscribe();

    let update = TokenUpdate {
        buffer_id: 1,
        tokens: vec![],
        start_line: 0,
        end_line: 0,
        full_refresh: false,
        layer: "syntax".into(),
        priority: 0,
    };

    state.broadcast(&update);

    let received = rx.try_recv().expect("Should receive update");
    assert_eq!(received.buffer_id, 1);
    assert_eq!(received.layer, "syntax");
    assert_eq!(received.priority, 0);
}

#[test]
fn test_broadcast_removes_disconnected() {
    let mut state = SyntaxStreamState::new();
    let rx = state.subscribe();
    assert_eq!(state.subscriber_count(), 1);

    // Drop the receiver to disconnect
    drop(rx);

    let update = TokenUpdate {
        buffer_id: 1,
        tokens: vec![],
        start_line: 0,
        end_line: 0,
        full_refresh: false,
        layer: "syntax".into(),
        priority: 0,
    };

    state.broadcast(&update);

    // Disconnected subscriber should be removed
    assert_eq!(state.subscriber_count(), 0);
}

#[tokio::test]
async fn test_notify_edit_with_subscriber() {
    let mut syntax = SyntaxSessionState::new();
    let mut stream = SyntaxStreamState::new();
    let id = buffer_id(1);

    // Set up a driver
    syntax.set(id, Box::new(TestDriver::new("rust")));
    syntax.get_mut(id).unwrap().parse("fn main() {}");

    // Subscribe
    let mut rx = stream.subscribe();
    assert_eq!(stream.subscriber_count(), 1);

    // Create a simple edit
    let edit = SyntaxEdit::insert(0, 0, 0, 3, 0, 3);

    // Notify edit
    stream.notify_edit(&mut syntax, id, "fn main() {}", &edit, 0, 0);

    // Should receive an update
    let update = rx.try_recv().expect("Should receive update");
    assert_eq!(update.buffer_id, 1);
    assert!(!update.full_refresh);
    assert_eq!(update.layer, "syntax");
    assert_eq!(update.priority, 0);
}

#[test]
fn test_notify_edit_no_driver() {
    let mut syntax = SyntaxSessionState::new();
    let mut stream = SyntaxStreamState::new();
    let id = buffer_id(1);

    // No driver set
    let edit = SyntaxEdit::insert(0, 0, 0, 3, 0, 3);

    // Should not panic
    stream.notify_edit(&mut syntax, id, "hello", &edit, 0, 0);
}

#[test]
fn test_send_full_refresh() {
    let mut syntax = SyntaxSessionState::new();
    let mut stream = SyntaxStreamState::new();
    let id = buffer_id(1);

    // Set up a driver
    syntax.set(id, Box::new(TestDriver::new("rust")));
    syntax.get_mut(id).unwrap().parse("fn main() {}");

    // Subscribe
    let mut rx = stream.subscribe();

    // Send full refresh
    stream.send_full_refresh(&syntax, id, 10);

    // Should receive a full refresh update
    let update = rx.try_recv().expect("Should receive update");
    assert_eq!(update.buffer_id, 1);
    assert!(update.full_refresh);
    assert_eq!(update.end_line, 9); // total_lines - 1
    assert_eq!(update.layer, "syntax");
    assert_eq!(update.priority, 0);
}

#[test]
fn test_notify_edit_no_subscribers_skips_extraction() {
    let mut syntax = SyntaxSessionState::new();
    let mut stream = SyntaxStreamState::new();
    let id = buffer_id(1);

    syntax.set(id, Box::new(TestDriver::new("rust")));

    // No subscribers: notify_edit should return early after driver.update()
    let edit = SyntaxEdit {
        start_byte: 0,
        old_end_byte: 0,
        new_end_byte: 5,
        start_row: 0,
        start_col: 0,
        old_end_row: 0,
        old_end_col: 0,
        new_end_row: 0,
        new_end_col: 5,
    };
    stream.notify_edit(&mut syntax, id, "hello", &edit, 0, 0);
    // No panic, no subscribers to receive
}

#[test]
fn test_send_full_refresh_no_driver_returns_early() {
    let syntax = SyntaxSessionState::new();
    let mut stream = SyntaxStreamState::new();
    let _rx = stream.subscribe(); // Has subscriber but no driver
    let unknown = buffer_id(999);

    // Should return early (no driver)
    stream.send_full_refresh(&syntax, unknown, 10);
    // No panic
}

#[test]
fn test_send_full_refresh_no_subscribers_returns_early() {
    let mut syntax = SyntaxSessionState::new();
    let stream = SyntaxStreamState::new();
    let id = buffer_id(1);
    syntax.set(id, Box::new(TestDriver::new("rust")));

    // Has driver but no subscribers: returns early
    // Note: we need &mut self for send_full_refresh, use a mutable binding
    let mut stream = stream;
    stream.send_full_refresh(&syntax, id, 10);
    // No panic
}

// =============================================================================
// text_event_to_syntax_edit tests (#740 Plan 09)
// =============================================================================

use reovim_driver_syntax::{compute_end_position, text_event_to_syntax_edit};

#[test]
fn test_text_event_to_syntax_edit_insert_single_line() {
    use reovim_driver_codec::{TextBufferModified, TextEdit, TextPosition};

    let event = TextBufferModified {
        buffer_id: buffer_id(1),
        edit: TextEdit::insert(TextPosition::new(0, 5), "hello".to_string()),
        start_byte: 5,
        old_end_byte: 5,
        new_end_byte: 10,
    };

    let edit = text_event_to_syntax_edit(&event);
    assert_eq!(edit.start_byte, 5);
    assert_eq!(edit.old_end_byte, 5); // Insert: old_end = start
    assert_eq!(edit.new_end_byte, 10);
    assert_eq!(edit.start_row, 0);
    assert_eq!(edit.start_col, 5);
    assert_eq!(edit.new_end_row, 0);
    assert_eq!(edit.new_end_col, 10); // 5 + "hello".len()
}

#[test]
fn test_text_event_to_syntax_edit_insert_multiline() {
    use reovim_driver_codec::{TextBufferModified, TextEdit, TextPosition};

    let event = TextBufferModified {
        buffer_id: buffer_id(1),
        edit: TextEdit::insert(TextPosition::new(1, 3), "ab\ncd".to_string()),
        start_byte: 10,
        old_end_byte: 10,
        new_end_byte: 15,
    };

    let edit = text_event_to_syntax_edit(&event);
    assert_eq!(edit.start_byte, 10);
    assert_eq!(edit.old_end_byte, 10);
    assert_eq!(edit.new_end_byte, 15);
    assert_eq!(edit.start_row, 1);
    assert_eq!(edit.start_col, 3);
    assert_eq!(edit.new_end_row, 2); // 1 + 1 newline
    assert_eq!(edit.new_end_col, 2); // len("cd")
}

#[test]
fn test_text_event_to_syntax_edit_delete_single_line() {
    use reovim_driver_codec::{TextBufferModified, TextEdit, TextPosition};

    let event = TextBufferModified {
        buffer_id: buffer_id(1),
        edit: TextEdit::delete(TextPosition::new(0, 0), "hello".to_string()),
        start_byte: 0,
        old_end_byte: 5,
        new_end_byte: 0,
    };

    let edit = text_event_to_syntax_edit(&event);
    assert_eq!(edit.start_byte, 0);
    assert_eq!(edit.old_end_byte, 5);
    assert_eq!(edit.new_end_byte, 0); // Delete: new_end = start
    assert_eq!(edit.start_row, 0);
    assert_eq!(edit.start_col, 0);
    assert_eq!(edit.old_end_row, 0);
    assert_eq!(edit.old_end_col, 5);
}

#[test]
fn test_text_event_to_syntax_edit_delete_multiline() {
    use reovim_driver_codec::{TextBufferModified, TextEdit, TextPosition};

    let event = TextBufferModified {
        buffer_id: buffer_id(1),
        edit: TextEdit::delete(TextPosition::new(0, 0), "abc\ndef\n".to_string()),
        start_byte: 0,
        old_end_byte: 8,
        new_end_byte: 0,
    };

    let edit = text_event_to_syntax_edit(&event);
    assert_eq!(edit.start_byte, 0);
    assert_eq!(edit.old_end_byte, 8);
    assert_eq!(edit.new_end_byte, 0);
    assert_eq!(edit.old_end_row, 2); // 0 + 2 newlines
    assert_eq!(edit.old_end_col, 0); // After final newline
}

#[test]
fn test_text_event_to_syntax_edit_zero_insert() {
    use reovim_driver_codec::{TextBufferModified, TextEdit, TextPosition};

    let event = TextBufferModified {
        buffer_id: buffer_id(1),
        edit: TextEdit::insert(TextPosition::new(5, 10), String::new()),
        start_byte: 50,
        old_end_byte: 50,
        new_end_byte: 50,
    };

    let edit = text_event_to_syntax_edit(&event);
    assert_eq!(edit.start_byte, 50);
    assert_eq!(edit.old_end_byte, 50);
    assert_eq!(edit.new_end_byte, 50);
    assert_eq!(edit.start_row, 5);
    assert_eq!(edit.start_col, 10);
    assert_eq!(edit.new_end_row, 5);
    assert_eq!(edit.new_end_col, 10); // Empty insert: end = start
}

#[test]
fn test_text_event_to_syntax_edit_large_offsets() {
    use reovim_driver_codec::{TextBufferModified, TextEdit, TextPosition};

    let event = TextBufferModified {
        buffer_id: buffer_id(1),
        edit: TextEdit::insert(TextPosition::new(1000, 500), "test".to_string()),
        start_byte: 50000,
        old_end_byte: 50000,
        new_end_byte: 50004,
    };

    let edit = text_event_to_syntax_edit(&event);
    assert_eq!(edit.start_byte, 50000);
    assert_eq!(edit.old_end_byte, 50000);
    assert_eq!(edit.new_end_byte, 50004);
    assert_eq!(edit.start_row, 1000);
    assert_eq!(edit.start_col, 500);
    assert_eq!(edit.new_end_row, 1000);
    assert_eq!(edit.new_end_col, 504); // 500 + "test".len()
}

// =============================================================================
// compute_end_position tests (#655)
// =============================================================================

#[test]
fn test_compute_end_position_single_line() {
    assert_eq!(compute_end_position(0, 0, "hello"), (0, 5));
    assert_eq!(compute_end_position(2, 3, "abc"), (2, 6));
}

#[test]
fn test_compute_end_position_multi_line() {
    assert_eq!(compute_end_position(0, 0, "ab\ncd"), (1, 2));
    assert_eq!(compute_end_position(5, 10, "x\ny\nz"), (7, 1));
}

#[test]
fn test_compute_end_position_empty() {
    assert_eq!(compute_end_position(3, 7, ""), (3, 7));
}

#[test]
fn test_compute_end_position_trailing_newline() {
    assert_eq!(compute_end_position(0, 0, "abc\n"), (1, 0));
}

#[test]
fn test_debug_impl() {
    let mut state = SyntaxStreamState::new();
    let _rx = state.subscribe();

    let debug = format!("{state:?}");
    assert!(debug.contains("SyntaxStreamState"));
    assert!(debug.contains("subscriber_count"));
}

// ========================================================================
// SyntaxSessionState re-export sanity test
// ========================================================================

// ========================================================================
// build_token_update tests
// ========================================================================

#[test]
fn test_build_token_update_with_driver() {
    let mut syntax = SyntaxSessionState::new();
    let id = buffer_id(1);

    syntax.set(id, Box::new(TestDriver::new("rust")));
    syntax.get_mut(id).unwrap().parse("fn main() {}");

    let update = build_token_update(&syntax, id, 10, true);
    assert!(update.is_some());

    let update = update.unwrap();
    assert_eq!(update.buffer_id, 1);
    assert!(update.full_refresh);
    assert_eq!(update.start_line, 0);
    assert_eq!(update.end_line, 9);
    assert!(!update.tokens.is_empty());
    assert_eq!(update.layer, "syntax");
    assert_eq!(update.priority, 0);
}

#[test]
fn test_build_token_update_no_driver() {
    let syntax = SyntaxSessionState::new();
    let id = buffer_id(1);

    let update = build_token_update(&syntax, id, 10, true);
    assert!(update.is_none());
}

#[test]
fn test_build_token_update_incremental() {
    let mut syntax = SyntaxSessionState::new();
    let id = buffer_id(1);

    syntax.set(id, Box::new(TestDriver::new("rust")));
    syntax.get_mut(id).unwrap().parse("fn main() {}");

    let update = build_token_update(&syntax, id, 5, false).unwrap();
    assert!(!update.full_refresh);
    assert_eq!(update.end_line, 4);
}

#[test]
fn test_syntax_session_state_reexport() {
    // Verify re-export works: SyntaxSessionState accessible from this module
    let mut state = SyntaxSessionState::new();
    let id = buffer_id(1);

    state.set_factory(Arc::new(TestFactory));
    assert!(state.ensure_driver(id, "rust", "fn main() {}"));
    assert!(state.get(id).is_some());
}
