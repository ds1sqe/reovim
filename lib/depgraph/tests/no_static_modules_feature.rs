//! Ratchet guard for retirement of the `static-modules` feature
//! (#769).
//!
//! Asserts that no `Cargo.toml` in the workspace defines a `[features]`
//! key named `static-modules`.
//!
//! Implementation: iterate every workspace crate's `features` map via
//! `cargo_metadata` and fail on any occurrence of the key.

use cargo_metadata::MetadataCommand;

const FEATURE: &str = "static-modules";

#[test]
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
