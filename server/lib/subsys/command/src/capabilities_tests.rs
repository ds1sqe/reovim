use super::*;

#[test]
fn capabilities_are_kebab_case() {
    let caps: &[&str] = &[COMMAND_DISPATCH];
    for cap in caps {
        assert!(
            !cap.is_empty()
                && cap
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c == '-' || c.is_ascii_digit()),
            "capability {cap:?} must be kebab-case ([a-z0-9-]+)"
        );
    }
}

#[test]
fn capabilities_have_no_duplicates() {
    let caps: &[&str] = &[COMMAND_DISPATCH];
    let unique: std::collections::HashSet<&&str> = caps.iter().collect();
    assert_eq!(unique.len(), caps.len(), "duplicate capability strings");
}
