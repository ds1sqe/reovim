//! Locks the `HalfBlock` pipeline dependencies in the cell-view crate.
//!
//! Plan 01 (`01-halfblock-rasterizer.md`) adds:
//!
//! 1. The `BackendRasterOutput` adapter, which requires a **direct**
//!    normal dep from `reovim-ext-client-tui-cap-cell-view` onto
//!    `reovim-driver-display`. This is the first
//!    `capability → driver` edge in the ext/client tree; the probe
//!    makes it explicit and intentional so the edge cannot drift back
//!    to a transitive accident.
//! 2. The `HalfBlockRasterizer` + `BackendRasterOutput` modules must
//!    compile — the probe compiles against these symbols as a smoke
//!    test of the public API surface.
//!
//! The `cargo_metadata` assertion (test #1) is the load-bearing one;
//! the compile-time symbol check (test #2) is a belt-and-braces
//! complement.

use cargo_metadata::{DependencyKind, MetadataCommand};

const CELL_VIEW: &str = "reovim-ext-client-tui-cap-cell-view";
const DRIVER_DISPLAY: &str = "reovim-driver-display";

#[test]
fn cell_view_depends_on_driver_display_for_halfblock_adapter() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");
    let pkg = metadata
        .workspace_packages()
        .into_iter()
        .find(|p| p.name.as_str() == CELL_VIEW)
        .expect("cell-view crate present");
    let has_direct_dep = pkg
        .dependencies
        .iter()
        .any(|d| d.kind == DependencyKind::Normal && d.name == DRIVER_DISPLAY);
    assert!(
        has_direct_dep,
        "{CELL_VIEW} must declare a direct normal dep on \
         {DRIVER_DISPLAY} — `BackendRasterOutput` wraps `RenderBackend` \
         and converts `CellStyle` to display `Style`. A transitive \
         accident is not enough; the edge is deliberate."
    );
}

#[test]
fn halfblock_rasterizer_and_backend_output_exported() {
    // Smoke test: the public API surface for Flight 74 must be
    // constructible from the crate root. Compilation failure here
    // means a symbol moved or was renamed; fix the re-exports in
    // `cell-view/src/lib.rs`.
    #[allow(unused_imports)]
    use reovim_ext_client_tui_cap_cell_view::{
        BackendRasterOutput, HalfBlockRasterizer, ViewHint, ViewRasterizer,
        convert_cell_style_to_display_style,
    };
    let r = HalfBlockRasterizer::new();
    assert_eq!(ViewRasterizer::hint(&r), ViewHint::HalfBlock);
}
