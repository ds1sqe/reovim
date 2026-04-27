#![allow(clippy::literal_string_with_formatting_args)]

use {
    super::*,
    crate::{parser, variables::VariableContext},
};

/// Helper: parse and expand a snippet at a given position.
fn expand_at(input: &str, pos: Position) -> (String, ActiveSnippet) {
    let body = parser::parse(input).unwrap();
    ActiveSnippet::expand(&body, pos, &VariableContext::empty())
}

// =========================================================================
// Basic expansion
// =========================================================================

#[test]
fn test_expand_plain_text() {
    let (text, snippet) = expand_at("hello world", Position::origin());
    assert_eq!(text, "hello world");
    assert!(snippet.tab_stops.is_empty());
    assert!(snippet.is_done());
}

#[test]
fn test_expand_single_tabstop() {
    let (text, snippet) = expand_at("fn $1()", Position::origin());
    assert_eq!(text, "fn ()");
    assert_eq!(snippet.tab_stop_count(), 1);
    let ts = snippet.current().unwrap();
    assert_eq!(ts.id, 1);
    assert_eq!(ts.start, Position::new(0, 3));
    assert_eq!(ts.end, Position::new(0, 3));
    assert!(ts.placeholder.is_empty());
}

#[test]
fn test_expand_multiple_tabstops() {
    let (text, snippet) = expand_at("$1 $2 $0", Position::origin());
    assert_eq!(text, "  ");
    assert_eq!(snippet.tab_stop_count(), 3);
    // $1, $2, then $0
    assert_eq!(snippet.tab_stops()[0].id, 1);
    assert_eq!(snippet.tab_stops()[1].id, 2);
    assert_eq!(snippet.tab_stops()[2].id, 0);
}

#[test]
fn test_expand_with_offset() {
    let (text, snippet) = expand_at("$1", Position::new(5, 10));
    assert_eq!(text, "");
    let ts = snippet.current().unwrap();
    assert_eq!(ts.start, Position::new(5, 10));
}

#[test]
fn test_expand_placeholder() {
    let (text, snippet) = expand_at("${1:name}", Position::origin());
    assert_eq!(text, "name");
    assert_eq!(snippet.tab_stop_count(), 1);
    let ts = snippet.current().unwrap();
    assert_eq!(ts.id, 1);
    assert_eq!(ts.start, Position::origin());
    assert_eq!(ts.end, Position::new(0, 4));
    assert_eq!(ts.placeholder, "name");
}

#[test]
fn test_expand_placeholder_with_text() {
    let (text, snippet) = expand_at("fn ${1:name}()", Position::origin());
    assert_eq!(text, "fn name()");
    let ts = snippet.current().unwrap();
    assert_eq!(ts.id, 1);
    assert_eq!(ts.start, Position::new(0, 3));
    assert_eq!(ts.end, Position::new(0, 7));
}

#[test]
fn test_expand_multiline() {
    let (text, snippet) = expand_at("fn $1() {\n\t$0\n}", Position::origin());
    assert_eq!(text, "fn () {\n\t\n}");
    assert_eq!(snippet.snippet_end, Position::new(2, 1));
}

#[test]
fn test_expand_nested_placeholder() {
    let (text, snippet) = expand_at("${1:outer ${2:inner}}", Position::origin());
    assert_eq!(text, "outer inner");
    assert_eq!(snippet.tab_stop_count(), 2);
    // $1 comes first (sorted by id), then $2
    assert_eq!(snippet.tab_stops()[0].id, 1);
    assert_eq!(snippet.tab_stops()[1].id, 2);
}

// =========================================================================
// Navigation: next / prev
// =========================================================================

#[test]
fn test_next_advances() {
    let (_, mut snippet) = expand_at("$1 $2 $0", Position::origin());
    assert_eq!(snippet.current().unwrap().id, 1);
    assert_eq!(snippet.next().unwrap().id, 2);
    assert_eq!(snippet.next().unwrap().id, 0);
    assert!(snippet.next().is_none());
}

