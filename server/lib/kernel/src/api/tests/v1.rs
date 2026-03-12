use super::*;

#[test]
fn test_version_compatibility() {
    // Same version is compatible
    assert!(is_compatible(Version::new(1, 0, 0), Version::new(1, 0, 0)));
    // Older minor is compatible
    assert!(is_compatible(Version::new(1, 0, 0), Version::new(1, 1, 0)));
    // Newer minor is NOT compatible
    assert!(!is_compatible(Version::new(1, 2, 0), Version::new(1, 1, 0)));
    // Different major is NOT compatible
    assert!(!is_compatible(Version::new(2, 0, 0), Version::new(1, 0, 0)));
}

#[test]
fn test_all_types_accessible_via_v1() {
    // Verify key types are accessible via api::v1
    let _: BufferId;
    let _: Position;
    let _: fn() -> EventBus = EventBus::new;
    let _: fn() -> Runtime = Runtime::new;
}

#[test]
fn test_macros_accessible() {
    // Verify printk macros work via v1
    pr_info!("test message");
    pr_debug!("debug: {}", 42);
}

#[test]
fn test_module_types_accessible() {
    // Verify ModuleId and ModuleError are accessible
    let id = ModuleId::new("test-module");
    assert_eq!(id.as_str(), "test-module");

    let _err: ModuleError = ModuleError::NotFound("test".into());
}

#[test]
fn test_mode_types_accessible() {
    // Verify Mode types are accessible
    let module = ModuleId::new("test");
    let mode_id = ModeId::new(module.clone(), "normal");
    let cmd_id = CommandId::new(module, "test-cmd");

    assert_eq!(mode_id.name(), "normal");
    assert_eq!(cmd_id.name(), "test-cmd");

    let stack = ModeStack::new(mode_id.clone());
    assert_eq!(stack.current(), &mode_id);
}
