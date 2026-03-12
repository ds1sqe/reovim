use super::*;

#[test]
fn test_valid_names() {
    assert!(validate_profile_name("default").is_ok());
    assert!(validate_profile_name("my-profile").is_ok());
    assert!(validate_profile_name("Profile_01").is_ok());
    assert!(validate_profile_name("a").is_ok());
    assert!(validate_profile_name("1-start-with-digit").is_ok());
}

#[test]
fn test_empty_name() {
    let err = validate_profile_name("").unwrap_err();
    assert!(err.contains("empty"));
}

#[test]
fn test_too_long() {
    let long = "a".repeat(65);
    let err = validate_profile_name(&long).unwrap_err();
    assert!(err.contains("too long"));
    assert!(err.contains("65"));

    // Exactly 64 is OK
    let exact = "a".repeat(64);
    assert!(validate_profile_name(&exact).is_ok());
}

#[test]
fn test_starts_with_hyphen() {
    let err = validate_profile_name("-bad").unwrap_err();
    assert!(err.contains("start with"));
}

#[test]
fn test_starts_with_underscore() {
    let err = validate_profile_name("_bad").unwrap_err();
    assert!(err.contains("start with"));
}

#[test]
fn test_path_traversal() {
    let err = validate_profile_name("../evil").unwrap_err();
    assert!(err.contains("invalid character") | err.contains("start with"));
}

#[test]
fn test_spaces() {
    let err = validate_profile_name("has space").unwrap_err();
    assert!(err.contains("invalid character"));
}

#[test]
fn test_unicode() {
    let err = validate_profile_name("café").unwrap_err();
    assert!(err.contains("invalid character"));
}

#[test]
fn test_dot() {
    let err = validate_profile_name("foo.bar").unwrap_err();
    assert!(err.contains("invalid character"));
}

#[test]
fn test_slash() {
    let err = validate_profile_name("foo/bar").unwrap_err();
    assert!(err.contains("invalid character"));
}
