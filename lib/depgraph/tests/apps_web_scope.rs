//! Dep-scope guard for `apps/web/` (#769).
//!
//! The standalone `reovim-web` bin (`apps/web/`, package
//! `reovim-app-web`) is a scaffold — the SSR runtime is not yet
//! implemented. Until it is, the web bin MUST NOT pull in any
//! `reovim-client-*` or `reovim-server` crate.
//!
//! Relax this probe when SSR wiring lands; until then the scaffold
//! returns `NotImplemented` and carries zero composition-weight.

use {
    cargo_metadata::{DependencyKind, MetadataCommand},
    std::collections::HashSet,
};

const PACKAGE: &str = "reovim-app-web";

/// Dep-name prefixes that MUST NOT appear on `reovim-app-web`.
const FORBIDDEN_PREFIXES: &[&str] = &["reovim-client-", "reovim-server"];

#[test]
fn apps_web_dep_surface_is_scaffold_only() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");

    let pkg = metadata
        .packages
        .iter()
        .find(|p| p.name.as_str() == PACKAGE)
        .unwrap_or_else(|| {
            panic!(
                "package `{PACKAGE}` not found in workspace — did apps/web/Cargo.toml \
                 `name` change, or was apps/web/ deleted?"
            )
        });

    let declared: HashSet<&str> = pkg
        .dependencies
        .iter()
        .filter(|d| d.kind == DependencyKind::Normal)
        .map(|d| d.name.as_str())
        .collect();

    let mut forbidden: Vec<String> = Vec::new();
    for dep in &declared {
        for prefix in FORBIDDEN_PREFIXES {
            if dep.starts_with(prefix) {
                forbidden.push(format!("{dep} (forbidden prefix `{prefix}`)"));
            }
        }
    }
    forbidden.sort();

    assert!(
        forbidden.is_empty(),
        "`{PACKAGE}` declares forbidden production deps (scaffold-only crate; SSR runtime \
         is not yet implemented):\n  {}",
        forbidden.join("\n  "),
    );
}
