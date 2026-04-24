//! Ratchet guard for retirement of the `static-modules` feature
//! (#769).
//!
//! Asserts that no `Cargo.toml` in the workspace defines a `[features]`
//! key named `static-modules`. While the feature still exists on
//! `apps/server/Cargo.toml`, this probe is `#[ignore]` so it runs only
//! when explicitly requested; the `#[ignore]` attribute is removed as
//! the final step of retirement, flipping the probe into a hard gate.
//!
//! Landing the probe ahead of the retirement keeps that flip to a
//! one-line deletion instead of a new-probe addition.
//!
//! Implementation: iterate every workspace crate's `features` map via
//! `cargo_metadata` and fail on any occurrence of the key.

use cargo_metadata::MetadataCommand;

const FEATURE: &str = "static-modules";

#[test]
#[ignore = "un-ignore once `static-modules` is retired workspace-wide (#769)"]
fn no_crate_defines_static_modules_feature() {
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
        "workspace crates still define the `{FEATURE}` feature:\n  {}",
        violations.join("\n  "),
    );
}
