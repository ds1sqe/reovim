//! Enforces that `reovim-server` has no dependency on text-domain crates
//! — neither normal nor dev. Domain-specific logic belongs in modules,
//! not the server. Test code uses driver re-exports (e.g.,
//! `reovim_driver_text_buffer::Position`) instead of importing domain crates
//! directly.
//!
//! The server reaches domain-text transitively through drivers
//! (`reovim-driver-text-session`, `reovim-driver-text-input`), which is acceptable —
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
fn server_has_no_domain_text_dependency() {
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

    // Check ALL dependency kinds (normal + dev + build).
    // Server tests use driver re-exports, so no domain dev-deps are needed.
    let direct_dep_names: HashSet<&str> = server_node
        .deps
        .iter()
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
        "reovim-server must not depend on text-domain crates (normal or dev). \
         Found: {violations:?}. Use driver re-exports instead.",
    );
}
