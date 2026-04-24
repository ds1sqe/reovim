//! Locks the shape of `reovim-domain-mesh` and `reovim-provider-mesh`
//! (#753 Flight 77).
//!
//! This probe is **shape-lock only**. The zero-edit-for-new-domain
//! invariant (no kernel / subsys / render-pipeline edit) is enforced
//! by the pre-existing prefix-match probes `kernel_no_domain`,
//! `server_no_domain`, and `subsys_no_domain`, which already cover
//! any `reovim-domain-*` and `reovim-provider-*` crate — including
//! the two introduced by this flight.

use cargo_metadata::{DependencyKind, MetadataCommand};
use reovim_domain_mesh::{Mesh, MeshContent, MeshEdit, MeshPosition, Vertex};

const DOMAIN_CRATE: &str = "reovim-domain-mesh";
const PROVIDER_CRATE: &str = "reovim-provider-mesh";

fn workspace_pkg<'a>(
    metadata: &'a cargo_metadata::Metadata,
    name: &str,
) -> &'a cargo_metadata::Package {
    metadata
        .workspace_packages()
        .into_iter()
        .find(|p| p.name.as_str() == name)
        .unwrap_or_else(|| panic!("{name} must be a workspace member"))
}

fn normal_deps(pkg: &cargo_metadata::Package) -> Vec<&str> {
    pkg.dependencies
        .iter()
        .filter(|d| d.kind == DependencyKind::Normal)
        .map(|d| d.name.as_str())
        .collect()
}

#[test]
fn domain_mesh_crate_is_workspace_member() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");
    let _ = workspace_pkg(&metadata, DOMAIN_CRATE);
}

#[test]
fn provider_mesh_crate_is_workspace_member() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");
    let _ = workspace_pkg(&metadata, PROVIDER_CRATE);
}

#[test]
fn domain_mesh_depends_only_on_reovim_domain() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");
    let pkg = workspace_pkg(&metadata, DOMAIN_CRATE);
    let deps = normal_deps(pkg);
    assert_eq!(
        deps,
        vec!["reovim-domain"],
        "{DOMAIN_CRATE} normal deps must be exactly [reovim-domain]; got {deps:?}"
    );
}

#[test]
fn provider_mesh_depends_only_on_reovim_domain_mesh() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");
    let pkg = workspace_pkg(&metadata, PROVIDER_CRATE);
    let deps = normal_deps(pkg);
    assert_eq!(
        deps,
        vec!["reovim-domain-mesh"],
        "{PROVIDER_CRATE} normal deps must be exactly [reovim-domain-mesh]; got {deps:?}"
    );
}

const fn assert_domain<T: reovim_domain::Domain>() {}

const fn take_position(p: MeshPosition) -> <Mesh as reovim_domain::Domain>::Position {
    p
}

#[allow(clippy::missing_const_for_fn)] // Vec<Vertex> Drop blocks const in stable; lint is a false positive
fn take_edit(e: MeshEdit) -> <Mesh as reovim_domain::Domain>::Edit {
    e
}

#[allow(clippy::missing_const_for_fn)] // Vec<Vertex> Drop blocks const in stable; lint is a false positive
fn take_content(c: MeshContent) -> <Mesh as reovim_domain::Domain>::Content {
    c
}

#[test]
fn mesh_domain_exports_public_symbols() {
    // Compile-time smoke: any rename of the public surface breaks
    // this test at build time.
    assert_domain::<Mesh>();
    take_position(MeshPosition { vertex: 0 });
    take_edit(MeshEdit::Replace(Vec::new()));
    take_content(MeshContent::new());
    let _ = Vertex {
        x: 0.0,
        y: 0.0,
        z: 0.0,
    };
}
