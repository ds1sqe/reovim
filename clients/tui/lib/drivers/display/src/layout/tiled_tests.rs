use super::*;

#[test]
fn test_min_dimensions() {
    const { assert!(MIN_WINDOW_WIDTH >= 1) };
    const { assert!(MIN_WINDOW_HEIGHT >= 1) };
}
