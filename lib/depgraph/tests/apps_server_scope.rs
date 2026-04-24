//! Dep-scope guard for `apps/server/` (#769).
//!
//! The standalone `reovim-server` bin crate (`apps/server/`, package
//! `reovim-app-server`) is the server-side composition root. Its dep
//! surface MUST include the server core (`reovim-server`,
//! `reovim-kernel`) and at least one `reovim-subsys-*` crate, and MUST
//! NOT include any client-side crate (`reovim-client-*`).
//!
//! The probe ratchets the server/client split open at depgraph level:
//! any drift that pulls a client-side crate into the server bin (or
//! strips the kernel / subsys surface) fails closed here before the
//! build silently accepts it.

use {
    cargo_metadata::{DependencyKind, MetadataCommand},
    std::collections::HashSet,
};

const PACKAGE: &str = "reovim-app-server";

/// Deps that MUST be present on the server bin.
const REQUIRED_DEPS: &[&str] = &["reovim-server", "reovim-kernel"];

/// At least one crate matching this prefix must be a direct dep.
const REQUIRED_PREFIX: &str = "reovim-subsys-";

/// Dep-name prefixes that MUST NOT appear on `reovim-app-server`.
const FORBIDDEN_PREFIXES: &[&str] = &["reovim-client-"];

#[test]
fn apps_server_dep_surface_is_correct() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");

    let pkg = metadata
        .packages
        .iter()
        .find(|p| p.name.as_str() == PACKAGE)
        .unwrap_or_else(|| {
            panic!(
                "package `{PACKAGE}` not found in workspace — did apps/server/Cargo.toml \
                 `name` change, or was apps/server/ deleted?"
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

    let has_required_prefix = declared.iter().any(|name| name.starts_with(REQUIRED_PREFIX));

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
        missing_required.is_empty(),
        "`{PACKAGE}` is missing required production deps: {missing_required:?}",
    );

    assert!(
        has_required_prefix,
        "`{PACKAGE}` must depend on at least one `{REQUIRED_PREFIX}*` subsys crate",
    );

    assert!(
        forbidden.is_empty(),
        "`{PACKAGE}` declares forbidden production deps (server bin must not pull in \
         client-side crates):\n  {}",
        forbidden.join("\n  "),
    );
}
