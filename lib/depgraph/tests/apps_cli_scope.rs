//! Dep-scope guard for `apps/cli/` (#769).
//!
//! The standalone `reovim-cli` bin (`apps/cli/`, package
//! `reovim-app-cli`) is a thin gRPC-client-only composition root. It
//! MUST pull in `reovim-client-cli` and `reovim-protocol`, and MUST
//! NOT pull in server-side crates (`reovim-server`, `reovim-kernel`).
//!
//! The CLI talks to a running server over the wire; bundling server
//! code into the CLI bin is a layering violation the probe fails on.

use {
    cargo_metadata::{DependencyKind, MetadataCommand},
    std::collections::HashSet,
};

const PACKAGE: &str = "reovim-app-cli";

const REQUIRED_DEPS: &[&str] = &["reovim-client-cli", "reovim-protocol"];

/// Dep names (exact match) that MUST NOT appear on `reovim-app-cli`.
const FORBIDDEN_DEPS: &[&str] = &["reovim-server", "reovim-kernel"];

#[test]
fn apps_cli_dep_surface_is_correct() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");

    let pkg = metadata
        .packages
        .iter()
        .find(|p| p.name.as_str() == PACKAGE)
        .unwrap_or_else(|| {
            panic!(
                "package `{PACKAGE}` not found in workspace — did apps/cli/Cargo.toml \
                 `name` change, or was apps/cli/ deleted?"
            )
        });

    let declared: HashSet<&str> = pkg
        .dependencies
        .iter()
        .filter(|d| d.kind == DependencyKind::Normal)
        .map(|d| d.name.as_str())
        .collect();

    let mut missing_required: Vec<&str> = REQUIRED_DEPS
        .iter()
        .copied()
        .filter(|name| !declared.contains(name))
        .collect();
    missing_required.sort_unstable();

    let mut forbidden: Vec<&str> = FORBIDDEN_DEPS
        .iter()
        .copied()
        .filter(|name| declared.contains(name))
        .collect();
    forbidden.sort_unstable();

    assert!(
        missing_required.is_empty(),
        "`{PACKAGE}` is missing required production deps: {missing_required:?}",
    );

    assert!(
        forbidden.is_empty(),
        "`{PACKAGE}` declares forbidden production deps (CLI bin must talk to the server \
         over the wire, not bundle it): {forbidden:?}",
    );
}
