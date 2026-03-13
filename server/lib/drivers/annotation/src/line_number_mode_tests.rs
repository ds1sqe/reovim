use super::*;

#[test]
fn test_line_number_mode_default() {
    let mode = LineNumberMode::default();
    assert_eq!(mode, LineNumberMode::None);
}

#[test]
fn test_line_number_mode_variants() {
    assert_eq!(LineNumberMode::None, LineNumberMode::None);
    assert_eq!(LineNumberMode::Absolute, LineNumberMode::Absolute);
    assert_eq!(LineNumberMode::Relative, LineNumberMode::Relative);
    assert_eq!(LineNumberMode::Hybrid, LineNumberMode::Hybrid);
}

#[test]
fn test_line_number_mode_inequality() {
    assert_ne!(LineNumberMode::None, LineNumberMode::Absolute);
    assert_ne!(LineNumberMode::Absolute, LineNumberMode::Relative);
    assert_ne!(LineNumberMode::Relative, LineNumberMode::Hybrid);
}

#[test]
fn test_line_number_mode_debug() {
    assert_eq!(format!("{:?}", LineNumberMode::None), "None");
    assert_eq!(format!("{:?}", LineNumberMode::Absolute), "Absolute");
    assert_eq!(format!("{:?}", LineNumberMode::Relative), "Relative");
    assert_eq!(format!("{:?}", LineNumberMode::Hybrid), "Hybrid");
}

#[test]
fn test_line_number_mode_clone() {
    let mode = LineNumberMode::Hybrid;
    let cloned = mode;
    assert_eq!(mode, cloned);
}

#[test]
fn test_line_number_mode_copy() {
    let mode = LineNumberMode::Absolute;
    let copied = mode;
    assert_eq!(mode, copied);
    // Both are still valid (Copy trait)
    assert_eq!(mode, LineNumberMode::Absolute);
    assert_eq!(copied, LineNumberMode::Absolute);
}
