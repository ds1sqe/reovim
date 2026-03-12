use super::*;

#[test]
fn test_search_state_default() {
    let state = SearchState::default();
    assert!(state.last_pattern.is_none());
    assert!(matches!(state.last_direction, Direction::Forward));
}

#[test]
fn test_search_state_set() {
    let mut state = SearchState::default();
    state.set("hello".to_string(), Direction::Backward);
    assert_eq!(state.pattern_for_repeat(), Some("hello"));
    assert!(matches!(state.direction_for_repeat(), Direction::Backward));
    assert!(matches!(state.direction_for_opposite(), Direction::Forward));
}

#[test]
fn test_pending_search() {
    let mut state = SearchState::default();
    assert!(!state.has_pending_search());

    state.start_pending_search(Direction::Forward);
    assert!(state.has_pending_search());

    let dir = state.take_pending_search();
    assert_eq!(dir, Some(Direction::Forward));
    assert!(!state.has_pending_search());
}

#[test]
fn test_clear_pending_search() {
    let mut state = SearchState::default();
    state.start_pending_search(Direction::Backward);
    assert!(state.has_pending_search());

    state.clear_pending_search();
    assert!(!state.has_pending_search());
    assert!(state.take_pending_search().is_none());
}

#[test]
fn test_pending_search_backward() {
    let mut state = SearchState::default();
    state.start_pending_search(Direction::Backward);
    assert!(state.has_pending_search());

    let dir = state.take_pending_search();
    assert_eq!(dir, Some(Direction::Backward));
}

#[test]
fn test_take_pending_search_when_none() {
    let mut state = SearchState::default();
    assert!(state.take_pending_search().is_none());
}

#[test]
fn test_direction_for_opposite_forward() {
    let mut state = SearchState::default();
    state.set("hello".to_string(), Direction::Forward);
    assert!(matches!(state.direction_for_opposite(), Direction::Backward));
}

#[test]
fn test_direction_for_opposite_backward() {
    let mut state = SearchState::default();
    state.set("hello".to_string(), Direction::Backward);
    assert!(matches!(state.direction_for_opposite(), Direction::Forward));
}

#[test]
fn test_pattern_for_repeat_none_by_default() {
    let state = SearchState::default();
    assert!(state.pattern_for_repeat().is_none());
}

#[test]
fn test_set_overwrites_previous_pattern() {
    let mut state = SearchState::default();
    state.set("first".to_string(), Direction::Forward);
    assert_eq!(state.pattern_for_repeat(), Some("first"));

    state.set("second".to_string(), Direction::Backward);
    assert_eq!(state.pattern_for_repeat(), Some("second"));
    assert!(matches!(state.direction_for_repeat(), Direction::Backward));
}

#[test]
fn test_session_extension_create() {
    let state = SearchState::create();
    assert!(state.last_pattern.is_none());
    assert!(matches!(state.last_direction, Direction::Forward));
    assert!(!state.has_pending_search());
}

#[test]
fn test_search_state_debug() {
    let state = SearchState::default();
    let debug = format!("{state:?}");
    assert!(debug.contains("SearchState"));
}
