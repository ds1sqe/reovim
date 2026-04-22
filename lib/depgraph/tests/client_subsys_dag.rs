//! Guard: intra-subsys dependency edges must match the locked DAG.
//!
//! The allowed intra-subsys edges are locked by the #753 Client Foundation
//! master plan (§Architecture intra-subsys DAG, 2026-04-22) after 3-agent
//! review + countdown round 1. This probe enforces that no `clients/lib/subsys/*`
//! crate declares a `[dependencies]` edge on another subsys crate unless that
//! edge appears in the `ALLOWED_EDGES` array below.
//!
//! Allowed edges (locked, 2026-04-22):
//!
//! - module    → capability
//! - module    → platform
//! - module    → protocol
//! - chrome    → capability
//! - chrome    → codec       (LOCKED α: `ProjectionDisplayCache` reads
//!   `DomainProjection` produced by `codec::from_proto`)
//! - render    → chrome
//! - render    → capability
//! - codec     → capability
//! - platform  → capability
//! - protocol  → codec
//!
//! NOTE: `render → codec` was audited in Phase A and the outcome is REMOVE
//! (zero-hop trace; `RenderTarget` has no codec dependency). It is NOT in
//! `ALLOWED_EDGES`. The probe fails closed if a future contributor adds that
//! edge to a subsys crate's Cargo.toml.
//!
//! Activation phase: Phase A (added). Vacuous-pass today — most subsys crates
//! do not yet exist (only `codec` exists, with no intra-subsys deps). The
//! probe's assertion is unconditional; vacuous pass is a property of today's
//! input. Do not add an `if input is empty { return }` short-circuit — the
//! probe must fail closed as soon as a non-vacuous input appears.

use {
    cargo_metadata::{DependencyKind, MetadataCommand},
    std::path::Path,
};

/// Short name → crate name mapping for subsys crates (locked by master plan).
/// These are the names from which the short "module"/"chrome"/... tokens are
/// derived. Any `clients/lib/subsys/<x>/` crate is identified by the `<x>`
/// segment and looked up in this table.
const SUBSYS_CRATE_NAMES: &[(&str, &str)] = &[
    ("module", "reovim-client-subsys-module"),
    ("chrome", "reovim-client-subsys-chrome"),
    ("render", "reovim-client-subsys-render"),
    ("codec", "reovim-client-subsys-codec"),
    ("capability", "reovim-client-subsys-capability"),
    ("platform", "reovim-client-subsys-platform"),
    ("protocol", "reovim-client-subsys-protocol"),
];

/// Allowed directed edges in the intra-subsys DAG.
/// Format: (`from_short_name`, `to_short_name`).
/// These are the ONLY legal `[dependencies]` edges between subsys crates.
const ALLOWED_EDGES: &[(&str, &str)] = &[
    ("module", "capability"),
    ("module", "platform"),
    ("module", "protocol"),
    ("chrome", "capability"),
    ("chrome", "codec"),
    ("render", "chrome"),
    ("render", "capability"),
    ("codec", "capability"),
    ("platform", "capability"),
    ("protocol", "codec"),
];

fn short_name(crate_name: &str) -> Option<&'static str> {
    SUBSYS_CRATE_NAMES
        .iter()
        .find_map(|(short, full)| (*full == crate_name).then_some(*short))
}

#[test]
fn intra_subsys_edges_match_locked_dag() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");
    let workspace_root: &Path = metadata.workspace_root.as_std_path();

    // Build set of subsys crate names.
    let subsys_crate_names: std::collections::HashSet<String> = metadata
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

    let mut violations: Vec<String> = Vec::new();

    for pkg in metadata.workspace_packages() {
        if !subsys_crate_names.contains(pkg.name.as_str()) {
            continue;
        }
        let Some(from_short) = short_name(pkg.name.as_str()) else {
            // Subsys crate not yet in the naming table — skip; the crate-count
            // probe will catch the unexpected addition.
            continue;
        };
        for dep in &pkg.dependencies {
            if dep.kind != DependencyKind::Normal {
                continue;
            }
            if !subsys_crate_names.contains(dep.name.as_str()) {
                continue;
            }
            let Some(to_short) = short_name(dep.name.as_str()) else {
                continue;
            };
            if !ALLOWED_EDGES.contains(&(from_short, to_short)) {
                violations.push(format!(
                    "{from_short} ({}) -> {to_short} ({}) is not in the locked intra-subsys DAG",
                    pkg.name, dep.name
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Intra-subsys DAG violation(s) detected. Only the edges listed in \
         ALLOWED_EDGES are permitted between clients/lib/subsys/* crates. \
         Violations:\n  {}\n\
         To add a new edge, update the master plan §Architecture intra-subsys \
         DAG and ALLOWED_EDGES in this probe in the same commit.",
        violations.join("\n  "),
    );
}
