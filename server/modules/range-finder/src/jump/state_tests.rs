use super::*;

fn make_lines_with_matches(count: usize) -> Vec<String> {
    // Create lines with exactly `count` occurrences of "he".
    let mut lines = Vec::new();
    let mut remaining = count;
    while remaining > 0 {
        let per_line = remaining.min(10);
        let line = "he ".repeat(per_line);
        lines.push(line.trim_end().to_string());
        remaining -= per_line;
    }
    lines
}

#[test]
fn test_default_inactive() {
    let state = JumpSessionState::default();
    assert!(!state.is_active());
    assert!(state.get_matches().is_none());
}

#[test]
fn test_start_transitions_to_waiting_first() {
    let mut state = JumpSessionState::default();
    state.start(vec!["hello".into()], 0, 0, Direction::Both);
    assert!(state.is_active());
}

#[test]
fn test_first_char_transitions_to_waiting_second() {
    let mut state = JumpSessionState::default();
    state.start(vec!["hello world".into()], 0, 100, Direction::Both);
    state.insert_char('h');
    // Should be in WaitingSecondChar now (still active, no matches shown).
    assert!(state.is_active());
    assert!(state.get_matches().is_none());
}

#[test]
fn test_second_char_zero_matches_cancels() {
    let mut state = JumpSessionState::default();
    state.start(vec!["hello world".into()], 0, 0, Direction::Both);
    state.insert_char('z');
    state.insert_char('z');
    assert!(!state.is_active());
    assert!(state.take_target().is_none());
}

#[test]
fn test_second_char_one_match_auto_jumps() {
    let mut state = JumpSessionState::default();
    state.start(vec!["hello world".into()], 0, 100, Direction::Both);
    state.insert_char('w');
    state.insert_char('o');
    // "wo" only matches once at col 6.
    assert!(!state.is_active());
    let target = state.take_target().expect("should have target");
    assert_eq!(target.line, 0);
    assert_eq!(target.col, 6);
}

#[test]
fn test_second_char_few_matches_shows_single_labels() {
    let mut state = JumpSessionState::default();
    // 3 matches of "he": need to be careful with cursor position.
    let lines = vec!["he he he".into()];
    state.start(lines, 0, 100, Direction::Both);
    state.insert_char('h');
    state.insert_char('e');
    // Should be ShowingLabels with single-char labels.
    assert!(state.is_active());
    let matches = state.get_matches().expect("should have matches");
    assert_eq!(matches.len(), 3);
    assert_eq!(matches[0].label.len(), 1); // single-char labels
}

#[test]
fn test_second_char_many_matches_shows_two_char_labels() {
    let mut state = JumpSessionState::default();
    let lines = make_lines_with_matches(30);
    state.start(lines, 100, 100, Direction::Both);
    state.insert_char('h');
    state.insert_char('e');
    assert!(state.is_active());
    let matches = state.get_matches().expect("should have matches");
    assert!(matches.len() >= 27);
    assert_eq!(matches[0].label.len(), 2); // two-char labels
}

#[test]
fn test_second_char_over_max_shows_capped_labels() {
    // >676 matches should be capped at 676 by find_matches.
    let mut state = JumpSessionState::default();
    let lines = make_lines_with_matches(700);
    state.start(lines, 10000, 10000, Direction::Both);
    state.insert_char('h');
    state.insert_char('e');
    // Should show labels (capped at 676), not cancel.
    assert!(state.is_active());
    let matches = state.get_matches().expect("should have matches");
    assert!(matches.len() <= 676);
}

#[test]
fn test_label_select_single_char_match() {
    let mut state = JumpSessionState::default();
    let lines = vec!["he he he".into()];
    state.start(lines, 0, 100, Direction::Both);
    state.insert_char('h');
    state.insert_char('e');

    // First label is "s".
    let matches = state.get_matches().unwrap();
    let first_label = matches[0].label.clone();
    let expected_line = matches[0].line;
    let expected_col = matches[0].col;

    state.insert_char(first_label.chars().next().unwrap());
    assert!(!state.is_active());
    let target = state.take_target().expect("should have target");
    assert_eq!(target.line, expected_line);
    assert_eq!(target.col, expected_col);
}

#[test]
fn test_label_select_single_char_no_match() {
    let mut state = JumpSessionState::default();
    let lines = vec!["he he he".into()];
    state.start(lines, 0, 100, Direction::Both);
    state.insert_char('h');
    state.insert_char('e');

    // Type a char that's not a valid label.
    state.insert_char('1');
    assert!(!state.is_active());
    assert!(state.take_target().is_none());
}

#[test]
fn test_label_select_two_char_first() {
    let mut state = JumpSessionState::default();
    let lines = make_lines_with_matches(30);
    state.start(lines, 100, 100, Direction::Both);
    state.insert_char('h');
    state.insert_char('e');

    // First label should be "ss". Type 's' first.
    assert!(state.is_active());
    state.insert_char('s');
    // Should be in WaitingLabelSecondChar now.
    assert!(state.is_active());
    assert!(state.get_matches().is_some());
}

