use super::*;

// =========================================================================
// Parsing
// =========================================================================

#[test]
fn test_parse_compatible_caret() {
    assert_eq!(VersionRange::parse("^1.2.3"), Some(VersionRange::Compatible((1, 2, 3))));
}

#[test]
fn test_parse_compatible_bare() {
    assert_eq!(VersionRange::parse("1.2.3"), Some(VersionRange::Compatible((1, 2, 3))));
}

#[test]
fn test_parse_exact() {
    assert_eq!(VersionRange::parse("=1.2.3"), Some(VersionRange::Exact((1, 2, 3))));
}

#[test]
fn test_parse_at_least() {
    assert_eq!(VersionRange::parse(">=1.2.3"), Some(VersionRange::AtLeast((1, 2, 3))));
}

#[test]
fn test_parse_with_spaces() {
    assert_eq!(VersionRange::parse("  ^1.2.3  "), Some(VersionRange::Compatible((1, 2, 3))));
}

#[test]
fn test_parse_empty() {
    assert_eq!(VersionRange::parse(""), None);
}

#[test]
fn test_parse_invalid_no_patch() {
    assert_eq!(VersionRange::parse("1.2"), None);
}

#[test]
fn test_parse_invalid_extra_parts() {
    assert_eq!(VersionRange::parse("1.2.3.4"), None);
}

#[test]
fn test_parse_invalid_non_numeric() {
    assert_eq!(VersionRange::parse("abc"), None);
}

#[test]
fn test_parse_invalid_partial_prefix() {
    assert_eq!(VersionRange::parse("^"), None);
    assert_eq!(VersionRange::parse("="), None);
    assert_eq!(VersionRange::parse(">="), None);
}

// =========================================================================
// Compatible (^) — post-1.0
// =========================================================================

#[test]
fn test_compatible_exact_match() {
    let range = VersionRange::Compatible((1, 2, 3));
    assert!(range.satisfies((1, 2, 3)));
}

#[test]
fn test_compatible_higher_patch() {
    let range = VersionRange::Compatible((1, 2, 3));
    assert!(range.satisfies((1, 2, 5)));
}

#[test]
fn test_compatible_higher_minor() {
    let range = VersionRange::Compatible((1, 2, 3));
    assert!(range.satisfies((1, 5, 0)));
}

#[test]
fn test_compatible_rejects_next_major() {
    let range = VersionRange::Compatible((1, 2, 3));
    assert!(!range.satisfies((2, 0, 0)));
}

#[test]
fn test_compatible_rejects_lower() {
    let range = VersionRange::Compatible((1, 2, 3));
    assert!(!range.satisfies((1, 2, 2)));
}

// =========================================================================
// Compatible (^) — pre-1.0
// =========================================================================

#[test]
fn test_compatible_pre_1_same_minor() {
    let range = VersionRange::Compatible((0, 2, 3));
    assert!(range.satisfies((0, 2, 5)));
}

#[test]
fn test_compatible_pre_1_rejects_next_minor() {
    let range = VersionRange::Compatible((0, 2, 3));
    assert!(!range.satisfies((0, 3, 0)));
}

#[test]
fn test_compatible_pre_01_exact_only() {
    let range = VersionRange::Compatible((0, 0, 3));
    assert!(range.satisfies((0, 0, 3)));
    assert!(!range.satisfies((0, 0, 4)));
}

// =========================================================================
// Exact (=)
// =========================================================================

#[test]
fn test_exact_match() {
    let range = VersionRange::Exact((1, 2, 3));
    assert!(range.satisfies((1, 2, 3)));
}

#[test]
fn test_exact_rejects_higher() {
    let range = VersionRange::Exact((1, 2, 3));
    assert!(!range.satisfies((1, 2, 4)));
}

#[test]
fn test_exact_rejects_lower() {
    let range = VersionRange::Exact((1, 2, 3));
    assert!(!range.satisfies((1, 2, 2)));
}

// =========================================================================
// AtLeast (>=)
// =========================================================================

#[test]
fn test_at_least_exact() {
    let range = VersionRange::AtLeast((1, 2, 3));
    assert!(range.satisfies((1, 2, 3)));
}

#[test]
fn test_at_least_higher() {
    let range = VersionRange::AtLeast((1, 2, 3));
    assert!(range.satisfies((2, 0, 0)));
    assert!(range.satisfies((1, 3, 0)));
}

#[test]
fn test_at_least_lower() {
    let range = VersionRange::AtLeast((1, 2, 3));
    assert!(!range.satisfies((1, 2, 2)));
}

// =========================================================================
// Display
// =========================================================================

#[test]
fn test_display_compatible() {
    assert_eq!(VersionRange::Compatible((1, 2, 3)).to_string(), "^1.2.3");
}

#[test]
fn test_display_exact() {
    assert_eq!(VersionRange::Exact((1, 2, 3)).to_string(), "=1.2.3");
}

#[test]
fn test_display_at_least() {
    assert_eq!(VersionRange::AtLeast((1, 2, 3)).to_string(), ">=1.2.3");
}

// =========================================================================
// Constraint checking
// =========================================================================

#[test]
fn test_check_empty_constraints() {
    let constraints: Vec<(&str, &str, &str)> = vec![];
    let versions = vec![("a", (1, 0, 0))];
    let violations = check_version_constraints(&constraints, &versions);
    assert!(violations.is_empty());
}

#[test]
fn test_check_satisfied() {
    let constraints = vec![("consumer", "provider", "^1.0.0")];
    let versions = vec![("consumer", (1, 0, 0)), ("provider", (1, 2, 3))];
    let violations = check_version_constraints(&constraints, &versions);
    assert!(violations.is_empty());
}

#[test]
fn test_check_violated() {
    let constraints = vec![("consumer", "provider", "^2.0.0")];
    let versions = vec![("consumer", (1, 0, 0)), ("provider", (1, 9, 0))];
    let violations = check_version_constraints(&constraints, &versions);
    assert_eq!(violations.len(), 1);
    assert_eq!(violations[0].source, "consumer");
    assert_eq!(violations[0].target, "provider");
    assert_eq!(violations[0].actual, (1, 9, 0));
}

#[test]
fn test_check_malformed_skipped() {
    let constraints = vec![("a", "b", "not-a-version")];
    let versions = vec![("a", (1, 0, 0)), ("b", (1, 0, 0))];
    let violations = check_version_constraints(&constraints, &versions);
    // Malformed constraint is skipped, not an error
    assert!(violations.is_empty());
}

#[test]
fn test_check_missing_target_ignored() {
    let constraints = vec![("a", "missing", "^1.0.0")];
    let versions = vec![("a", (1, 0, 0))];
    let violations = check_version_constraints(&constraints, &versions);
    // Missing target is handled by dep resolver, not constraint checker
    assert!(violations.is_empty());
}

#[test]
fn test_violation_display() {
    let v = ConstraintViolation {
        source: "consumer",
        target: "provider",
        required: VersionRange::Compatible((2, 0, 0)),
        actual: (1, 9, 0),
    };
    let msg = v.to_string();
    assert!(msg.contains("consumer"));
    assert!(msg.contains("provider"));
    assert!(msg.contains("^2.0.0"));
    assert!(msg.contains("1.9.0"));
}
