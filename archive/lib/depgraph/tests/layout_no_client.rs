//! Enforces that `reovim-subsys-layout` keeps zero direct client, driver, or
//! module manifest dependencies.

use cargo_metadata::MetadataCommand;

#[test]
fn layout_has_no_client_driver_or_module_dependencies() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");
    let layout = metadata
        .packages
        .iter()
        .find(|package| package.name == "reovim-subsys-layout")
        .expect("reovim-subsys-layout in workspace");

    let violations: Vec<String> = layout
        .dependencies
        .iter()
        .filter(|dependency| is_forbidden_dependency(dependency.name.as_str()))
        .map(|dependency| format!("{} ({:?})", dependency.name, dependency.kind))
        .collect();

    assert!(
        violations.is_empty(),
        "reovim-subsys-layout must not declare direct client, driver, or module \
         dependencies. Found: {violations:?}",
    );
}

fn is_forbidden_dependency(name: &str) -> bool {
    name == "reovim-client-model"
        || name.starts_with("reovim-driver-")
        || name.starts_with("reovim-module-")
}
