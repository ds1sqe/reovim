use super::*;

#[test]
fn test_module_id() {
    assert_eq!(MODULE.as_str(), "illuminate");
}

#[test]
fn test_next_reference_id() {
    assert_eq!(NEXT_REFERENCE.module().as_str(), "illuminate");
    assert_eq!(NEXT_REFERENCE.name(), "next-reference");
}

#[test]
fn test_prev_reference_id() {
    assert_eq!(PREV_REFERENCE.module().as_str(), "illuminate");
    assert_eq!(PREV_REFERENCE.name(), "prev-reference");
}

#[test]
fn test_command_ids_are_unique() {
    assert_ne!(NEXT_REFERENCE, PREV_REFERENCE);
}
