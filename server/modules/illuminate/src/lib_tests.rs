use reovim_kernel::api::v1::Module;

use super::*;

#[test]
fn test_module_id() {
    let module = IlluminateModule::new();
    assert_eq!(module.id().as_str(), "illuminate");
}

#[test]
fn test_module_name() {
    let module = IlluminateModule::new();
    assert_eq!(module.name(), "Illuminate");
}

#[test]
fn test_module_version() {
    let module = IlluminateModule::new();
    assert_eq!(module.version(), Version::new(0, 1, 0));
}

#[test]
fn test_module_extension_kinds() {
    let module = IlluminateModule::new();
    assert_eq!(module.extension_kinds(), &["illuminate"]);
}

#[test]
fn test_module_default() {
    let module = IlluminateModule;
    assert_eq!(module.id().as_str(), "illuminate");
}

#[test]
fn test_module_exit() {
    let mut module = IlluminateModule::new();
    assert!(module.exit().is_ok());
}
