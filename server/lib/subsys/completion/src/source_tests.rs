use std::sync::Arc;

use crate::CompletionKind;

use super::*;

struct TestSource {
    source_id: &'static str,
    prio: u16,
    available: bool,
}

impl CompletionSource for TestSource {
    fn id(&self) -> &'static str {
        self.source_id
    }

    fn priority(&self) -> u16 {
        self.prio
    }

    fn is_available(&self, _ctx: &CompletionContext) -> bool {
        self.available
    }

    fn complete(&self, ctx: &CompletionContext) -> Vec<CompletionItem> {
        if ctx.prefix.is_empty() {
            return vec![];
        }
        vec![CompletionItem {
            label: format!("{}_item", self.source_id),
            insert_text: format!("{}_item", self.source_id),
            kind: CompletionKind::Text,
            detail: None,
            documentation: None,
            source_id: self.source_id,
            is_snippet: false,
            sort_priority: self.prio,
        }]
    }
}

fn make_context(prefix: &str) -> CompletionContext {
    CompletionContext {
        content: String::new(),
        cursor_offset: 0,
        line: 0,
        col: 0,
        prefix: prefix.to_string(),
        buffer_id: 0,
        file_path: None,
        language_id: None,
    }
}

#[test]
fn source_id() {
    let source = TestSource {
        source_id: "test",
        prio: 100,
        available: true,
    };
    assert_eq!(source.id(), "test");
}

#[test]
fn source_priority() {
    let source = TestSource {
        source_id: "test",
        prio: 200,
        available: true,
    };
    assert_eq!(source.priority(), 200);
}

#[test]
fn source_available() {
    let available = TestSource {
        source_id: "a",
        prio: 100,
        available: true,
    };
    let unavailable = TestSource {
        source_id: "b",
        prio: 100,
        available: false,
    };
    let ctx = make_context("test");
    assert!(available.is_available(&ctx));
    assert!(!unavailable.is_available(&ctx));
}

#[test]
fn source_complete_with_prefix() {
    let source = TestSource {
        source_id: "buf",
        prio: 100,
        available: true,
    };
    let ctx = make_context("fn");
    let items = source.complete(&ctx);
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].label, "buf_item");
    assert_eq!(items[0].source_id, "buf");
    assert_eq!(items[0].sort_priority, 100);
}

#[test]
fn source_complete_empty_prefix() {
    let source = TestSource {
        source_id: "buf",
        prio: 100,
        available: true,
    };
    let ctx = make_context("");
    let items = source.complete(&ctx);
    assert!(items.is_empty());
}

#[test]
fn trait_object_safety_arc() {
    let source: Arc<dyn CompletionSource> = Arc::new(TestSource {
        source_id: "arc",
        prio: 50,
        available: true,
    });
    assert_eq!(source.id(), "arc");
    assert_eq!(source.priority(), 50);
}

#[test]
fn trait_object_safety_box() {
    let source: Box<dyn CompletionSource> = Box::new(TestSource {
        source_id: "boxed",
        prio: 75,
        available: false,
    });
    assert_eq!(source.id(), "boxed");
    assert!(!source.is_available(&make_context("x")));
}
