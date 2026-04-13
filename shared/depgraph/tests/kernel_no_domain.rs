//! Enforces that `reovim-kernel` has no dependency — direct OR
//! TRANSITIVE — on any domain crate beyond the base `reovim-domain`
//! trait crate. This is the compile-time boundary for the three-layer
//! model: kernel provides mechanisms, domains provide policy types.
//!
//! `reovim-domain-text` was removed from the kernel dependency graph
//! as part of the kernel purity audit. All text-domain tests now live
//! in `reovim-domain-text` itself.

use {cargo_metadata::MetadataCommand, std::collections::HashSet};

/// Allowed domain crates: only the base domain trait.
const ALLOWED_DOMAIN_CRATES: &[&str] = &[
    "reovim-domain", // base Domain trait (opaque associated types)
];

#[test]
fn kernel_has_no_transitive_domain_dependency() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");

    let resolve = metadata.resolve.as_ref().expect("resolved dep graph");

    // Find the kernel package node in the resolved graph.
    let kernel_id = metadata
        .packages
        .iter()
        .find(|p| p.name == "reovim-kernel")
        .map(|p| &p.id)
        .expect("reovim-kernel in workspace");

    // BFS the resolved graph from kernel, collecting all reachable
    // package IDs.
    let mut reachable: HashSet<&cargo_metadata::PackageId> = HashSet::new();
    let mut frontier = vec![kernel_id];

    while let Some(id) = frontier.pop() {
        if !reachable.insert(id) {
            continue;
        }
        if let Some(node) = resolve.nodes.iter().find(|n| &n.id == id) {
            for dep in &node.deps {
                frontier.push(&dep.pkg);
            }
        }
    }

    // Check each reachable package: none may start with `reovim-domain-`
    // unless explicitly allowed.
    let domain_reachable: Vec<_> = reachable
        .iter()
        .filter_map(|id| metadata.packages.iter().find(|p| &p.id == *id))
        .filter(|p| p.name.starts_with("reovim-domain-"))
        .filter(|p| !ALLOWED_DOMAIN_CRATES.contains(&p.name.as_str()))
        .map(|p| p.name.as_str())
        .collect();

    assert!(
        domain_reachable.is_empty(),
        "reovim-kernel must not depend transitively on any domain event crate. \
         Allowed: {ALLOWED_DOMAIN_CRATES:?}. Found: {domain_reachable:?}",
    );
}
