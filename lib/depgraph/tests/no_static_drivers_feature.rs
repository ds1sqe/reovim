//! Ratchet for runtime cdylib loading of server drivers (#769 Phase 4
//! + #774 SP02 broadening).
//!
//! Two assertions:
//!
//! 1. `no_crate_defines_static_drivers_feature` — no `Cargo.toml` in
//!    the workspace defines a `[features]` key named
//!    `static-drivers`. Preserves L1's "no static-driver fallback
//!    feature" invariant from the master plan.
//! 2. `apps_server_does_not_dep_on_migrated_drivers` — `apps/server`
//!    has zero `Normal`-kind dep on any driver crate that has
//!    completed cdylib migration. SP02 seeds this with `net-grpc`;
//!    each subsequent ABI Deferrals sub-plan appends one row.
//!
//! Per master plan §AC12 (post-replan-2): per-dep assertions cover the
//! full migrated set by mission close.

use cargo_metadata::{DependencyKind, MetadataCommand};

const FEATURE: &str = "static-drivers";

/// Driver crates that have completed cdylib migration. `apps/server`
/// must NOT have a Normal-kind dep on any of these.
///
/// Sub-plan provenance:
///
/// - `reovim-driver-net-grpc` — #774 SP02
/// - `reovim-driver-text-buffer` — #774 SP03 (cdylib lands; rlib dep
///   retained on apps/server for `TestBufferManager`. The dep drop and
///   the entry below land in SP04 once `KernelContext::new` retires the
///   `BufferManager` slot. See
///   `tmp/deferral-draft-sp03-bufmgr-dep-drop.md`.)
/// - (future entries: command, text-input, text-syntax, text-session
///   per their respective sub-plans)
const MIGRATED_DRIVERS: &[&str] = &["reovim-driver-net-grpc"];

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

#[test]
fn apps_server_does_not_dep_on_migrated_drivers() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");
    let apps_server = metadata
        .workspace_packages()
        .iter()
        .find(|p| p.name.as_str() == "reovim-app-server")
        .copied()
        .expect("reovim-app-server present in workspace");

    let violations: Vec<&str> = apps_server
        .dependencies
        .iter()
        .filter(|d| d.kind == DependencyKind::Normal && MIGRATED_DRIVERS.contains(&d.name.as_str()))
        .map(|d| d.name.as_str())
        .collect();

    assert!(
        violations.is_empty(),
        "apps/server still has Normal deps on migrated drivers: {violations:?}.\n\
         Each driver in MIGRATED_DRIVERS ships as a cdylib loaded at runtime\n\
         via reovim-subsys-driver-loader; remove the Cargo.toml entry."
    );
}
