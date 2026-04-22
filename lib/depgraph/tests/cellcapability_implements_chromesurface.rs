//! Locks the invariant that `CellCapability` implements `ChromeSurface`.
//!
//! Two checks:
//!
//! 1. The `impl ChromeSurface for CellCapability` block is declared in
//!    source (grep pattern over `render_surface_impl.rs`).
//! 2. The cell capability crate depends on `reovim-client-driver` where
//!    `ChromeSurface` lives. The dep direction is `ExtClient{tui}` →
//!    `RepoCore` (allowed by `core_ext_boundary.rs`).

use cargo_metadata::{DependencyKind, MetadataCommand};
use std::{fs, path::PathBuf};

fn workspace_root() -> PathBuf {
    MetadataCommand::new()
        .exec()
        .expect("cargo metadata")
        .workspace_root
        .into()
}

#[test]
fn cell_capability_crate_declares_the_impl() {
    let root = workspace_root();
    // The impl lives in a dedicated submodule.
    let impl_file = root.join("ext/client/tui/capabilities/cell/src/render_surface_impl.rs");
    let src = fs::read_to_string(&impl_file).expect("render_surface_impl.rs exists");
    assert!(
        src.contains("impl ChromeSurface for CellCapability"),
        "render_surface_impl.rs must contain `impl ChromeSurface \
         for CellCapability`. Found file but the impl declaration is \
         missing or uses a stale trait name.",
    );
}

#[test]
fn cell_capability_crate_depends_on_client_driver() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");
    let pkg = metadata
        .workspace_packages()
        .into_iter()
        .find(|p| p.name.as_str() == "reovim-ext-client-tui-cap-cell")
        .expect("cell capability crate is a workspace member");

    let has_driver_dep = pkg
        .dependencies
        .iter()
        .any(|d| d.name == "reovim-client-driver" && matches!(d.kind, DependencyKind::Normal));

    assert!(
        has_driver_dep,
        "reovim-ext-client-tui-cap-cell must depend on reovim-client-driver \
         to access the ChromeSurface trait, Style, Color, and Rect types. \
         Edge direction: ExtClient{{tui}} → RepoCore (classifier-allowed).",
    );
}
