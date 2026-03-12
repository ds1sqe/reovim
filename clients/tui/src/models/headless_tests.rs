use super::*;

#[test]
fn test_headless_model_new() {
    let model = HeadlessModel::new(80, 24);
    assert_eq!(model.size(), (80, 24));
}

#[test]
fn test_headless_model_resize() {
    let mut model = HeadlessModel::new(80, 24);
    model.resize(120, 40);
    assert_eq!(model.size(), (120, 40));
}

#[test]
fn test_headless_model_render_self_cursor() {
    let model = HeadlessModel::new(80, 24);
    assert!(model.render_self_cursor());
}

#[test]
fn test_headless_model_capture() {
    let model = HeadlessModel::new(5, 1);
    let capture = model.capture("plain").unwrap();
    // Empty frame buffer should have spaces
    assert_eq!(capture, "     ");
}

#[test]
fn test_headless_model_flush() {
    let mut model = HeadlessModel::new(80, 24);
    // Flush should succeed (no-op)
    assert!(model.flush().is_ok());
}
