//! Core/Ext boundary enforcement (Plan 13 Phase 19).
//!
//! Adding an extension must be a pure addition with zero edits to core.
//! This test keeps the repo honest by forbidding core crates from
//! depending on anything under `ext/`.
//!
//! Rules:
//! - **Repo-core** (`arch/`, `lib/`, `uapi/`,
//!   `server/lib/{kernel,subsys,server}/`, `clients/lib/`) MUST NOT
//!   declare a production (`[dependencies]`) edge into any `ext/` crate.
//! - **Composition roots** (`apps/bin/`, `tools/*`) are exempt — the
//!   application binary and dev tools (benchmarks, perf reports, test
//!   harnesses) legitimately pull in extensions to wire or exercise the
//!   system.
//! - **Client-core** (`clients/<platform>/` where `<platform>` is not
//!   `lib`) MAY depend on its OWN platform's ext tree
//!   (`ext/client/<platform>/*`) but MUST NOT depend on another
//!   platform's ext (e.g., `clients/cli/` may not pull
//!   `ext/client/tui/*`).
//! - `[dev-dependencies]` are unrestricted (tests may use any fixture).
//! - Test fixtures (`**/tests/fixtures/**`) are exempt.

use {
    cargo_metadata::{DependencyKind, MetadataCommand},
    std::{collections::HashMap, path::Path},
};

#[derive(Debug, Clone, PartialEq, Eq)]
enum Location {
    /// Composition roots (`apps/bin/`, `tools/*`) — may depend on anything.
    App,
    /// Repo-core: MUST NOT depend on ext/.
    RepoCore,
    /// `clients/<platform>/`: may depend on its own `ext/client/<platform>/*`.
    ClientCore { platform: String },
    /// `ext/server/*` and `ext/client/lib/`.
    ExtServer,
    /// `ext/client/<platform>/*` (platform-specific client ext).
    ExtClient { platform: String },
    /// Test fixture crate — exempt.
    Fixture,
    /// Crate outside the classified zones (should not exist after Plan 13).
    Other,
}

/// True when `rel` is exactly `tree` or sits inside it (`{tree}/...`).
///
/// `cargo_metadata` gives the crate directory without a trailing slash,
/// so a top-level crate at `arch/Cargo.toml` appears as `"arch"`. Plain
/// `starts_with("arch/")` would miss that case.
fn matches_tree(rel: &str, tree: &str) -> bool {
    rel == tree || rel.starts_with(&format!("{tree}/"))
}

fn classify(rel_path: &str) -> Location {
    let rel = rel_path.trim_start_matches("./").trim_end_matches('/');

    // Fixtures first (more specific than any parent).
    if rel.contains("/tests/fixtures/") || rel.contains("tests/fixtures/") {
        return Location::Fixture;
    }

    // ext/ (most specific).
    if let Some(rest) = rel.strip_prefix("ext/client/") {
        let platform = rest.split('/').next().unwrap_or("").to_string();
        if platform == "lib" {
            // Shared across client platforms — treat as server-like ext
            // (no platform affinity).
            return Location::ExtServer;
        }
        return Location::ExtClient { platform };
    }
    if rel.starts_with("ext/") {
        return Location::ExtServer;
    }

    // clients/lib/ is repo-core (shared client infra).
    if matches_tree(rel, "clients/lib") {
        return Location::RepoCore;
    }

    // clients/<platform>/ is client-core.
    if let Some(rest) = rel.strip_prefix("clients/") {
        let platform = rest.split('/').next().unwrap_or("").to_string();
        if !platform.is_empty() {
            return Location::ClientCore { platform };
        }
    }

    // Composition roots — may depend on anything (including ext/).
    if matches_tree(rel, "apps") || matches_tree(rel, "tools") {
        return Location::App;
    }

    // Repo-core trees.
    const REPO_CORE_TREES: &[&str] = &[
        "arch",
        "lib",
        "uapi",
        "server/lib/kernel",
        "server/lib/subsys",
        "server/lib/server",
    ];
    for tree in REPO_CORE_TREES {
        if matches_tree(rel, tree) {
            return Location::RepoCore;
        }
    }

    Location::Other
}

#[test]
fn no_repo_core_depends_on_ext() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");
    let workspace_root: &Path = metadata.workspace_root.as_std_path();

    // Build package name -> Location map for in-workspace crates.
    let mut locations: HashMap<String, Location> = HashMap::new();
    for pkg in &metadata.packages {
        let manifest = pkg.manifest_path.as_std_path();
        let Ok(rel) = manifest.strip_prefix(workspace_root) else {
            continue;
        };
        // Strip trailing Cargo.toml to get the crate directory.
        let dir = rel.parent().expect("manifest has parent");
        let rel_str = dir.to_string_lossy().replace('\\', "/");
        locations.insert(pkg.name.to_string(), classify(&rel_str));
    }

    let mut violations: Vec<String> = Vec::new();

    for pkg in &metadata.packages {
        let Some(origin) = locations.get(pkg.name.as_ref()) else {
            continue;
        };

        for dep in &pkg.dependencies {
            if dep.kind != DependencyKind::Normal {
                continue;
            }
            let Some(target) = locations.get(dep.name.as_str()) else {
                continue;
            };

            let is_violation = match (origin, target) {
                // Repo-core MUST NOT touch ext.
                (Location::RepoCore, Location::ExtServer) => true,
                (Location::RepoCore, Location::ExtClient { .. }) => true,

                // Client-core may only touch its own platform's ext.
                (Location::ClientCore { platform }, Location::ExtClient { platform: p2 }) => {
                    platform != p2
                }
                // Client-core must NOT touch server-side ext (server drivers,
                // server modules, server domain, or shared client ext libs).
                (Location::ClientCore { .. }, Location::ExtServer) => true,

                // App, Ext*, Fixture, Other have no restriction here.
                _ => false,
            };

            if is_violation {
                violations
                    .push(format!("{} ({:?}) -> {} ({:?})", pkg.name, origin, dep.name, target,));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Core/Ext boundary violations:\n  {}",
        violations.join("\n  "),
    );
}

#[test]
fn every_workspace_crate_is_classified() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");
    let workspace_root: &Path = metadata.workspace_root.as_std_path();

    let mut unclassified: Vec<String> = Vec::new();
    for pkg in &metadata.packages {
        let manifest = pkg.manifest_path.as_std_path();
        let Ok(rel) = manifest.strip_prefix(workspace_root) else {
            continue;
        };
        let dir = rel.parent().expect("manifest has parent");
        let rel_str = dir.to_string_lossy().replace('\\', "/");
        if classify(&rel_str) == Location::Other {
            unclassified.push(format!("{} @ {rel_str}", pkg.name));
        }
    }

    assert!(
        unclassified.is_empty(),
        "Crates outside the layer taxonomy:\n  {}",
        unclassified.join("\n  "),
    );
}
