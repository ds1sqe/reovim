//! Guard: forbidden cross-category dependency edges in `ext/client/`.
//!
//! The #753 Client Foundation master plan (§Architecture) defines which
//! inter-category dep directions are allowed and which are forbidden.
//!
//! FORBIDDEN edges (this probe asserts their absence):
//!
//! - platforms↛driver      (platforms are substrate; drivers bind to them)
//! - platforms↛module      (platforms do not hold a module list)
//! - platforms↛capabilities (capabilities are consumed by drivers/modules)
//! - capabilities↛driver   (capabilities are pure impl; nothing ext below)
//! - capabilities↛module   (same)
//! - capabilities↛platforms (same)
//! - driver↛module         (drivers are contract impls; modules use them)
//!
//! EXPLICITLY ALLOWED and NOT asserted here:
//!
//! - module→driver — modules MAY depend on drivers (e.g., a module consuming
//!   a concrete render driver). The master plan allows this edge. This probe
//!   MUST NOT assert module↛driver to avoid blocking that legitimate use.
//!
//! Activation phase: Phase A (added). Vacuous-pass today — the new flat
//! category directories do not yet exist (only the legacy `tui/` sub-tree
//! is present). The probe's assertion is unconditional; vacuous pass is a
//! property of today's input. Do not add an `if input is empty { return }`
//! short-circuit — the probe must fail closed as soon as crates in these
//! categories are created.

use {
    cargo_metadata::{DependencyKind, MetadataCommand},
    std::path::Path,
};

/// Forbidden edge pairs: (`from_category`, `to_category`).
///
/// NOTE: `("module", "driver")` is intentionally ABSENT — that edge is allowed
/// per the master plan (modules MAY depend on drivers). Do not add it here.
const FORBIDDEN_EDGES: &[(&str, &str)] = &[
    ("platforms", "driver"),
    ("platforms", "module"),
    ("platforms", "capabilities"),
    ("capabilities", "driver"),
    ("capabilities", "module"),
    ("capabilities", "platforms"),
    ("driver", "module"),
];

/// Returns the ext/client category for a crate at the given workspace-relative
/// path, or `None` if the crate is not in `ext/client/<category>/`.
fn ext_client_category(rel_path: &str) -> Option<&'static str> {
    let rest = rel_path.strip_prefix("ext/client/")?;
    let category = rest.split('/').next()?;
    match category {
        "platforms" => Some("platforms"),
        "driver" => Some("driver"),
        "module" => Some("module"),
        "capabilities" => Some("capabilities"),
        _ => None,
    }
}

#[test]
fn ext_client_category_isolation_no_forbidden_edges() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");
    let workspace_root: &Path = metadata.workspace_root.as_std_path();

    // Build (crate_name → category) for ext/client crates.
    let mut category_map: std::collections::HashMap<String, &'static str> =
        std::collections::HashMap::new();
    for pkg in metadata.workspace_packages() {
        let manifest = pkg.manifest_path.as_std_path();
        let Ok(rel) = manifest.strip_prefix(workspace_root) else {
            continue;
        };
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        if let Some(cat) = ext_client_category(&rel_str) {
            category_map.insert(pkg.name.to_string(), cat);
        }
    }

    let mut violations: Vec<String> = Vec::new();

    for pkg in metadata.workspace_packages() {
        let Some(&from_cat) = category_map.get(pkg.name.as_str()) else {
            continue;
        };
        for dep in &pkg.dependencies {
            if dep.kind != DependencyKind::Normal {
                continue;
            }
            let Some(&to_cat) = category_map.get(dep.name.as_str()) else {
                continue;
            };
            if FORBIDDEN_EDGES.contains(&(from_cat, to_cat)) {
                violations.push(format!(
                    "{} (ext/client/{from_cat}) -> {} (ext/client/{to_cat}): \
                     {from_cat}↛{to_cat} is a forbidden cross-category edge",
                    pkg.name, dep.name
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "Forbidden ext/client cross-category dependency edge(s) detected.\n\
         Allowed direction summary: module→driver is OK; all other cross-\
         category downward edges to platforms/capabilities are forbidden.\n\
         Violations:\n  {}",
        violations.join("\n  "),
    );
}
