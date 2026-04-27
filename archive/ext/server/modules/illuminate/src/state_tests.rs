use super::*;

#[test]
fn test_highlight_kind_as_str() {
    assert_eq!(HighlightKind::Text.as_str(), "text");
    assert_eq!(HighlightKind::Read.as_str(), "read");
    assert_eq!(HighlightKind::Write.as_str(), "write");
}

#[test]
fn test_highlight_kind_equality() {
    assert_eq!(HighlightKind::Text, HighlightKind::Text);
    assert_ne!(HighlightKind::Text, HighlightKind::Read);
    assert_ne!(HighlightKind::Read, HighlightKind::Write);
}

#[test]
fn test_highlight_kind_clone() {
    let kind = HighlightKind::Write;
    let cloned = kind;
    assert_eq!(kind, cloned);
}

#[test]
fn test_highlight_kind_debug() {
    let debug = format!("{:?}", HighlightKind::Text);
    assert_eq!(debug, "Text");
}

#[test]
fn test_highlight_range_debug() {
    let range = HighlightRange {
        start_line: 1,
        start_col: 5,
        end_line: 1,
        end_col: 10,
        kind: HighlightKind::Read,
    };
    let debug = format!("{range:?}");
    assert!(debug.contains("start_line: 1"));
    assert!(debug.contains("Read"));
}

#[test]
fn test_highlight_range_clone() {
    let range = HighlightRange {
        start_line: 0,
        start_col: 0,
        end_line: 0,
        end_col: 5,
        kind: HighlightKind::Text,
    };
    let cloned = range.clone();
    assert_eq!(range, cloned);
}

#[test]
fn test_state_create_defaults() {
    let state = IlluminateState::create();
    assert!(!state.active);
    assert!(state.word.is_empty());
    assert!(state.ranges.is_empty());
    assert_eq!(state.sequence, 0);
    assert_eq!(state.origin_line, 0);
    assert_eq!(state.origin_col, 0);
    assert_eq!(state.shadow_line, u32::MAX);
    assert_eq!(state.shadow_col, u32::MAX);
    assert_eq!(state.shadow_buffer_id, BufferId::from_raw(0));
    assert_eq!(state.idle_ticks, 0);
    assert!(!state.computed);
}

#[test]
fn test_state_set_highlights() {
    let mut state = IlluminateState::create();
    let ranges = vec![HighlightRange {
        start_line: 5,
        start_col: 0,
        end_line: 5,
        end_col: 3,
        kind: HighlightKind::Text,
    }];

    state.set_highlights(BufferId::from_raw(1), "foo".to_string(), ranges, 5, 0);

    assert!(state.active);
    assert_eq!(state.word, "foo");
    assert_eq!(state.ranges.len(), 1);
    assert_eq!(state.origin_line, 5);
    assert_eq!(state.origin_col, 0);
    assert_eq!(state.sequence, 1);
    assert!(state.computed);
}

#[test]
fn test_state_set_highlights_increments_sequence() {
    let mut state = IlluminateState::create();
    state.set_highlights(BufferId::from_raw(1), "a".to_string(), vec![], 0, 0);
    assert_eq!(state.sequence, 1);

    state.set_highlights(BufferId::from_raw(1), "b".to_string(), vec![], 1, 0);
    assert_eq!(state.sequence, 2);
}

#[test]
fn test_state_clear() {
    let mut state = IlluminateState::create();
    state.set_highlights(
        BufferId::from_raw(1),
        "foo".to_string(),
        vec![HighlightRange {
            start_line: 0,
            start_col: 0,
            end_line: 0,
            end_col: 3,
            kind: HighlightKind::Text,
        }],
        0,
        0,
    );
    let seq_before = state.sequence;

    state.clear();

    assert!(!state.active);
    assert!(state.word.is_empty());
    assert!(state.ranges.is_empty());
    assert!(!state.computed);
    // Sequence incremented because was active
    assert_eq!(state.sequence, seq_before + 1);
}

#[test]
fn test_state_clear_when_inactive_does_not_increment_sequence() {
    let mut state = IlluminateState::create();
    assert_eq!(state.sequence, 0);

    state.clear();

    assert_eq!(state.sequence, 0);
}

#[test]
fn test_cursor_moved_resets_idle() {
    let mut state = IlluminateState::create();
    state.idle_ticks = 5;
    state.computed = true;

    state.cursor_moved(BufferId::from_raw(7), 10, 3);

    assert_eq!(state.shadow_buffer_id, BufferId::from_raw(7));
    assert_eq!(state.shadow_line, 10);
    assert_eq!(state.shadow_col, 3);
    assert_eq!(state.idle_ticks, 0);
    assert!(!state.computed);
}

