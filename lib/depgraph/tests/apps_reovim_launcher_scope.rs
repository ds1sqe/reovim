//! Dep-scope guard for `apps/reovim/` (#769).
//!
//! The `reovim` launcher bin (`apps/reovim/`, package
//! `reovim-app-launcher`) is a subprocess-only passthrough today — it
//! parses a clap dispatch table and calls
//! `std::process::Command::new("reovim-<kind>")`. No embedded mode,
//! no transport plumbing. As a result it MUST declare zero `reovim-*`
//! workspace dependencies.
//!
//! Relax this probe when an in-process (dual-mode) composition root
//! lands, to allow `apps/reovim/` → `apps/{server,tui,cli,web}`
//! library-target edges behind named features.

use {
    cargo_metadata::{DependencyKind, MetadataCommand},
    std::collections::HashSet,
};

const PACKAGE: &str = "reovim-app-launcher";

/// Prefix used to detect any in-workspace `reovim-*` production dep.
const FORBIDDEN_PREFIX: &str = "reovim-";

#[test]
fn apps_reovim_launcher_has_zero_reovim_deps() {
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

    let declared: HashSet<&str> = pkg
        .dependencies
        .iter()
        .filter(|d| d.kind == DependencyKind::Normal)
        .map(|d| d.name.as_str())
        .collect();

    let mut violations: Vec<&str> = declared
        .iter()
        .copied()
        .filter(|name| name.starts_with(FORBIDDEN_PREFIX))
        .collect();
    violations.sort_unstable();

    assert!(
        violations.is_empty(),
        "`{PACKAGE}` declares `reovim-*` production dep(s) — the launcher must be \
         subprocess-only until an in-process composition root lands: {violations:?}",
    );
}
