//! Guards the Plan 17 v3 codec-foundation invariant.
//!
//! Three checks:
//!
//! 1. `codec_foundation_present_and_old_tree_gone`: `cargo_metadata` shows
//!    both new foundation crates exist and every retired render/surface
//!    codec crate is absent.
//! 2. `client_codec_subsys_stays_kernel_free`: the new client-side subsys
//!    crate does not take a direct dep on `reovim-kernel` (server-side
//!    `RepoCore` contract must not contaminate client `RepoCore`).
//! 3. `no_retired_codec_dep_edge`: no workspace package has a dep edge
//!    (normal or dev) to any of the seven retired crates; locks the
//!    deletion against resurrection via copy-paste.

use cargo_metadata::{DependencyKind, MetadataCommand};

const MUST_EXIST: &[&str] = &[
    "reovim-client-subsys-codec",
    "reovim-ext-client-tui-cap-cell",
];

const MUST_NOT_EXIST: &[&str] = &[
    "reovim-subsys-render-codec",
    "reovim-subsys-surface-codec",
    "reovim-render-codec",
    "reovim-surface-codec",
    "reovim-render-codec-tui",
    "reovim-surface-codec-tui",
    "reovim-surface-codec-pixel",
];

/// Direct-dep edges forbidden for a given crate. `reovim-kernel` is a
/// server-side `RepoCore` contract (`Service` marker et al.) and must not
/// be pulled into client `RepoCore`. No legitimate transitive path exists
/// through the stated deps (`parking_lot` + `std`), so a direct-dep
/// check is sufficient.
const FORBIDDEN_DEPS: &[(&str, &str)] =
    &[("reovim-client-subsys-codec", "reovim-kernel")];

#[test]
fn codec_foundation_present_and_old_tree_gone() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");
    let names: std::collections::HashSet<&str> = metadata
        .workspace_packages()
        .into_iter()
        .map(|p| p.name.as_str())
        .collect();

    let missing: Vec<&&str> = MUST_EXIST.iter().filter(|n| !names.contains(**n)).collect();
    let resurrected: Vec<&&str> = MUST_NOT_EXIST
        .iter()
        .filter(|n| names.contains(**n))
        .collect();

    assert!(
        missing.is_empty() && resurrected.is_empty(),
        "codec foundation invariant broken. \
         missing (must exist): {missing:?}; \
         resurrected (must not exist): {resurrected:?}. \
         Plan 17 v3 created the first two and deleted the other seven; \
         restoring any deleted crate reopens the `ClientCore → ExtServer` \
         boundary violation fixed by v3."
    );
}

#[test]
fn client_codec_subsys_stays_kernel_free() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");

    let mut offenders: Vec<String> = Vec::new();

    for (crate_name, forbidden) in FORBIDDEN_DEPS {
        let Some(pkg) = metadata
            .workspace_packages()
            .into_iter()
            .find(|p| p.name.as_str() == *crate_name)
        else {
            // Presence is asserted by the other test; skip quietly here
            // so the two failures don't collide on the same regression.
            continue;
        };
        for dep in &pkg.dependencies {
            if dep.name == *forbidden
                && matches!(
                    dep.kind,
                    DependencyKind::Normal | DependencyKind::Development
                )
            {
                let kind = match dep.kind {
                    DependencyKind::Normal => "normal",
                    DependencyKind::Development => "dev",
                    _ => "other",
                };
                offenders.push(format!("{crate_name} -> {forbidden} ({kind})"));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "forbidden dep edge(s) found: {offenders:?}. \
         `reovim-kernel` is a server-side `RepoCore` contract and must not \
         contaminate client `RepoCore` (Plan 17 v3 §4.1)."
    );
}

#[test]
fn no_retired_codec_dep_edge() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");

    let retired: std::collections::HashSet<&str> = MUST_NOT_EXIST.iter().copied().collect();

    let mut offenders: Vec<String> = Vec::new();

    for pkg in metadata.workspace_packages() {
        for dep in &pkg.dependencies {
            if retired.contains(dep.name.as_str())
                && matches!(
                    dep.kind,
                    DependencyKind::Normal | DependencyKind::Development
                )
            {
                let kind = match dep.kind {
                    DependencyKind::Normal => "normal",
                    DependencyKind::Development => "dev",
                    _ => "other",
                };
                offenders.push(format!("{} -> {} ({kind})", pkg.name, dep.name));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "workspace has a dependency edge to a retired codec crate: \
         {offenders:?}. Plan 17 v3 Phase C deleted these seven crates; \
         add the new crate (Plan 18+) with a fresh name instead of \
         resurrecting the old path."
    );
}
