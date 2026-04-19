//! Enforces that `reovim-server` keeps zero production `reovim-driver-*`
//! dependencies. Dev-only driver usage is already gone; this guard focuses on the
//! manifest seam that Plan 10 Phase 2A must keep driver-free.

use cargo_metadata::{DependencyKind, MetadataCommand};

#[test]
fn server_has_zero_production_driver_dependencies() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");
    let server = metadata
        .packages
        .iter()
        .find(|package| package.name == "reovim-server")
        .expect("reovim-server in workspace");

    let violations: Vec<&str> = server
        .dependencies
        .iter()
        .filter(|dependency| dependency.kind == DependencyKind::Normal)
        .map(|dependency| dependency.name.as_str())
        .filter(|name| name.starts_with("reovim-driver-"))
        .collect();

    assert!(
        violations.is_empty(),
        "reovim-server must not declare production driver dependencies. Found: {violations:?}",
    );
}