#[test]
fn test_prev_goes_back() {
    let (_, mut snippet) = expand_at("$1 $2 $0", Position::origin());
    snippet.next(); // → $2
    snippet.next(); // → $0
    assert_eq!(snippet.prev().unwrap().id, 2);
    assert_eq!(snippet.prev().unwrap().id, 1);
    assert!(snippet.prev().is_none()); // already at first
}

#[test]
fn test_prev_at_first_returns_none() {
    let (_, mut snippet) = expand_at("$1 $2", Position::origin());
    assert!(snippet.prev().is_none());
}

#[test]
fn test_next_exhausted_is_done() {
    let (_, mut snippet) = expand_at("$1", Position::origin());
    assert!(!snippet.is_done());
    snippet.next(); // past end
    assert!(snippet.is_done());
}

#[test]
fn test_no_tabstops_is_done() {
    let (_, snippet) = expand_at("plain text", Position::origin());
    assert!(snippet.is_done());
}

// =========================================================================
// Position tracking: update_positions
// =========================================================================

#[test]
fn test_update_positions_insert_before() {
    let (_, mut snippet) = expand_at("$1", Position::new(0, 5));
    // Insert 3 chars before the tab stop on same line
    let edit = Edit::insert(Position::new(0, 2), "abc");
    snippet.update_positions(&edit);
    assert_eq!(snippet.current().unwrap().start, Position::new(0, 8));
}

#[test]
fn test_update_positions_insert_after() {
    let (_, mut snippet) = expand_at("$1", Position::new(0, 5));
    // Insert after the tab stop: no change
    let edit = Edit::insert(Position::new(0, 10), "xyz");
    snippet.update_positions(&edit);
    assert_eq!(snippet.current().unwrap().start, Position::new(0, 5));
}

#[test]
fn test_update_positions_insert_newline() {
    let (_, mut snippet) = expand_at("$1", Position::new(0, 5));
    // Insert newline before
    let edit = Edit::insert(Position::new(0, 2), "a\nb");
    snippet.update_positions(&edit);
    // Position shifts down by 1 line
    assert_eq!(snippet.current().unwrap().start, Position::new(1, 4));
}

#[test]
fn test_update_positions_delete() {
    let (_, mut snippet) = expand_at("$1", Position::new(0, 5));
    // Delete 2 chars before the tab stop
    let edit = Edit::delete(Position::new(0, 2), "ab");
    snippet.update_positions(&edit);
    assert_eq!(snippet.current().unwrap().start, Position::new(0, 3));
}

#[test]
fn test_update_positions_updates_snippet_bounds() {
    let (_, mut snippet) = expand_at("hello $1 world", Position::origin());
    let orig_end = snippet.snippet_end;
    let edit = Edit::insert(Position::origin(), "prefix ");
    snippet.update_positions(&edit);
    // Snippet start and end should both shift
    assert!(snippet.snippet_start.column > 0);
    assert!(snippet.snippet_end.column > orig_end.column);
}

// =========================================================================
// Validity: is_valid
// =========================================================================

#[test]
fn test_is_valid_at_start() {
    let (_, snippet) = expand_at("hello $1", Position::origin());
    assert!(snippet.is_valid(Position::origin()));
}

#[test]
fn test_is_valid_at_end() {
    let (_, snippet) = expand_at("hello", Position::origin());
    assert!(snippet.is_valid(snippet.snippet_end));
}

#[test]
fn test_is_valid_in_middle() {
    let (_, snippet) = expand_at("hello world", Position::origin());
    assert!(snippet.is_valid(Position::new(0, 5)));
}

#[test]
fn test_is_valid_before_start() {
    let (_, snippet) = expand_at("hello", Position::new(0, 5));
    assert!(!snippet.is_valid(Position::new(0, 3)));
}

