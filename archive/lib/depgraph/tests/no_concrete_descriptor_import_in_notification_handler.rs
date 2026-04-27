//! Guards the Plan 17-β.2a post-refactor invariant.
//!
//! After Plan 17-β.2a, `clients/tui/src/notification_handler.rs` must
//! not import any concrete surface-descriptor types from ext handler
//! crates. The dispatch layer goes through
//! `SurfaceDescriptorHandler::decode_and_apply` and the narrow
//! `SurfaceApplyContext` trait — no concrete `CellGridSurfaceInfo`,
//! no `PixelSurfaceInfo`, no future ext-specific types.
//!
//! Regression of this invariant reopens the policy-leak that 17-β.2a
//! closed: the routing layer would once again have to know which
//! boxed type the handler produces, which is exactly what the
//! `decode_and_apply` method eliminates.

use {
    cargo_metadata::MetadataCommand,
    std::{fs, path::PathBuf},
};

const FORBIDDEN_IMPORTS: &[&str] = &[
    // Any reovim-tui-mod-*-cell-grid type import in notification_handler.rs
    // is a policy-leak regression. The module is explicitly a routing
    // layer; it must not know decoder-side concrete types.
    "reovim_tui_mod_surface_descriptor_cell_grid::CellGridSurfaceInfo",
    "use reovim_tui_mod_surface_descriptor_cell_grid",
];

fn workspace_root() -> PathBuf {
    MetadataCommand::new()
        .exec()
        .expect("cargo metadata")
        .workspace_root
        .into()
}

#[test]
fn notification_handler_does_not_import_concrete_descriptor_types() {
    let root = workspace_root();
    let target = root.join("clients/tui/src/notification_handler.rs");
    let src = fs::read_to_string(&target).expect("notification_handler.rs readable");

    let mut offenders: Vec<&&str> = Vec::new();
    for pat in FORBIDDEN_IMPORTS {
        if src.contains(pat) {
            offenders.push(pat);
        }
    }

    assert!(
        offenders.is_empty(),
        "clients/tui/src/notification_handler.rs imports concrete \
         descriptor type(s) from ext handler crates: {offenders:?}. \
         Plan 17-β.2a moved the decode+apply policy into the handler \
         crate via decode_and_apply; the routing layer must not see \
         concrete descriptor types. Restore the trait-level dispatch.",
    );
}