#[test]
fn test_cursor_moved_buffer_change_resets_idle() {
    let mut state = IlluminateState::create();
    state.shadow_buffer_id = BufferId::from_raw(1);
    state.shadow_line = 10;
    state.shadow_col = 3;
    state.idle_ticks = 5;
    state.computed = true;

    state.cursor_moved(BufferId::from_raw(2), 10, 3);

    assert_eq!(state.shadow_buffer_id, BufferId::from_raw(2));
    assert_eq!(state.idle_ticks, 0);
    assert!(!state.computed);
}

#[test]
fn test_cursor_moved_same_position_no_reset() {
    let mut state = IlluminateState::create();
    state.shadow_buffer_id = BufferId::from_raw(1);
    state.shadow_line = 10;
    state.shadow_col = 3;
    state.idle_ticks = 5;
    state.computed = true;

    state.cursor_moved(BufferId::from_raw(1), 10, 3);

    // Should not reset — same position
    assert_eq!(state.idle_ticks, 5);
    assert!(state.computed);
}

#[test]
fn test_tick_increments_counter() {
    let mut state = IlluminateState::create();
    assert_eq!(state.tick(), 1);
    assert_eq!(state.tick(), 2);
    assert_eq!(state.tick(), 3);
}

// ========================================================================
// Navigation: next_range_index
// ========================================================================

fn make_ranges() -> Vec<HighlightRange> {
    vec![
        HighlightRange {
            start_line: 2,
            start_col: 5,
            end_line: 2,
            end_col: 8,
            kind: HighlightKind::Text,
        },
        HighlightRange {
            start_line: 5,
            start_col: 10,
            end_line: 5,
            end_col: 13,
            kind: HighlightKind::Read,
        },
        HighlightRange {
            start_line: 8,
            start_col: 0,
            end_line: 8,
            end_col: 3,
            kind: HighlightKind::Write,
        },
    ]
}

#[test]
fn test_next_range_no_ranges() {
    let state = IlluminateState::create();
    assert!(state.next_range_index(0, 0).is_none());
}

#[test]
fn test_next_range_before_first() {
    let mut state = IlluminateState::create();
    state.ranges = make_ranges();
    assert_eq!(state.next_range_index(0, 0), Some(0));
}

#[test]
fn test_next_range_between_ranges() {
    let mut state = IlluminateState::create();
    state.ranges = make_ranges();
    assert_eq!(state.next_range_index(3, 0), Some(1));
}

#[test]
fn test_next_range_at_last_wraps() {
    let mut state = IlluminateState::create();
    state.ranges = make_ranges();
    // Past the last range — wraps to 0
    assert_eq!(state.next_range_index(10, 0), Some(0));
}

#[test]
fn test_next_range_same_line_different_col() {
    let mut state = IlluminateState::create();
    state.ranges = make_ranges();
    // At (2, 4) — before range[0] at (2, 5)
    assert_eq!(state.next_range_index(2, 4), Some(0));
    // At (2, 5) — at range[0], next is range[1]
    assert_eq!(state.next_range_index(2, 5), Some(1));
}

// ========================================================================
// Navigation: prev_range_index
// ========================================================================

#[test]
fn test_prev_range_no_ranges() {
    let state = IlluminateState::create();
    assert!(state.prev_range_index(0, 0).is_none());
}

#[test]
fn test_prev_range_after_last() {
    let mut state = IlluminateState::create();
    state.ranges = make_ranges();
    assert_eq!(state.prev_range_index(10, 0), Some(2));
}

#[test]
fn test_prev_range_between_ranges() {
    let mut state = IlluminateState::create();
    state.ranges = make_ranges();
    assert_eq!(state.prev_range_index(6, 0), Some(1));
}

#[test]
fn test_prev_range_before_first_wraps() {
    let mut state = IlluminateState::create();
    state.ranges = make_ranges();
    // Before all ranges — wraps to last
    assert_eq!(state.prev_range_index(0, 0), Some(2));
}

#[test]
fn test_prev_range_same_line_different_col() {
    let mut state = IlluminateState::create();
    state.ranges = make_ranges();
    // At (2, 6) — after range[0] at (2, 5)
    assert_eq!(state.prev_range_index(2, 6), Some(0));
    // At (2, 5) — at range[0], prev wraps to last
    assert_eq!(state.prev_range_index(2, 5), Some(2));
}

#[test]
fn test_state_debug() {
    let state = IlluminateState::create();
    let debug = format!("{state:?}");
    assert!(debug.contains("IlluminateState"));
    assert!(debug.contains("active: false"));
}