#[test]
fn test_label_select_two_char_complete() {
    let mut state = JumpSessionState::default();
    let lines = make_lines_with_matches(30);
    state.start(lines, 100, 100, Direction::Both);
    state.insert_char('h');
    state.insert_char('e');

    let matches = state.get_matches().unwrap();
    let first_label = matches[0].label.clone();
    let expected_line = matches[0].line;
    let expected_col = matches[0].col;
    assert_eq!(first_label.len(), 2);

    let mut chars = first_label.chars();
    state.insert_char(chars.next().unwrap());
    state.insert_char(chars.next().unwrap());

    assert!(!state.is_active());
    let target = state.take_target().expect("should have target");
    assert_eq!(target.line, expected_line);
    assert_eq!(target.col, expected_col);
}

#[test]
fn test_label_select_two_char_no_match() {
    let mut state = JumpSessionState::default();
    let lines = make_lines_with_matches(30);
    state.start(lines, 100, 100, Direction::Both);
    state.insert_char('h');
    state.insert_char('e');

    // Type valid first char, invalid second char.
    state.insert_char('s');
    state.insert_char('1'); // Not a valid label char.
    assert!(!state.is_active());
    assert!(state.take_target().is_none());
}

#[test]
fn test_cancel_from_waiting_first() {
    let mut state = JumpSessionState::default();
    state.start(vec!["hello".into()], 0, 0, Direction::Both);
    state.cancel();
    assert!(!state.is_active());
}

#[test]
fn test_cancel_from_waiting_second() {
    let mut state = JumpSessionState::default();
    state.start(vec!["hello".into()], 0, 0, Direction::Both);
    state.insert_char('h');
    state.cancel();
    assert!(!state.is_active());
}

#[test]
fn test_cancel_from_showing_labels() {
    let mut state = JumpSessionState::default();
    let lines = vec!["he he he".into()];
    state.start(lines, 0, 100, Direction::Both);
    state.insert_char('h');
    state.insert_char('e');
    assert!(state.is_active());
    state.cancel();
    assert!(!state.is_active());
}

#[test]
fn test_cancel_from_waiting_label_second() {
    let mut state = JumpSessionState::default();
    let lines = make_lines_with_matches(30);
    state.start(lines, 100, 100, Direction::Both);
    state.insert_char('h');
    state.insert_char('e');
    state.insert_char('s'); // First char of two-char label.
    assert!(state.is_active());
    state.cancel();
    assert!(!state.is_active());
}

#[test]
fn test_insert_char_when_inactive_noop() {
    let mut state = JumpSessionState::default();
    state.insert_char('a');
    assert!(!state.is_active());
    assert!(state.take_target().is_none());
}

#[test]
fn test_take_target_consumes() {
    let mut state = JumpSessionState::default();
    state.start(vec!["hello world".into()], 0, 100, Direction::Both);
    state.insert_char('w');
    state.insert_char('o');
    assert!(state.take_target().is_some());
    assert!(state.take_target().is_none());
}

#[test]
fn test_take_target_none_when_no_jump() {
    let state = JumpSessionState::default();
    // Can't call take_target on non-mut default without starting.
    let mut state2 = JumpSessionState::default();
    assert!(state2.take_target().is_none());
    // Suppress unused warning.
    let _ = state;
}

#[test]
fn test_is_active_states() {
    let mut state = JumpSessionState::default();
    assert!(!state.is_active()); // Inactive

    state.start(vec!["he he he".into()], 0, 100, Direction::Both);
    assert!(state.is_active()); // WaitingFirstChar

    state.insert_char('h');
    assert!(state.is_active()); // WaitingSecondChar

    state.insert_char('e');
    assert!(state.is_active()); // ShowingLabels
}

#[test]
fn test_get_matches_showing_labels() {
    let mut state = JumpSessionState::default();
    let lines = vec!["he he he".into()];
    state.start(lines, 0, 100, Direction::Both);
    state.insert_char('h');
    state.insert_char('e');

    let matches = state.get_matches();
    assert!(matches.is_some());
    assert_eq!(matches.unwrap().len(), 3);
}

#[test]
fn test_get_matches_inactive() {
    let state = JumpSessionState::default();
    assert!(state.get_matches().is_none());
}

#[test]
fn test_session_extension_create() {
    let state = JumpSessionState::create();
    assert!(!state.is_active());
}

#[test]
fn test_text_input_sink_available() {
    let mut state = JumpSessionState::default();
    assert!(SessionExtension::as_text_input_sink(&mut state).is_some());
}

