//! Guards the Plan 17-β.2a trait-amendment invariant.
//!
//! After Plan 17-β.2a lands, `reovim-client-subsys-codec` must export:
//!
//! - `SurfaceApplyContext` trait (narrow apply-side hook surface).
//! - `SurfaceDescriptorApplyError` enum (wraps decode + apply
//!   failures).
//! - `SurfaceDescriptorHandler::decode_and_apply` method (so the
//!   routing layer never re-passes a `Box<dyn Any + Send>` back to
//!   the handler that produced it).
//!
//! This probe reads the crate's `lib.rs` and fails if any of the
//! required symbols is missing from re-exports. Treats missing
//! symbols as a hard architectural regression: removing them
//! silently reopens the policy-leak that 17-β.2a was designed to
//! close.

use cargo_metadata::MetadataCommand;
use std::{fs, path::PathBuf};

const REQUIRED_EXPORTS: &[&str] = &[
    "SurfaceApplyContext",
    "SurfaceDescriptorApplyError",
];

fn workspace_root() -> PathBuf {
    MetadataCommand::new()
        .exec()
        .expect("cargo metadata")
        .workspace_root
        .into()
}

#[test]
fn codec_subsys_exports_apply_symbols() {
    let root = workspace_root();
    let lib_rs = root.join("clients/lib/subsys/codec/src/lib.rs");
    let src = fs::read_to_string(&lib_rs).expect("codec subsys lib.rs readable");

    let mut missing: Vec<&&str> = Vec::new();
    for sym in REQUIRED_EXPORTS {
        if !src.contains(sym) {
            missing.push(sym);
        }
    }

    assert!(
        missing.is_empty(),
        "reovim-client-subsys-codec lib.rs missing required Plan 17-β.2a \
         exports: {missing:?}. Removing these symbols silently reopens the \
         Box<dyn Any + Send> policy-leak in the routing layer. Restore the \
         trait + error type before landing.",
    );
}

#[test]
fn decode_and_apply_method_declared() {
    let root = workspace_root();
    let surface_rs = root.join("clients/lib/subsys/codec/src/surface_descriptor.rs");
    let src = fs::read_to_string(&surface_rs).expect("surface_descriptor.rs readable");

    assert!(
        src.contains("fn decode_and_apply"),
        "SurfaceDescriptorHandler trait must declare decode_and_apply \
         (Plan 17-β.2a Phase A). Found no such method in \
         clients/lib/subsys/codec/src/surface_descriptor.rs.",
    );
}
