use super::*;

#[test]
fn direction_default_is_forward() {
    assert_eq!(Direction::default(), Direction::Forward);
}

#[test]
fn direction_debug() {
    assert_eq!(format!("{:?}", Direction::Forward), "Forward");
    assert_eq!(format!("{:?}", Direction::Backward), "Backward");
}

#[test]
fn direction_clone_copy_eq() {
    let a = Direction::Forward;
    let b = a;
    assert_eq!(a, b);
    assert_ne!(Direction::Forward, Direction::Backward);
}

#[test]
fn search_match_debug_clone_eq() {
    let m = SearchMatch {
        start: Position::new(0, 0),
        end: Position::new(0, 5),
    };
    let cloned = m.clone();
    assert_eq!(m, cloned);
    let debug = format!("{m:?}");
    assert!(debug.contains("SearchMatch"));
}

#[test]
fn search_error_display() {
    let err = SearchError::InvalidPattern("unclosed group".into());
    assert!(err.to_string().contains("E486"));
    assert!(err.to_string().contains("unclosed group"));
}

#[test]
fn search_error_debug() {
    let err = SearchError::InvalidPattern("bad".into());
    let debug = format!("{err:?}");
    assert!(debug.contains("InvalidPattern"));
}

#[test]
fn search_error_clone_eq() {
    let a = SearchError::InvalidPattern("x".into());
    let b = a.clone();
    assert_eq!(a, b);
}

#[test]
fn search_error_is_error() {
    let err = SearchError::InvalidPattern("test".into());
    let _: &dyn std::error::Error = &err;
}
