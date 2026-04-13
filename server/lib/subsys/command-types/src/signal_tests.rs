use super::*;

#[test]
fn test_runtime_signal_quit_exists() {
    let signal = RuntimeSignal::Quit;
    assert_eq!(signal, RuntimeSignal::Quit);
}

#[test]
fn test_runtime_signal_debug_format() {
    let debug = format!("{:?}", RuntimeSignal::Quit);
    assert_eq!(debug, "Quit");
}

#[test]
fn test_runtime_signal_clone() {
    let signal = RuntimeSignal::Quit;
    let cloned = signal.clone();
    assert_eq!(signal, cloned);
}

#[test]
fn test_runtime_signal_eq() {
    assert_eq!(RuntimeSignal::Quit, RuntimeSignal::Quit);
}

#[test]
fn test_runtime_signal_exhaustive_match() {
    // Ensures adding a new variant requires updating this test
    let signal = RuntimeSignal::Quit;
    match signal {
        RuntimeSignal::Quit => {}
    }
}
