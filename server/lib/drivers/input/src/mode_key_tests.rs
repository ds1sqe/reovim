use {crate::ModeProviderKey, reovim_kernel::api::v1::ServiceKey};

#[test]
fn test_service_key_impl() {
    assert_eq!(ModeProviderKey::service_name(), "Mode");
}

#[test]
fn test_equality() {
    assert_eq!(ModeProviderKey::Entry, ModeProviderKey::Entry);
}

#[test]
#[cfg_attr(coverage_nightly, coverage(off))]
fn test_debug() {
    assert_eq!(format!("{:?}", ModeProviderKey::Entry), "Entry");
}
