use super::*;

#[test]
fn test_registration_flags_default() {
    let flags = RegistrationFlags::new();
    assert!(!flags.is_required());
    assert!(!flags.is_deferrable());
    assert!(!flags.is_early());
    assert!(!flags.is_fallback());

    // Default trait should match new()
    let default_flags = RegistrationFlags::default();
    assert_eq!(flags, default_flags);
}

#[test]
fn test_registration_flags_required() {
    let flags = RegistrationFlags::required();
    assert!(flags.is_required());
    assert!(!flags.is_deferrable());
    assert!(!flags.is_early());
    assert!(!flags.is_fallback());
}

#[test]
fn test_registration_flags_deferrable() {
    let flags = RegistrationFlags::deferrable();
    assert!(!flags.is_required());
    assert!(flags.is_deferrable());
    assert!(!flags.is_early());
    assert!(!flags.is_fallback());
}

#[test]
fn test_registration_flags_chainable_builders() {
    // Test chaining multiple flags
    let flags = RegistrationFlags::new().set_required().set_early();

    assert!(flags.is_required());
    assert!(!flags.is_deferrable());
    assert!(flags.is_early());
    assert!(!flags.is_fallback());

    // Test all flags together
    let all_flags = RegistrationFlags::new()
        .set_required()
        .set_deferrable()
        .set_early()
        .set_fallback();

    assert!(all_flags.is_required());
    assert!(all_flags.is_deferrable());
    assert!(all_flags.is_early());
    assert!(all_flags.is_fallback());
}

#[test]
fn test_registration_flags_query_methods() {
    let flags = RegistrationFlags::new().set_required().set_early();

    assert!(flags.is_required());
    assert!(!flags.is_deferrable());
    assert!(flags.is_early());
    assert!(!flags.is_fallback());
}
