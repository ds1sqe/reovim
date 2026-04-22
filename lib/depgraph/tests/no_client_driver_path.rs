//! Guard: `clients/lib/driver` must not appear as a workspace member after
//! Phase F.
//!
//! The #753 Client Foundation chain eliminates the god-crate
//! `clients/lib/driver/` by Phase F. Once Phase F lands, the workspace
//! `Cargo.toml` must not list `clients/lib/driver` as a member. This probe
//! is the programmatic tripwire against resurrection — if the directory or
//! workspace entry reappears, this test fails CI immediately.
//!
//! Activation phase: Phase F landing (when `clients/lib/driver/` is deleted
//! and removed from the workspace). The `#[ignore]` is removed at that point.
//!
//! This probe is `#[ignore]`-gated today (Phase A) because the legacy crate
//! still exists and is a workspace member. Landing the probe now makes the
//! rule visible and auditable from Phase A onward.
//!
//! Unconditional assertion body: the probe's assertion code is always present.
//! Do not add an `if crate_exists { return }` short-circuit — that would
//! invert the semantics (this probe fails when the crate IS present, not when
//! it is absent).

use {cargo_metadata::MetadataCommand, std::path::Path};

#[test]
#[ignore = "activate at Phase F landing when clients/lib/driver is deleted from workspace"]
fn clients_lib_driver_not_a_workspace_member() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");
    let workspace_root: &Path = metadata.workspace_root.as_std_path();

    let offenders: Vec<String> = metadata
        .workspace_packages()
        .into_iter()
        .filter_map(|pkg| {
            let manifest = pkg.manifest_path.as_std_path();
            let rel = manifest.strip_prefix(workspace_root).ok()?;
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            if rel_str.starts_with("clients/lib/driver/")
                || rel_str == "clients/lib/driver/Cargo.toml"
            {
                Some(format!("{} @ {rel_str}", pkg.name))
            } else {
                None
            }
        })
        .collect();

    assert!(
        offenders.is_empty(),
        "clients/lib/driver is still a workspace member after Phase F. \
         This crate must be deleted and removed from workspace Cargo.toml \
         as part of Phase F (#753). Offending packages:\n  {}",
        offenders.join("\n  "),
    );
}
