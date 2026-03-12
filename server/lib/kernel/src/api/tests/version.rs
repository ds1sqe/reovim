use super::*;

#[test]
fn test_version_new() {
    let v = Version::new(1, 2, 3);
    assert_eq!(v.major, 1);
    assert_eq!(v.minor, 2);
    assert_eq!(v.patch, 3);
}

#[test]
fn test_version_display() {
    let v = Version::new(1, 2, 3);
    assert_eq!(v.to_string(), "1.2.3");
}

#[test]
fn test_api_version_constants() {
    assert_eq!(API_VERSION, Version::new(0, 2, 0));
    assert_eq!(API_VERSION_STR, "0.2.0");
}

#[test]
fn test_is_compatible_same_version() {
    assert!(is_compatible(Version::new(1, 0, 0), Version::new(1, 0, 0)));
}

#[test]
fn test_is_compatible_older_minor() {
    assert!(is_compatible(Version::new(1, 0, 0), Version::new(1, 1, 0)));
    assert!(is_compatible(Version::new(1, 1, 0), Version::new(1, 5, 0)));
}

#[test]
fn test_is_compatible_newer_minor_not_compatible() {
    assert!(!is_compatible(Version::new(1, 2, 0), Version::new(1, 1, 0)));
}

#[test]
fn test_is_compatible_different_major_not_compatible() {
    assert!(!is_compatible(Version::new(2, 0, 0), Version::new(1, 0, 0)));
    assert!(!is_compatible(Version::new(1, 0, 0), Version::new(2, 0, 0)));
}

#[test]
fn test_is_compatible_patch_ignored() {
    assert!(is_compatible(Version::new(1, 0, 0), Version::new(1, 0, 5)));
    assert!(is_compatible(Version::new(1, 0, 10), Version::new(1, 0, 0)));
}

#[test]
fn test_check_api_version_compatible() {
    // Exact match
    assert!(check_api_version(Version::new(0, 2, 0)).is_ok());
    // Older minor versions are compatible
    assert!(check_api_version(Version::new(0, 1, 0)).is_ok());
    assert!(check_api_version(Version::new(0, 0, 0)).is_ok());
}

#[test]
fn test_check_api_version_major_mismatch() {
    // API is 0.2.0, requesting 1.0.0 should fail with major mismatch
    let result = check_api_version(Version::new(1, 0, 0));
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.kind, VersionErrorKind::MajorMismatch);
}

#[test]
fn test_check_api_version_minor_too_new() {
    // API is 0.2.0, requesting 0.5.0 should fail (minor too new)
    let result = check_api_version(Version::new(0, 5, 0));
    assert!(result.is_err());
    let err = result.unwrap_err();
    assert_eq!(err.kind, VersionErrorKind::MinorTooNew);
}

#[test]
fn test_version_error_display() {
    let err = VersionError {
        kind: VersionErrorKind::MajorMismatch,
        required: Version::new(2, 0, 0),
        provided: Version::new(1, 0, 0),
    };
    assert!(err.to_string().contains("major version mismatch"));

    let err = VersionError {
        kind: VersionErrorKind::MinorTooNew,
        required: Version::new(1, 5, 0),
        provided: Version::new(1, 0, 0),
    };
    assert!(err.to_string().contains("minor version too new"));
}

#[test]
fn test_version_ordering() {
    // Patch ordering
    assert!(Version::new(1, 0, 0) < Version::new(1, 0, 1));
    assert!(Version::new(1, 0, 1) < Version::new(1, 0, 2));

    // Minor ordering
    assert!(Version::new(1, 0, 9) < Version::new(1, 1, 0));
    assert!(Version::new(1, 1, 0) < Version::new(1, 2, 0));

    // Major ordering
    assert!(Version::new(1, 9, 9) < Version::new(2, 0, 0));
    assert!(Version::new(2, 0, 0) < Version::new(3, 0, 0));

    // Equal versions
    assert!(Version::new(1, 2, 3) == Version::new(1, 2, 3));
    assert!(Version::new(1, 2, 3) <= Version::new(1, 2, 3));
    assert!(Version::new(1, 2, 3) >= Version::new(1, 2, 3));
}

#[test]
fn test_version_sorting() {
    let mut versions = [
        Version::new(2, 0, 0),
        Version::new(1, 0, 1),
        Version::new(1, 0, 0),
        Version::new(1, 1, 0),
        Version::new(0, 1, 0),
    ];
    versions.sort();

    assert_eq!(versions[0], Version::new(0, 1, 0));
    assert_eq!(versions[1], Version::new(1, 0, 0));
    assert_eq!(versions[2], Version::new(1, 0, 1));
    assert_eq!(versions[3], Version::new(1, 1, 0));
    assert_eq!(versions[4], Version::new(2, 0, 0));
}

#[test]
fn test_version_min_max() {
    let v1 = Version::new(1, 0, 0);
    let v2 = Version::new(2, 0, 0);

    assert_eq!(v1.min(v2), v1);
    assert_eq!(v1.max(v2), v2);
}

#[test]
fn test_version_is_repr_c() {
    // Verify Version has predictable FFI-safe layout
    // 3 * u32 = 12 bytes, aligned to 4 bytes
    assert_eq!(std::mem::size_of::<Version>(), 12);
    assert_eq!(std::mem::align_of::<Version>(), 4);
}
