//! Enforces that reovim-server does not acquire NEW driver dependencies
//! beyond the 6 documented domain-locked deps. Any new driver dep is
//! a regression that must be justified and added to the exclusion list.

use cargo_metadata::MetadataCommand;

/// Known driver deps that are documented and justified.
/// Each requires `SessionRuntime`, domain types, or bridge functions
/// that cannot be abstracted to subsys without a session facade.
// TODO: create tracking issue for Tier 2 server decoupling (session facade)
const KNOWN_DRIVER_DEPS: &[&str] = &[
    "reovim-driver-text-buffer",
    "reovim-driver-codec",
    "reovim-driver-command",
    "reovim-driver-text-input",
    "reovim-driver-text-session",
    "reovim-driver-text-syntax",
];

#[test]
fn server_has_no_unexpected_driver_deps() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");
    let resolve = metadata.resolve.as_ref().expect("resolved dep graph");

    let server_id = metadata
        .packages
        .iter()
        .find(|p| p.name == "reovim-server")
        .map(|p| &p.id)
        .expect("reovim-server in workspace");

    let server_node = resolve
        .nodes
        .iter()
        .find(|n| n.id == *server_id)
        .expect("reovim-server in resolve graph");

    let mut unexpected = Vec::new();

    for dep in &server_node.deps {
        let dep_name = metadata
            .packages
            .iter()
            .find(|p| p.id == dep.pkg)
            .map_or("<unknown>", |p| p.name.as_str());

        if dep_name.starts_with("reovim-driver-") && !KNOWN_DRIVER_DEPS.contains(&dep_name) {
            unexpected.push(dep_name.to_string());
        }
    }

    assert!(
        unexpected.is_empty(),
        "reovim-server has unexpected driver dependencies. \
         Known deps: {KNOWN_DRIVER_DEPS:?}. \
         Unexpected: {unexpected:?}. \
         Add to KNOWN_DRIVER_DEPS with justification if intentional.",
    );
}
