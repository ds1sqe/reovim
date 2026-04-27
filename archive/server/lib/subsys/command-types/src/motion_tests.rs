use super::*;

#[test]
fn test_motion_type_default() {
    let motion = MotionType::default();
    assert_eq!(motion, MotionType::Characterwise);
    assert!(motion.is_characterwise());
    assert!(!motion.is_linewise());
}

#[test]
fn test_motion_type_characterwise() {
    let motion = MotionType::Characterwise;
    assert!(motion.is_characterwise());
    assert!(!motion.is_linewise());
}

#[test]
fn test_motion_type_linewise() {
    let motion = MotionType::Linewise;
    assert!(motion.is_linewise());
    assert!(!motion.is_characterwise());
}

#[test]
fn test_motion_type_equality() {
    assert_eq!(MotionType::Characterwise, MotionType::Characterwise);
    assert_eq!(MotionType::Linewise, MotionType::Linewise);
    assert_ne!(MotionType::Characterwise, MotionType::Linewise);
}

#[test]
fn test_motion_type_copy_clone() {
    let motion = MotionType::Linewise;
    let copied = motion;
    #[allow(clippy::clone_on_copy)]
    let cloned = motion.clone();
    assert_eq!(motion, copied);
    assert_eq!(motion, cloned);
}

#[test]
fn test_motion_type_hash() {
    use std::collections::HashSet;

    let mut set = HashSet::new();
    set.insert(MotionType::Characterwise);
    set.insert(MotionType::Linewise);

    assert_eq!(set.len(), 2);
    assert!(set.contains(&MotionType::Characterwise));
    assert!(set.contains(&MotionType::Linewise));
}

#[test]
fn test_motion_type_hash_duplicate_insert() {
    use std::collections::HashSet;

    let mut set = HashSet::new();
    set.insert(MotionType::Characterwise);
    set.insert(MotionType::Characterwise);

    assert_eq!(set.len(), 1);
}

#[test]
fn test_motion_type_debug() {
    let debug_char = format!("{:?}", MotionType::Characterwise);
    assert_eq!(debug_char, "Characterwise");

    let debug_line = format!("{:?}", MotionType::Linewise);
    assert_eq!(debug_line, "Linewise");
}