#[test]
fn test_is_valid_after_end() {
    let (_, snippet) = expand_at("hi", Position::origin());
    assert!(!snippet.is_valid(Position::new(0, 100)));
}

#[test]
fn test_is_valid_different_line_before() {
    let (_, snippet) = expand_at("hello", Position::new(5, 0));
    assert!(!snippet.is_valid(Position::new(4, 0)));
}

#[test]
fn test_is_valid_different_line_after() {
    let (_, snippet) = expand_at("hello", Position::new(0, 0));
    assert!(!snippet.is_valid(Position::new(1, 0)));
}

#[test]
fn test_is_valid_multiline() {
    let (_, snippet) = expand_at("line1\nline2\nline3", Position::origin());
    assert!(snippet.is_valid(Position::new(1, 3)));
    assert!(!snippet.is_valid(Position::new(3, 0)));
}

// =========================================================================
// Accessors
// =========================================================================

#[test]
fn test_tab_stops_accessor() {
    let (_, snippet) = expand_at("$1 $2", Position::origin());
    assert_eq!(snippet.tab_stops().len(), 2);
}

#[test]
fn test_snippet_start_accessor() {
    let pos = Position::new(3, 7);
    let (_, snippet) = expand_at("hello", pos);
    assert_eq!(snippet.snippet_start(), pos);
}

#[test]
fn test_snippet_end_accessor() {
    let (_, snippet) = expand_at("hi", Position::origin());
    assert_eq!(snippet.snippet_end(), Position::new(0, 2));
}

// =========================================================================
// TabStopRange
// =========================================================================

#[test]
fn test_tabstop_range_clone() {
    let range = TabStopRange {
        id: 1,
        start: Position::origin(),
        end: Position::new(0, 5),
        placeholder: "hello".to_string(),
    };
    let cloned = range.clone();
    assert_eq!(range, cloned);
}

#[test]
fn test_tabstop_range_debug() {
    let range = TabStopRange {
        id: 0,
        start: Position::origin(),
        end: Position::origin(),
        placeholder: String::new(),
    };
    let debug = format!("{range:?}");
    assert!(debug.contains("TabStopRange"));
}

// =========================================================================
// ActiveSnippet clone/debug
// =========================================================================

#[test]
fn test_active_snippet_clone() {
    let (_, snippet) = expand_at("$1", Position::origin());
    let cloned = snippet.clone();
    assert_eq!(cloned.tab_stop_count(), snippet.tab_stop_count());
}

#[test]
fn test_active_snippet_debug() {
    let (_, snippet) = expand_at("$1", Position::origin());
    let debug = format!("{snippet:?}");
    assert!(debug.contains("ActiveSnippet"));
}

// =========================================================================
// Tab stop ordering
// =========================================================================

#[test]
fn test_tabstop_ordering_with_zero() {
    let (_, snippet) = expand_at("$2 $1 $0", Position::origin());
    // Should be sorted: $1, $2, $0
    assert_eq!(snippet.tab_stops()[0].id, 1);
    assert_eq!(snippet.tab_stops()[1].id, 2);
    assert_eq!(snippet.tab_stops()[2].id, 0);
}

#[test]
fn test_tabstop_ordering_no_zero() {
    let (_, snippet) = expand_at("$3 $1 $2", Position::origin());
    assert_eq!(snippet.tab_stops()[0].id, 1);
    assert_eq!(snippet.tab_stops()[1].id, 2);
    assert_eq!(snippet.tab_stops()[2].id, 3);
}

#[test]
fn test_tabstop_ordering_multi_digit() {
    let (_, snippet) = expand_at("${10} ${1} ${5}", Position::origin());
    assert_eq!(snippet.tab_stops()[0].id, 1);
    assert_eq!(snippet.tab_stops()[1].id, 5);
    assert_eq!(snippet.tab_stops()[2].id, 10);
}

// =========================================================================
// Realistic snippet
// =========================================================================

