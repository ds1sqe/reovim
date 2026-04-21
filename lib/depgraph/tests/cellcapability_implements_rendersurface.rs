//! Guards the Plan 21 / 17-β.2b-impl-a invariant: `CellCapability`
//! implements `RenderSurface`. This is the architectural unlock that
//! enables 17-β.2b-impl-b consumer migration — every
//! `&mut dyn RenderSurface` call-site accepts a `CellCapability` only
//! because this impl exists.
//!
//! Two checks:
//!
//! 1. The impl block is declared in source (grep pattern).
//! 2. The cell capability crate depends on `reovim-client-driver`
//!    (where `RenderSurface` lives).

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
    let impl_file =
        root.join("ext/client/tui/capabilities/cell/src/render_surface_impl.rs");
    let src = fs::read_to_string(&impl_file)
        .expect("render_surface_impl.rs exists — Plan 21 Phase B landed");
    assert!(
        src.contains("impl RenderSurface for CellCapability"),
        "render_surface_impl.rs must contain `impl RenderSurface \
         for CellCapability` — Plan 21 architectural unlock. Found \
         file but the impl declaration is missing.",
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

    let has_driver_dep = pkg.dependencies.iter().any(|d| {
        d.name == "reovim-client-driver"
            && matches!(d.kind, DependencyKind::Normal)
    });

    assert!(
        has_driver_dep,
        "reovim-ext-client-tui-cap-cell must depend on \
         reovim-client-driver to access the RenderSurface trait, \
         Style, Color, and Rect types (Plan 21 §1 architectural \
         decision — ExtClient{{tui}} → RepoCore edge, classifier-allowed).",
    );
}
