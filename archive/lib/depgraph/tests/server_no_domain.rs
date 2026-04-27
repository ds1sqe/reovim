//! Enforces that reovim-server has no direct dependency on any domain
//! or provider crate. The server is a pure dispatch layer — domain
//! logic belongs in modules, domain types are accessed via driver
//! re-exports.

use cargo_metadata::MetadataCommand;

#[test]
fn server_has_no_direct_domain_deps() {
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

    let mut violations = Vec::new();

    for dep in &server_node.deps {
        let dep_name = metadata
            .packages
            .iter()
            .find(|p| p.id == dep.pkg)
            .map_or("<unknown>", |p| p.name.as_str());

        if dep_name.starts_with("reovim-domain-") || dep_name.starts_with("reovim-provider-") {
            violations.push(dep_name.to_string());
        }
    }

    assert!(
        violations.is_empty(),
        "reovim-server must not depend directly on domain or provider crates. \
         Domain types should be accessed via driver re-exports. \
         Found: {violations:?}",
    );
}