#[test]
fn test_function_snippet() {
    let (text, snippet) =
        expand_at("fn ${1:name}(${2:params}) -> ${3:Type} {\n\t$0\n}", Position::origin());
    assert_eq!(text, "fn name(params) -> Type {\n\t\n}");
    assert_eq!(snippet.tab_stop_count(), 4);
    // $1, $2, $3, $0
    assert_eq!(snippet.tab_stops()[0].id, 1);
    assert_eq!(snippet.tab_stops()[1].id, 2);
    assert_eq!(snippet.tab_stops()[2].id, 3);
    assert_eq!(snippet.tab_stops()[3].id, 0);
}

#[test]
fn test_function_snippet_positions() {
    let (_, snippet) =
        expand_at("fn ${1:name}(${2:params}) -> ${3:Type} {\n\t$0\n}", Position::origin());
    // $1: "name" at col 3..7
    assert_eq!(snippet.tab_stops()[0].start, Position::new(0, 3));
    assert_eq!(snippet.tab_stops()[0].end, Position::new(0, 7));
    // $2: "params" at col 8..14
    assert_eq!(snippet.tab_stops()[1].start, Position::new(0, 8));
    assert_eq!(snippet.tab_stops()[1].end, Position::new(0, 14));
    // $3: "Type" at col 19..23
    assert_eq!(snippet.tab_stops()[2].start, Position::new(0, 19));
    assert_eq!(snippet.tab_stops()[2].end, Position::new(0, 23));
    // $0: at line 1, col 1 (after \t)
    assert_eq!(snippet.tab_stops()[3].start, Position::new(1, 1));
}

// =========================================================================
// Edge: expand with empty body
// =========================================================================

#[test]
fn test_expand_empty_body() {
    let body = SnippetBody::new(vec![]);
    let (text, snippet) =
        ActiveSnippet::expand(&body, Position::origin(), &VariableContext::empty());
    assert!(text.is_empty());
    assert!(snippet.is_done());
    assert_eq!(snippet.snippet_start(), Position::origin());
    assert_eq!(snippet.snippet_end(), Position::origin());
}

// =========================================================================
// Unimplemented variants (Variable, Choice) are no-ops
// =========================================================================

#[test]
fn test_expand_variable_resolved() {
    use crate::ast::SnippetElement;
    let body = SnippetBody::new(vec![
        SnippetElement::Text("before ".to_string()),
        SnippetElement::Variable {
            name: "TM_FILENAME".to_string(),
            default: None,
            transform: None,
        },
        SnippetElement::Text(" after".to_string()),
    ]);
    let ctx = VariableContext {
        file_path: Some("/src/main.rs".to_string()),
        ..VariableContext::empty()
    };
    let (text, snippet) = ActiveSnippet::expand(&body, Position::origin(), &ctx);
    assert_eq!(text, "before main.rs after");
    assert!(snippet.tab_stops.is_empty());
}

#[test]
fn test_expand_variable_unresolved_no_default() {
    use crate::ast::SnippetElement;
    let body = SnippetBody::new(vec![
        SnippetElement::Text("before ".to_string()),
        SnippetElement::Variable {
            name: "TM_FILENAME".to_string(),
            default: None,
            transform: None,
        },
        SnippetElement::Text(" after".to_string()),
    ]);
    let (text, snippet) =
        ActiveSnippet::expand(&body, Position::origin(), &VariableContext::empty());
    // No file_path, no default → variable emits nothing
    assert_eq!(text, "before  after");
    assert!(snippet.tab_stops.is_empty());
}

#[test]
fn test_expand_variable_unresolved_with_default() {
    use crate::ast::SnippetElement;
    let body = SnippetBody::new(vec![SnippetElement::Variable {
        name: "TM_FILENAME".to_string(),
        default: Some(vec![SnippetElement::Text("untitled".to_string())]),
        transform: None,
    }]);
    let (text, _) = ActiveSnippet::expand(&body, Position::origin(), &VariableContext::empty());
    assert_eq!(text, "untitled");
}

