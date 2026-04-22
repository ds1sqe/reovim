//! Guard: count of `clients/lib/subsys/*` Cargo.toml files is tracked.
//!
//! This probe is a ratchet tripwire against god-crate regrowth. It counts
//! the number of `clients/lib/subsys/*/Cargo.toml` entries reported by
//! `cargo metadata` and asserts the count is at least the expected minimum.
//!
//! Ratchet schedule (from the #753 Client Foundation master plan):
//!
//! - Phase A landing (today): count = 1 (only `codec` exists).
//! - Phase D landing: count must be exactly 7:
//!   `module`, `chrome`, `render`, `codec`, `capability`, `platform`,
//!   `protocol`.
//!
//! The probe currently asserts count >= 1 (Phase A baseline). At Phase D
//! landing, update the assertion to `== 7` and remove the `>= 1` form.
//!
//! Activation phase: Phase A (added). The probe's assertion is unconditional.
//! Vacuous pass is a property of today's input (count = 1). Do not add an
//! `if count == 0 { return }` short-circuit — the probe must fail closed as
//! soon as the count drops below the expected minimum (regrowth guard) or
//! exceeds an unexpected maximum.
//!
//! Non-vacuous ratchet: at Phase D landing, replace the `>=` assert below
//! with `assert_eq!(count, 7, ...)`. No mode flag; rewrite the assertion
//! directly.

use {cargo_metadata::MetadataCommand, std::path::Path};

/// The minimum expected number of clients/lib/subsys/* crates during Phase A.
/// Replace this probe's assert with `assert_eq!(count, 7, ...)` at Phase D
/// landing.
const EXPECTED_MIN_COUNT: usize = 1;

#[test]
fn client_subsys_crate_count_matches_ratchet() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");
    let workspace_root: &Path = metadata.workspace_root.as_std_path();

    let subsys_crates: Vec<String> = metadata
        .workspace_packages()
        .into_iter()
        .filter_map(|pkg| {
            let manifest = pkg.manifest_path.as_std_path();
            let rel = manifest.strip_prefix(workspace_root).ok()?;
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            if rel_str.starts_with("clients/lib/subsys/") {
                Some(pkg.name.to_string())
            } else {
                None
            }
        })
        .collect();

    let count = subsys_crates.len();

    assert!(
        count >= EXPECTED_MIN_COUNT,
        "clients/lib/subsys/* crate count is {count}, expected at least \
         {EXPECTED_MIN_COUNT}. A crate appears to have been removed. \
         Found: {subsys_crates:?}"
    );
}
