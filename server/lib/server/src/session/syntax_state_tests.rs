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