#[test]
fn test_expand_choice_first_default() {
    use crate::ast::SnippetElement;
    let body = SnippetBody::new(vec![
        SnippetElement::Choice {
            id: 1,
            choices: vec!["a".to_string(), "b".to_string()],
        },
        SnippetElement::Text("end".to_string()),
    ]);
    let (text, snippet) =
        ActiveSnippet::expand(&body, Position::origin(), &VariableContext::empty());
    // First choice "a" is inserted as default text
    assert_eq!(text, "aend");
    assert_eq!(snippet.tab_stops.len(), 1);
    let ts = &snippet.tab_stops[0];
    assert_eq!(ts.id, 1);
    assert_eq!(ts.start, Position::origin());
    assert_eq!(ts.end, Position::new(0, 1));
    assert_eq!(ts.placeholder, "a");
}

#[test]
fn test_expand_choice_empty() {
    use crate::ast::SnippetElement;
    let body = SnippetBody::new(vec![
        SnippetElement::Choice {
            id: 1,
            choices: vec![],
        },
        SnippetElement::Text("end".to_string()),
    ]);
    let (text, snippet) =
        ActiveSnippet::expand(&body, Position::origin(), &VariableContext::empty());
    // Empty choices — tab stop with no text
    assert_eq!(text, "end");
    assert_eq!(snippet.tab_stops.len(), 1);
    let ts = &snippet.tab_stops[0];
    assert_eq!(ts.start, Position::origin());
    assert_eq!(ts.end, Position::origin()); // start == end
}

// =========================================================================
// next() on empty snippet
// =========================================================================

#[test]
fn test_next_on_empty_returns_none() {
    let (_, mut snippet) = expand_at("plain text", Position::origin());
    assert!(snippet.tab_stops.is_empty());
    // Calling next() on empty snippet should be a no-op
    assert!(snippet.next().is_none());
    assert!(snippet.is_done());
}

// =========================================================================
// reconcile_typing
// =========================================================================

#[test]
fn test_reconcile_typing_same_line() {
    // "fn $1()" — $1 at col 3, $0-like paren at col 3 (after)
    let (_, mut snippet) = expand_at("$1 $2", Position::origin());
    // $1 at col 0, $2 at col 1
    let ts2_before = snippet.tab_stops()[1].start;
    assert_eq!(ts2_before, Position::new(0, 1));

    // Simulate user typing 5 chars at $1 (cursor moved from col 0 to col 5)
    snippet.reconcile_typing(Position::new(0, 5));

    // $2 should shift right by 5
    let ts2_after = snippet.tab_stops()[1].start;
    assert_eq!(ts2_after, Position::new(0, 6));
}

#[test]
fn test_reconcile_typing_cursor_at_start_no_change() {
    let (_, mut snippet) = expand_at("$1 $2", Position::origin());
    let ts2_before = snippet.tab_stops()[1].start;

    // Cursor still at $1 start (no typing)
    snippet.reconcile_typing(Position::new(0, 0));

    assert_eq!(snippet.tab_stops()[1].start, ts2_before);
}

#[test]
fn test_reconcile_typing_cursor_before_start_no_change() {
    // $1 at col 3
    let (_, mut snippet) = expand_at("abc$1 $2", Position::origin());
    let ts2_before = snippet.tab_stops()[1].start;

    // Cursor moved left of $1 start (unusual, e.g. backspace past start)
    snippet.reconcile_typing(Position::new(0, 2));

    assert_eq!(snippet.tab_stops()[1].start, ts2_before);
}

