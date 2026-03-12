use {super::*, reovim_kernel::api::v1::Module};

#[test]
fn test_module_id() {
    let module = GitSignsModule::new();
    assert_eq!(module.id().as_str(), "git-signs");
}

#[test]
fn test_module_name() {
    let module = GitSignsModule::new();
    assert_eq!(module.name(), "Git Signs");
}

#[test]
fn test_module_version() {
    let module = GitSignsModule::new();
    let v = module.version();
    assert_eq!(v.major, 0);
    assert_eq!(v.minor, 1);
    assert_eq!(v.patch, 0);
}

#[test]
fn test_module_default() {
    let m1 = GitSignsModule::new();
    let m2 = GitSignsModule;
    assert_eq!(m1.id(), m2.id());
}

#[test]
fn test_exit_succeeds() {
    let mut module = GitSignsModule::new();
    assert!(module.exit().is_ok());
}
