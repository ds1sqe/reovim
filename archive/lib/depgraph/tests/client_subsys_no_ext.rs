//! Guard: `clients/lib/subsys/*` crates must not depend on `ext/client/*`
//! crates.
//!
//! Rule: closed-contract subsys crates are `RepoCore`. `RepoCore` must never
//! import from ext/. This probe iterates every crate whose workspace path
//! starts with `clients/lib/subsys/` and asserts that none of its normal
//! `[dependencies]` resolve to a crate whose path starts with `ext/client/`.
//!
//! Activation phase: Phase A (added).
//!
//! Vacuous-pass note: this probe's assertion is unconditional. Vacuous pass
//! is a property of today's input — only one subsys crate (`codec`) exists
//! and it has zero ext/client deps. Do not add an `if input is empty { return
//! }` short-circuit — the probe must fail closed as soon as a non-vacuous
//! input appears.
//!
//! Non-vacuous activation: as each subsys crate is created in Phases B–D,
//! the probe enforces the no-ext rule for the new crate automatically.

use {
    cargo_metadata::{DependencyKind, MetadataCommand},
    std::path::Path,
};

#[test]
fn client_subsys_crates_have_no_ext_client_deps() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");
    let workspace_root: &Path = metadata.workspace_root.as_std_path();

    // Collect (package_name, manifest_path_rel) for subsys crates.
    let subsys_pkgs: Vec<(String, String)> = metadata
        .workspace_packages()
        .into_iter()
        .filter_map(|pkg| {
            let manifest = pkg.manifest_path.as_std_path();
            let rel = manifest.strip_prefix(workspace_root).ok()?;
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            if rel_str.starts_with("clients/lib/subsys/") {
                Some((pkg.name.to_string(), rel_str))
            } else {
                None
            }
        })
        .collect();

    // Collect crate names whose workspace path starts with ext/client/.
    let ext_client_names: std::collections::HashSet<String> = metadata
        .workspace_packages()
        .into_iter()
        .filter_map(|pkg| {
            let manifest = pkg.manifest_path.as_std_path();
            let rel = manifest.strip_prefix(workspace_root).ok()?;
            let rel_str = rel.to_string_lossy().replace('\\', "/");
            if rel_str.starts_with("ext/client/") {
                Some(pkg.name.to_string())
            } else {
                None
            }
        })
        .collect();

    let mut violations: Vec<String> = Vec::new();

    for (subsys_name, _subsys_path) in &subsys_pkgs {
        let Some(pkg) = metadata
            .workspace_packages()
            .into_iter()
            .find(|p| p.name.as_str() == subsys_name.as_str())
        else {
            continue;
        };
        for dep in &pkg.dependencies {
            if dep.kind != DependencyKind::Normal {
                continue;
            }
            if ext_client_names.contains(dep.name.as_str()) {
                violations.push(format!(
                    "{subsys_name} -> {} (ext/client dep forbidden in subsys)",
                    dep.name
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "clients/lib/subsys/* crates must not depend on ext/client/* crates \
         (RepoCore cannot import from ext). Violations:\n  {}",
        violations.join("\n  "),
    );
}
