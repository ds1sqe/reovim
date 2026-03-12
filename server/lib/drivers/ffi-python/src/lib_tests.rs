use super::*;

#[test]
fn test_module_base_defaults() {
    let module = PyModuleBase::new();
    assert_eq!(module.id().as_str(), "unnamed-module");
    assert_eq!(module.name(), "Unnamed Module");
    assert_eq!(module.version(), (0, 0, 0));
    assert!(!module.supports_hot_reload());
    assert!(module.save_state().is_none());
    assert!(module.dependencies().is_empty());
    assert!(module.optional_dependencies().is_empty());
    // Registration methods default to empty
    assert!(module.commands().is_empty());
    assert!(module.keybindings().is_empty());
    assert!(module.event_handlers().is_empty());
}

#[test]
fn test_module_base_api_version() {
    let module = PyModuleBase::new();
    let api_ver = module.api_version();
    assert_eq!(api_ver.major(), reovim_kernel::api::v1::API_VERSION.major);
    assert_eq!(api_ver.minor(), reovim_kernel::api::v1::API_VERSION.minor);
    assert_eq!(api_ver.patch(), reovim_kernel::api::v1::API_VERSION.patch);
}

#[test]
fn test_module_base_restore_state_is_noop() {
    let mut module = PyModuleBase::new();
    // Should not panic
    module.restore_state(&[1, 2, 3]);
}

#[test]
fn test_module_base_exit_is_noop() {
    let mut module = PyModuleBase::new();
    // Should not panic
    module.exit();
}