#[test]
fn test_reconcile_typing_multiline() {
    // $1 at col 0, $2 at col 1
    let (_, mut snippet) = expand_at("$1 $2", Position::origin());
    let ts2_before = snippet.tab_stops()[1].start;
    assert_eq!(ts2_before, Position::new(0, 1));

    // Simulate user typing text with a newline (cursor now on line 1, col 3)
    snippet.reconcile_typing(Position::new(1, 3));

    // $2 should be pushed down to line 1
    let ts2_after = snippet.tab_stops()[1].start;
    assert_eq!(ts2_after.line, 1);
}

#[test]
fn test_reconcile_typing_cursor_above_start_no_change() {
    // $1 at line 1
    let (_, mut snippet) = expand_at("\n$1 $2", Position::origin());
    let ts2_before = snippet.tab_stops()[1].start;

    // Cursor above $1 start line (unusual)
    snippet.reconcile_typing(Position::new(0, 0));

    assert_eq!(snippet.tab_stops()[1].start, ts2_before);
}

#[test]
fn test_reconcile_typing_no_tabstops() {
    let (_, mut snippet) = expand_at("plain text", Position::origin());
    // Should not panic when no tab stops exist
    snippet.reconcile_typing(Position::new(0, 5));
}

#[test]
fn test_reconcile_typing_past_last_stop() {
    let (_, mut snippet) = expand_at("$1", Position::origin());
    snippet.next(); // exhaust all stops
    assert!(snippet.is_done());
    // Should not panic when current_index is past end
    snippet.reconcile_typing(Position::new(0, 5));
}

// =========================================================================
// Phase 2: current_index, mirror_indices, update_tab_stop
// =========================================================================

#[test]
fn test_current_index_initial() {
    let (_, snippet) = expand_at("$1 $2 $0", Position::origin());
    assert_eq!(snippet.current_index(), 0);
}

#[test]
fn test_current_index_after_next() {
    let (_, mut snippet) = expand_at("$1 $2 $0", Position::origin());
    snippet.next();
    assert_eq!(snippet.current_index(), 1);
    snippet.next();
    assert_eq!(snippet.current_index(), 2);
}

#[test]
fn test_current_index_after_prev() {
    let (_, mut snippet) = expand_at("$1 $2 $0", Position::origin());
    snippet.next();
    snippet.next();
    snippet.prev();
    assert_eq!(snippet.current_index(), 1);
}

#[test]
fn test_mirror_indices_no_duplicates() {
    let (_, snippet) = expand_at("$1 $2 $0", Position::origin());
    // Each ID is unique — no mirrors
    let mirrors = snippet.mirror_indices(1, 0);
    assert!(mirrors.is_empty());
}

#[test]
fn test_mirror_indices_with_duplicates() {
    // Build a snippet with duplicate tab stop IDs manually
    let body = crate::ast::SnippetBody::new(vec![
        crate::ast::SnippetElement::Placeholder {
            id: 1,
            body: vec![crate::ast::SnippetElement::Text("name".to_string())],
        },
        crate::ast::SnippetElement::Text(" = ".to_string()),
        crate::ast::SnippetElement::TabStop {
            id: 1,
            transform: None,
        },
        crate::ast::SnippetElement::Text(";".to_string()),
    ]);
    let (text, snippet) =
        ActiveSnippet::expand(&body, Position::origin(), &VariableContext::empty());
    assert_eq!(text, "name = ;");

    // Tab stop $1 appears twice — indices 0 and 1 (sorted by id)
    let tab_stops = snippet.tab_stops();
    assert_eq!(tab_stops.len(), 2);
    assert_eq!(tab_stops[0].id, 1);
    assert_eq!(tab_stops[1].id, 1);

    // Mirror of index 0 should return index 1
    let mirrors = snippet.mirror_indices(1, 0);
    assert_eq!(mirrors, vec![1]);

    // Mirror of index 1 should return index 0
    let mirrors = snippet.mirror_indices(1, 1);
    assert_eq!(mirrors, vec![0]);
}

