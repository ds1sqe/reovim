//! Enforces that `reovim-server` has no DIRECT dependency on text-domain
//! crates. Domain-specific logic belongs in modules, not the server.
//!
//! The server reaches domain-text transitively through drivers
//! (`reovim-driver-session`, `reovim-driver-input`), which is acceptable —
//! drivers MAY depend on domain types per the layer model. The guard
//! ensures the server's own `Cargo.toml` stays clean.

use {cargo_metadata::MetadataCommand, std::collections::HashSet};

/// Crates that `reovim-server` MUST NOT depend on directly.
const FORBIDDEN_DIRECT_DEPS: &[&str] = &[
    "reovim-domain-text",
    "reovim-domain-text-events",
    "reovim-provider-text",
];

#[test]
fn server_has_no_direct_domain_text_dependency() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");
    let resolve = metadata.resolve.as_ref().expect("resolved dep graph");

    let server_id = metadata
        .packages
        .iter()
        .find(|p| p.name == "reovim-server")
        .map(|p| &p.id)
        .expect("reovim-server in workspace");

    // Check DIRECT dependencies only (not transitive through drivers).
    let server_node = resolve
        .nodes
        .iter()
        .find(|n| n.id == *server_id)
        .expect("reovim-server in resolve graph");

    // Filter to normal (non-dev, non-build) dependencies only.
    // Dev-dependencies are acceptable — tests may use domain types.
    let direct_dep_names: HashSet<&str> = server_node
        .deps
        .iter()
        .filter(|dep| {
            dep.dep_kinds
                .iter()
                .any(|dk| dk.kind == cargo_metadata::DependencyKind::Normal)
        })
        .filter_map(|dep| {
            metadata
                .packages
                .iter()
                .find(|p| p.id == dep.pkg)
                .map(|p| p.name.as_str())
        })
        .collect();

    let violations: Vec<&&str> = FORBIDDEN_DIRECT_DEPS
        .iter()
        .filter(|name| direct_dep_names.contains(**name))
        .collect();

    assert!(
        violations.is_empty(),
        "reovim-server must not directly depend on text-domain crates. \
         Found: {violations:?}. These belong in modules, not the server.",
    );
}