#[test]
fn test_two_char_label_invalid_first_char() {
    let mut state = JumpSessionState::default();
    let lines = make_lines_with_matches(30);
    state.start(lines, 100, 100, Direction::Both);
    state.insert_char('h');
    state.insert_char('e');

    // Labels are two-char. Type '1' which isn't a valid label prefix.
    state.insert_char('1');
    assert!(!state.is_active());
    assert!(state.take_target().is_none());
}

#[test]
fn test_has_target_true_after_auto_jump() {
    let mut state = JumpSessionState::default();
    state.start(vec!["hello world".into()], 0, 100, Direction::Both);
    state.insert_char('w');
    state.insert_char('o');
    assert!(state.has_target());
}

#[test]
fn test_has_target_false_when_inactive() {
    let state = JumpSessionState::default();
    assert!(!state.has_target());
}

#[test]
fn test_has_target_false_after_take() {
    let mut state = JumpSessionState::default();
    state.start(vec!["hello world".into()], 0, 100, Direction::Both);
    state.insert_char('w');
    state.insert_char('o');
    assert!(state.has_target());
    let _ = state.take_target();
    assert!(!state.has_target());
}

#[test]
fn test_generate_labels_called_correctly() {
    // Verify generate_labels is consistent with what state machine expects.
    let labels = crate::jump::search::generate_labels(5);
    assert_eq!(labels.len(), 5);
    assert_eq!(labels[0], "s");
}

// =========================================================================
// start_with_matches
// =========================================================================

#[test]
fn test_start_with_matches_empty() {
    let mut state = JumpSessionState::default();
    state.start_with_matches(vec![]);
    assert!(!state.is_active());
}

#[test]
fn test_start_with_matches_shows_labels() {
    let mut state = JumpSessionState::default();
    let labels = crate::jump::search::generate_labels(3);
    let matches = vec![
        JumpMatch::new(0, 5, labels[0].clone(), 5),
        JumpMatch::new(0, 10, labels[1].clone(), 10),
        JumpMatch::new(0, 15, labels[2].clone(), 15),
    ];
    state.start_with_matches(matches);
    assert!(state.is_active());
    let m = state.get_matches().expect("should have matches");
    assert_eq!(m.len(), 3);
    assert_eq!(m[0].label, "s");
}

#[test]
fn test_start_with_matches_label_selection() {
    let mut state = JumpSessionState::default();
    let labels = crate::jump::search::generate_labels(3);
    let matches = vec![
        JumpMatch::new(0, 5, labels[0].clone(), 5),
        JumpMatch::new(0, 10, labels[1].clone(), 10),
        JumpMatch::new(0, 15, labels[2].clone(), 15),
    ];
    state.start_with_matches(matches);

    // Select second label ("f")
    state.insert_char('f');
    assert!(!state.is_active());
    let target = state.take_target().expect("should have target");
    assert_eq!(target.line, 0);
    assert_eq!(target.col, 10);
}

#[test]
fn test_start_with_matches_cancel() {
    let mut state = JumpSessionState::default();
    let labels = crate::jump::search::generate_labels(2);
    let matches = vec![
        JumpMatch::new(0, 5, labels[0].clone(), 5),
        JumpMatch::new(0, 10, labels[1].clone(), 10),
    ];
    state.start_with_matches(matches);
    state.cancel();
    assert!(!state.is_active());
    assert!(state.take_target().is_none());
}

#[test]
fn test_start_with_matches_clears_previous_target() {
    let mut state = JumpSessionState::default();
    // First: auto-jump sets a target
    state.start(vec!["hello world".into()], 0, 100, Direction::Both);
    state.insert_char('w');
    state.insert_char('o');
    assert!(state.has_target());

    // start_with_matches clears the old target
    let labels = crate::jump::search::generate_labels(2);
    let matches = vec![
        JumpMatch::new(0, 5, labels[0].clone(), 5),
        JumpMatch::new(0, 10, labels[1].clone(), 10),
    ];
    state.start_with_matches(matches);
    assert!(!state.has_target());
    assert!(state.is_active());
}

#[test]
#[allow(clippy::cast_possible_truncation)]
fn test_start_with_matches_two_char_labels() {
    let mut state = JumpSessionState::default();
    let labels = crate::jump::search::generate_labels(30);
    let matches: Vec<JumpMatch> = labels
        .iter()
        .enumerate()
        .map(|(i, l)| JumpMatch::new(0, i as u32, l.clone(), i as u32))
        .collect();
    state.start_with_matches(matches);
    assert!(state.is_active());

    let m = state.get_matches().unwrap();
    assert_eq!(m[0].label.len(), 2); // Two-char labels

    // Select "ss" (first label)
    state.insert_char('s');
    assert!(state.is_active()); // Waiting for second char
    state.insert_char('s');
    assert!(!state.is_active());
    let target = state.take_target().expect("should have target");
    assert_eq!(target.col, 0);
}