#[test]
fn test_mirror_indices_excludes_self() {
    let body = crate::ast::SnippetBody::new(vec![
        crate::ast::SnippetElement::TabStop {
            id: 1,
            transform: None,
        },
        crate::ast::SnippetElement::Text(" ".to_string()),
        crate::ast::SnippetElement::TabStop {
            id: 1,
            transform: None,
        },
        crate::ast::SnippetElement::Text(" ".to_string()),
        crate::ast::SnippetElement::TabStop {
            id: 1,
            transform: None,
        },
    ]);
    let (_, snippet) = ActiveSnippet::expand(&body, Position::origin(), &VariableContext::empty());
    assert_eq!(snippet.tab_stops().len(), 3);

    // Excluding index 1, mirrors are [0, 2]
    let mirrors = snippet.mirror_indices(1, 1);
    assert_eq!(mirrors, vec![0, 2]);
}

#[test]
fn test_update_tab_stop() {
    let (_, mut snippet) = expand_at("${1:name} $2", Position::origin());
    let ts = &snippet.tab_stops()[0];
    assert_eq!(ts.placeholder, "name");
    assert_eq!(ts.start, Position::origin());
    assert_eq!(ts.end, Position::new(0, 4));

    // Update the tab stop
    snippet.update_tab_stop(0, Position::new(0, 0), Position::new(0, 5), "hello".to_string());

    let ts = &snippet.tab_stops()[0];
    assert_eq!(ts.placeholder, "hello");
    assert_eq!(ts.end, Position::new(0, 5));
}

#[test]
fn test_update_tab_stop_out_of_bounds() {
    let (_, mut snippet) = expand_at("$1", Position::origin());
    // Should not panic on out-of-bounds index
    snippet.update_tab_stop(99, Position::origin(), Position::new(0, 5), "test".to_string());
}

// =========================================================================
// Phase 5: Variable with transform
// =========================================================================

#[test]
fn test_expand_variable_with_transform() {
    use crate::ast::{FormatItem, SnippetElement, Transform};
    let body = SnippetBody::new(vec![SnippetElement::Variable {
        name: "TM_FILENAME".to_string(),
        default: None,
        transform: Some(Transform {
            regex: "(.*)\\..*".to_string(),
            replacement: vec![FormatItem::Capture(1)],
            options: String::new(),
        }),
    }]);
    let ctx = VariableContext {
        file_path: Some("/src/main.rs".to_string()),
        ..VariableContext::empty()
    };
    let (text, _) = ActiveSnippet::expand(&body, Position::origin(), &ctx);
    // Transform strips file extension
    assert_eq!(text, "main");
}

#[test]
fn test_expand_variable_with_transform_no_match() {
    use crate::ast::{FormatItem, SnippetElement, Transform};
    let body = SnippetBody::new(vec![SnippetElement::Variable {
        name: "TM_FILENAME".to_string(),
        default: None,
        transform: Some(Transform {
            regex: "xyz".to_string(),
            replacement: vec![FormatItem::Text("replaced".to_string())],
            options: String::new(),
        }),
    }]);
    let ctx = VariableContext {
        file_path: Some("/src/main.rs".to_string()),
        ..VariableContext::empty()
    };
    let (text, _) = ActiveSnippet::expand(&body, Position::origin(), &ctx);
    // No match — original value preserved
    assert_eq!(text, "main.rs");
}

// =========================================================================
// Phase 5: Choice expansion via parser
// =========================================================================

#[test]
fn test_expand_choice_via_parser() {
    let body = parser::parse("type ${1|public,private|} $0").unwrap();
    let (text, snippet) =
        ActiveSnippet::expand(&body, Position::origin(), &VariableContext::empty());
    assert_eq!(text, "type public ");
    // $1 choice + $0
    assert_eq!(snippet.tab_stop_count(), 2);
    assert_eq!(snippet.tab_stops()[0].id, 1);
    assert_eq!(snippet.tab_stops()[0].placeholder, "public");
    assert_eq!(snippet.tab_stops()[1].id, 0);
}
