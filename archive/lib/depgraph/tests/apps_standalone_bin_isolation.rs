//! Standalone-bin isolation guard (#769).
//!
//! Every crate under `apps/` is a standalone composition root. The
//! only `apps/*` → `apps/*` edges permitted are the four
//! `reovim-app-launcher` → `reovim-app-{server,tui,cli,web}` edges that
//! the dual-mode launcher uses to pull sibling `lib` targets into an
//! embedded composition. All other `apps/*` → `apps/*` edges are
//! forbidden.
//!
//! Canonical edge list and the feature gates in `apps/reovim/Cargo.toml`
//! that admit each edge:
//!
//! | Edge                                               | Admitting feature |
//! |----------------------------------------------------|-------------------|
//! | `reovim-app-launcher` → `reovim-app-server`        | `embedded`        |
//! | `reovim-app-launcher` → `reovim-app-tui`           | `embedded-tui`    |
//! | `reovim-app-launcher` → `reovim-app-cli`           | `embedded-cli`    |
//! | `reovim-app-launcher` → `reovim-app-web`           | `embedded-web`    |
//!
//! The sibling-specific companion probe `apps_reovim_launcher_scope.rs`
//! enforces the orthogonal rule that every admitted edge be declared
//! `optional = true` and referenced as `dep:<name>` from its feature.
//!
//! Implementation borrows the `cargo_metadata`-based pattern from
//! `core_ext_boundary.rs`:
//!
//! 1. Build a name → `manifest_path` map of every workspace crate that
//!    sits under `apps/`.
//! 2. Walk every dependency (`DependencyKind::Normal`) of each `apps/*`
//!    package and fail on any dep whose target also lives under `apps/`
//!    and is not in the allowlist.
//!
//! Dev-deps are unrestricted — tests may wire sibling fixtures freely.

use {
    cargo_metadata::{DependencyKind, MetadataCommand},
    std::{
        collections::{HashMap, HashSet},
        path::Path,
    },
};

/// Allowed `(origin, target)` edges inside `apps/`. Exactly the four
/// launcher-to-sibling-lib edges that embedded composition requires.
const ALLOWED_EDGES: &[(&str, &str)] = &[
    ("reovim-app-launcher", "reovim-app-server"),
    ("reovim-app-launcher", "reovim-app-tui"),
    ("reovim-app-launcher", "reovim-app-cli"),
    ("reovim-app-launcher", "reovim-app-web"),
];

#[test]
fn no_standalone_bin_depends_on_another() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");
    let workspace_root: &Path = metadata.workspace_root.as_std_path();

    // Collect every workspace crate whose manifest sits under apps/.
    let mut apps_crates: HashMap<String, String> = HashMap::new();
    for pkg in &metadata.packages {
        let manifest = pkg.manifest_path.as_std_path();
        let Ok(rel) = manifest.strip_prefix(workspace_root) else {
            continue;
        };
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        if rel_str.starts_with("apps/") {
            apps_crates.insert(pkg.name.to_string(), rel_str);
        }
    }

    let allowlist: HashSet<(&str, &str)> = ALLOWED_EDGES.iter().copied().collect();

    let mut violations: Vec<String> = Vec::new();

    for pkg in &metadata.packages {
        let Some(origin_rel) = apps_crates.get(pkg.name.as_ref()) else {
            continue;
        };

        for dep in &pkg.dependencies {
            if dep.kind != DependencyKind::Normal {
                continue;
            }

            let Some(target_rel) = apps_crates.get(dep.name.as_str()) else {
                continue;
            };

            let edge = (pkg.name.as_str(), dep.name.as_str());
            if allowlist.contains(&edge) {
                continue;
            }

            violations
                .push(format!("{} ({}) -> {} ({})", pkg.name, origin_rel, dep.name, target_rel));
        }
    }

    assert!(
        violations.is_empty(),
        "apps/* crates depend on each other (only the four launcher→sibling edges are \
         permitted):\n  {}",
        violations.join("\n  "),
    );
}
