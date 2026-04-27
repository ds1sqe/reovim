use super::super::*;

#[test]
fn test_default_state() {
    assert_eq!(RuntimeState::default(), RuntimeState::Booting);
}

#[test]
fn test_is_running() {
    assert!(!RuntimeState::Booting.is_running());
    assert!(RuntimeState::Running.is_running());
    assert!(!RuntimeState::Stopping.is_running());
    assert!(!RuntimeState::Emergency.is_running());
}

#[test]
fn test_is_shutting_down() {
    assert!(!RuntimeState::Booting.is_shutting_down());
    assert!(!RuntimeState::Running.is_shutting_down());
    assert!(RuntimeState::Stopping.is_shutting_down());
    assert!(RuntimeState::Emergency.is_shutting_down());
}

#[test]
fn test_can_accept_work() {
    assert!(!RuntimeState::Booting.can_accept_work());
    assert!(RuntimeState::Running.can_accept_work());
    assert!(!RuntimeState::Stopping.can_accept_work());
    assert!(!RuntimeState::Emergency.can_accept_work());
}

#[test]
fn test_is_terminal() {
    assert!(!RuntimeState::Booting.is_terminal());
    assert!(!RuntimeState::Running.is_terminal());
    assert!(RuntimeState::Stopping.is_terminal());
    assert!(RuntimeState::Emergency.is_terminal());
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_display() {
    assert_eq!(format!("{}", RuntimeState::Booting), "Booting");
    assert_eq!(format!("{}", RuntimeState::Running), "Running");
    assert_eq!(format!("{}", RuntimeState::Stopping), "Stopping");
    assert_eq!(format!("{}", RuntimeState::Emergency), "Emergency");
}

#[test]
#[allow(clippy::clone_on_copy)] // Intentionally testing Clone impl
fn test_clone_and_copy() {
    let state = RuntimeState::Running;
    let cloned = state.clone();
    let copied = state;
    assert_eq!(state, cloned);
    assert_eq!(state, copied);
}

#[test]
fn test_hash() {
    use std::collections::HashSet;
    let mut set = HashSet::new();
    set.insert(RuntimeState::Booting);
    set.insert(RuntimeState::Running);
    assert!(set.contains(&RuntimeState::Booting));
    assert!(set.contains(&RuntimeState::Running));
    assert!(!set.contains(&RuntimeState::Stopping));
}
