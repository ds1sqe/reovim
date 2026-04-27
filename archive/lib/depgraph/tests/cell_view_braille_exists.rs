//! Compile-time symbol smoke test for `BrailleRasterizer`.
//!
//! Plan 02 (`02-braille-rasterizer.md`) adds `BrailleRasterizer` to
//! `reovim-ext-client-tui-cap-cell-view`. The shared
//! `cell-view → reovim-driver-display` dep is already asserted via
//! `cargo_metadata` by `cell_view_half_block_exists.rs`; duplicating
//! that check would be pointless. This probe instead pins the public
//! API surface: if the symbol moves or is renamed, this test fails at
//! compile time.

#[test]
fn braille_rasterizer_exported() {
    #[allow(unused_imports)]
    use reovim_ext_client_tui_cap_cell_view::{BrailleRasterizer, ViewHint, ViewRasterizer};
    let r = BrailleRasterizer::new();
    assert_eq!(ViewRasterizer::hint(&r), ViewHint::Braille);
}
