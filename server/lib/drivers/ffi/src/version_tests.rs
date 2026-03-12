use super::*;

#[test]
fn test_abi_version_constants() {
    assert_eq!(ABI_VERSION, Version::new(1, 2, 0));
    assert_eq!(ABI_VERSION_MAJOR, 1);
    assert_eq!(ABI_VERSION_MINOR, 2);
    assert_eq!(ABI_VERSION_PATCH, 0);
}

#[test]
fn test_reovim_abi_version() {
    let v = reovim_abi_version();
    assert_eq!(v, ABI_VERSION);
}

#[test]
fn test_abi_compatibility_same_version() {
    let v = Version::new(1, 0, 0);
    assert!(reovim_abi_is_compatible(v, v));
}

#[test]
fn test_abi_compatibility_older_minor() {
    // Module requires 1.0.0, kernel provides 1.1.0 - compatible
    assert!(reovim_abi_is_compatible(Version::new(1, 0, 0), Version::new(1, 1, 0)));

    // Module requires 1.0.0, kernel provides 1.5.0 - compatible
    assert!(reovim_abi_is_compatible(Version::new(1, 0, 0), Version::new(1, 5, 0)));
}

#[test]
fn test_abi_compatibility_newer_minor_incompatible() {
    // Module requires 1.2.0, kernel provides 1.1.0 - NOT compatible
    assert!(!reovim_abi_is_compatible(Version::new(1, 2, 0), Version::new(1, 1, 0)));
}

#[test]
fn test_abi_compatibility_different_major_incompatible() {
    // Different major versions are never compatible
    assert!(!reovim_abi_is_compatible(Version::new(1, 0, 0), Version::new(2, 0, 0)));
    assert!(!reovim_abi_is_compatible(Version::new(2, 0, 0), Version::new(1, 0, 0)));
}

#[test]
fn test_abi_v1_2_backward_compatible() {
    // Modules compiled against ABI 1.0.0 or 1.1.0 work with new 1.2.0 kernel
    assert!(reovim_abi_is_compatible(Version::new(1, 0, 0), ABI_VERSION));
    assert!(reovim_abi_is_compatible(Version::new(1, 1, 0), ABI_VERSION));
}

#[test]
fn test_abi_v1_2_forward_incompatible() {
    // Modules requiring ABI 1.2.0 do NOT work with old 1.0.0 or 1.1.0 kernel
    assert!(!reovim_abi_is_compatible(ABI_VERSION, Version::new(1, 0, 0)));
    assert!(!reovim_abi_is_compatible(ABI_VERSION, Version::new(1, 1, 0)));
}

#[test]
fn test_abi_compatibility_patch_ignored() {
    // Patch version should be ignored
    assert!(reovim_abi_is_compatible(Version::new(1, 0, 5), Version::new(1, 0, 0)));
    assert!(reovim_abi_is_compatible(Version::new(1, 0, 0), Version::new(1, 0, 10)));
}
