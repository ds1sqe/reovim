//! Ratchet for runtime cdylib loading of server drivers (#769 Phase 4).
//!
//! Asserts that no `Cargo.toml` in the workspace defines a `[features]`
//! key named `static-drivers` (the analog of `static-modules` retired
//! in Phase 3 by `no_static_modules_feature.rs`).
//!
//! Phase 4 reduced its cdylib migration to scaffolding-only after the
//! round-3 trait FFI-routability audit (see plan 05 §Out of scope and
//! `tmp/4-blocker-text-syntax-no-implementor.md`). All actual driver
//! migrations move to follow-up issue #774. The migrated set this
//! phase is empty, so this probe is currently a feature-flag-absence
//! ratchet preserving L1's "no static-driver fallback feature"
//! invariant from the master plan.
//!
//! When #774 lands a driver migration, the probe broadens by adding
//! per-dep assertions that `apps/server/Cargo.toml` no longer
//! compile-time-deps on the migrated driver crate.
//!
//! Implementation: iterate every workspace crate's `features` map via
//! `cargo_metadata` and fail on any occurrence of the key.

use cargo_metadata::MetadataCommand;

const FEATURE: &str = "static-drivers";

#[test]
fn no_crate_defines_static_drivers_feature() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");

    let mut violations: Vec<String> = Vec::new();
    for pkg in metadata.workspace_packages() {
        if pkg.features.contains_key(FEATURE) {
            violations.push(pkg.name.to_string());
        }
    }
    violations.sort();

    assert!(
        violations.is_empty(),
        "workspace crates define the `{FEATURE}` feature:\n  {}",
        violations.join("\n  "),
    );
}
