//! Guard: count of `clients/lib/subsys/*` Cargo.toml files is tracked.
//!
//! This probe is a ratchet tripwire against god-crate regrowth. It counts
//! the number of `clients/lib/subsys/*/Cargo.toml` entries reported by
//! `cargo metadata` and asserts the count equals the expected exact value.
//!
//! Ratchet schedule (from the #753 Client Foundation master plan):
//!
//! - Phase A landing (2026-04-22): count = 1 (only `codec` exists).
//! - Phase C landing (2026-04-22): count = 4 (`codec`, `module`, `chrome`,
//!   `render`). Updated from 1 → 4 at Phase C landing.
//! - Phase D landing (2026-04-23): count = 7 (`codec`, `module`, `chrome`,
//!   `render`, `capability`, `platform`, `protocol`).
//! - #769 Phase 0 landing (2026-04-23): count = 8 (adds `driver-loader`
//!   for the runtime-loaded driver ABI).
//!
//! Activation phase: Phase A (added). The probe's assertion is unconditional.
//! Do not add an `if count == 0 { return }` short-circuit — the probe must
//! fail closed as soon as the count diverges from the expected value (catches
//! both regrowth and unexpected additions).

use {cargo_metadata::MetadataCommand, std::path::Path};

/// The exact expected number of clients/lib/subsys/* crates after #769 Phase 0.
const EXPECTED_COUNT: usize = 8;

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

    assert_eq!(
        count, EXPECTED_COUNT,
        "clients/lib/subsys/* crate count is {count}, expected exactly \
         {EXPECTED_COUNT} (`codec`, `module`, `chrome`, `render`, `capability`, \
         `platform`, `protocol`, `driver-loader`). A crate was added or \
         removed unexpectedly. Found: {subsys_crates:?}"
    );
}
