//! Dep-scope guard for `apps/reovim/` (#769).
//!
//! The `reovim` launcher at `apps/reovim/` is a dual-mode composition
//! root. In subprocess mode it spawns sibling bins with zero
//! `reovim-*` workspace deps; in embedded mode it pulls the sibling
//! `lib` targets into the same process behind feature gates.
//!
//! The allowlist below is a closed set of five `reovim-*` crate names.
//! Each allowlisted dep must be declared `optional = true` in
//! `apps/reovim/Cargo.toml` AND referenced as `dep:<name>` from at
//! least one named feature (the feature gate is what admits the dep
//! under cargo resolution). Any other `reovim-*` production dep, or
//! an allowlisted dep not behind a feature gate, is a violation.

use {
    cargo_metadata::{DependencyKind, MetadataCommand},
    std::collections::HashSet,
};

const PACKAGE: &str = "reovim-app-launcher";

/// Prefix used to detect any in-workspace `reovim-*` production dep.
const FORBIDDEN_PREFIX: &str = "reovim-";

/// Closed allowlist of `reovim-*` deps admitted when declared
/// `optional = true` and referenced from a named feature.
const ALLOWLIST: &[&str] = &[
    "reovim-app-server",
    "reovim-app-tui",
    "reovim-app-cli",
    "reovim-app-web",
    "reovim-server",
];

#[test]
fn apps_reovim_launcher_respects_feature_gated_allowlist() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");

    let pkg = metadata
        .packages
        .iter()
        .find(|p| p.name.as_str() == PACKAGE)
        .unwrap_or_else(|| {
            panic!(
                "package `{PACKAGE}` not found in workspace — did apps/reovim/Cargo.toml \
                 `name` change, or was apps/reovim/ deleted?"
            )
        });

    let allowlist: HashSet<&str> = ALLOWLIST.iter().copied().collect();

    // Set of deps referenced as `dep:<name>` from any feature.
    let feature_gated_deps: HashSet<String> = pkg
        .features
        .values()
        .flat_map(|vals| vals.iter())
        .filter_map(|spec| spec.strip_prefix("dep:").map(str::to_owned))
        .collect();

    let mut violations: Vec<String> = Vec::new();

    for dep in &pkg.dependencies {
        if dep.kind != DependencyKind::Normal {
            continue;
        }
        let name = dep.name.as_str();
        if !name.starts_with(FORBIDDEN_PREFIX) {
            continue;
        }

        if !allowlist.contains(name) {
            violations.push(format!("{name}: not in allowlist"));
            continue;
        }

        if !dep.optional {
            violations.push(format!("{name}: allowlisted but not declared optional"));
            continue;
        }

        if !feature_gated_deps.contains(name) {
            violations.push(format!(
                "{name}: allowlisted and optional but not referenced as `dep:{name}` from any feature"
            ));
        }
    }

    violations.sort_unstable();

    assert!(
        violations.is_empty(),
        "`{PACKAGE}` violates the feature-gated allowlist (allowed: {ALLOWLIST:?}; each must be \
         optional=true AND referenced as dep:<name> from a named feature):\n  {}",
        violations.join("\n  "),
    );
}
