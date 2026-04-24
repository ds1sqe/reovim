//! Standalone-bin isolation guard (#769).
//!
//! Every crate under `apps/` is a standalone composition root: no
//! `apps/*` crate may declare a production dependency on another
//! `apps/*` crate. The allowlist is empty.
//!
//! Relax this probe to allow specific
//! `apps/reovim/` → `apps/{server,tui,cli,web}` library-target edges
//! when an in-process launcher lands (as optional deps behind named
//! features).
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

/// Allowed `(origin, target)` edges inside `apps/`. Currently empty.
const ALLOWED_EDGES: &[(&str, &str)] = &[];

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

            violations.push(format!(
                "{} ({}) -> {} ({})",
                pkg.name, origin_rel, dep.name, target_rel,
            ));
        }
    }

    assert!(
        violations.is_empty(),
        "apps/* crates depend on each other (forbidden today; relax this probe when an \
         in-process launcher is wired to allow apps/reovim/ → \
         apps/{{server,tui,cli,web}} library-target edges):\n  {}",
        violations.join("\n  "),
    );
}
