//! Guard: `apps/bin/reovim` client-side deps must be ONLY
//! `reovim-client-ext-platform-*` crates.
//!
//! After Phase F the binary's client-side dependency surface is restricted to
//! exactly one dep per supported platform: `reovim-client-ext-platform-<p>`.
//! No `reovim-client-subsys-*` deps, no `reovim-client-ext-driver-*`,
//! no `reovim-client-ext-module-*`, no `reovim-client-ext-capabilities-*`.
//!
//! Activation phase: Phase A (probe landed). Non-vacuous enforcement begins
//! at Phase F landing when the binary is rewritten to dispatch directly to
//! `ext_client_platform_<p>::run(args)`.
//!
//! Vacuous-pass note: this probe's assertion is unconditional. Vacuous pass
//! is a property of today's input — `apps/bin/reovim/Cargo.toml` currently
//! has no `reovim-client-ext-platform-*` deps (those crates don't exist yet)
//! and the forbidden dep-name patterns do not match any of the currently
//! allowed client deps. The probe will fail closed automatically once
//! a forbidden dep is introduced. Do not add an `if input is empty { return
//! }` short-circuit.

use {
    cargo_metadata::{DependencyKind, MetadataCommand},
    std::path::Path,
};

/// Forbidden crate-name prefixes for client-side deps of apps/bin/reovim.
/// Any dep whose name matches one of these prefixes is a violation.
const FORBIDDEN_PREFIXES: &[&str] = &[
    "reovim-client-subsys-",
    "reovim-client-ext-driver-",
    "reovim-client-ext-module-",
    "reovim-client-ext-capabilities-",
];

#[test]
fn apps_bin_has_no_forbidden_client_side_deps() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");
    let workspace_root: &Path = metadata.workspace_root.as_std_path();

    // Find the apps/bin package (crate name: "reovim").
    let bin_pkg = metadata.workspace_packages().into_iter().find(|pkg| {
        let manifest = pkg.manifest_path.as_std_path();
        let Ok(rel) = manifest.strip_prefix(workspace_root) else {
            return false;
        };
        let rel_str = rel.to_string_lossy().replace('\\', "/");
        rel_str.starts_with("apps/bin/")
    });

    let bin_pkg =
        bin_pkg.expect("apps/bin/reovim package missing from workspace — workspace layout drifted");

    let mut violations: Vec<String> = Vec::new();

    for dep in &bin_pkg.dependencies {
        if dep.kind != DependencyKind::Normal {
            continue;
        }
        for prefix in FORBIDDEN_PREFIXES {
            if dep.name.starts_with(prefix) {
                violations.push(format!(
                    "apps/bin/reovim -> {} ({prefix}* prefix is forbidden; \
                     bin may only depend on reovim-client-ext-platform-* crates)",
                    dep.name
                ));
            }
        }
    }

    assert!(
        violations.is_empty(),
        "apps/bin/reovim has forbidden client-side dep(s). After Phase F the \
         binary's only client-side deps are reovim-client-ext-platform-* \
         (one per supported platform). Violations:\n  {}",
        violations.join("\n  "),
    );
}
