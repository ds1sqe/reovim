//! Guards the Plan 17-β.1 post-wiring invariant.
//!
//! Two checks:
//!
//! 1. `tui_depends_on_codec_subsys_and_handler`: `reovim-client-tui`
//!    has both `reovim-client-subsys-codec` and
//!    `reovim-tui-mod-surface-descriptor-cell-grid` in its
//!    production dependency set. Without the first, dispatch has
//!    no registry; without the second, the kind=0x0001 handler is
//!    never registered.
//! 2. `handler_crate_is_workspace_member_and_depends_on_subsys`:
//!    `reovim-tui-mod-surface-descriptor-cell-grid` is a workspace
//!    package and depends on `reovim-client-subsys-codec` (so its
//!    handler implementation can refer to the
//!    `SurfaceDescriptorHandler` trait).

use cargo_metadata::{DependencyKind, MetadataCommand};

fn workspace_package<'a>(
    metadata: &'a cargo_metadata::Metadata,
    name: &str,
) -> Option<&'a cargo_metadata::Package> {
    metadata
        .workspace_packages()
        .into_iter()
        .find(|p| p.name.as_str() == name)
}

fn has_normal_dep(pkg: &cargo_metadata::Package, dep_name: &str) -> bool {
    pkg.dependencies
        .iter()
        .any(|d| d.name == dep_name && matches!(d.kind, DependencyKind::Normal))
}

#[test]
fn tui_depends_on_codec_subsys_and_handler() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");

    let tui = workspace_package(&metadata, "reovim-client-tui")
        .expect("reovim-client-tui is a workspace package");

    assert!(
        has_normal_dep(tui, "reovim-client-subsys-codec"),
        "reovim-client-tui must depend on reovim-client-subsys-codec \
         (dispatch registry trait). See Plan 17-β.1 Phase C.",
    );

    assert!(
        has_normal_dep(tui, "reovim-tui-mod-surface-descriptor-cell-grid"),
        "reovim-client-tui must depend on \
         reovim-tui-mod-surface-descriptor-cell-grid \
         (built-in cell-grid handler). See Plan 17-β.1 Phase C.",
    );
}

#[test]
fn handler_crate_is_workspace_member_and_depends_on_subsys() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");

    let handler = workspace_package(&metadata, "reovim-tui-mod-surface-descriptor-cell-grid")
        .expect(
            "reovim-tui-mod-surface-descriptor-cell-grid is a workspace \
         package (Plan 17-β.1 Phase B)",
        );

    assert!(
        has_normal_dep(handler, "reovim-client-subsys-codec"),
        "reovim-tui-mod-surface-descriptor-cell-grid must depend on \
         reovim-client-subsys-codec for SurfaceDescriptorHandler trait.",
    );
}
