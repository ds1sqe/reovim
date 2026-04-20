use super::*;

#[test]
fn test_terminal_size() {
    // Should not fail even in CI (will get default size)
    let result = Terminal::size();
    // We don't assert Ok because CI may not have a terminal
    // Just verify it doesn't panic
    let _ = result;
}
