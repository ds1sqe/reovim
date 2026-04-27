//! Dep-scope guard for `apps/web/` (#769 / #753 Flight 76).
//!
//! The standalone `reovim-web` bin (`apps/web/`, package
//! `reovim-app-web`) is a thin launcher over CLM v7's
//! `ext/client/platforms/web/`. It MUST pull in exactly
//! `reovim-client-ext-platform-web` as its only client-side dep, and
//! MUST NOT pull in any server-side crate (`reovim-server`,
//! `reovim-kernel`, `reovim-subsys-*`, `reovim-driver-*`, or
//! `reovim-module-*`).
//!
//! Flight 76 (Plan 03) replaced the Flight-pre-76 scaffold-only
//! policy with the platform delegation mirroring `apps/tui`.

use {
    cargo_metadata::{DependencyKind, MetadataCommand},
    std::collections::HashSet,
};

const PACKAGE: &str = "reovim-app-web";

const REQUIRED_DEPS: &[&str] = &["reovim-client-ext-platform-web"];

/// Dep-name prefixes that MUST NOT appear on `reovim-app-web`.
const FORBIDDEN_PREFIXES: &[&str] = &[
    "reovim-server",
    "reovim-kernel",
    "reovim-subsys-",
    "reovim-driver-",
    "reovim-module-",
];

#[test]
fn apps_web_dep_surface_is_correct() {
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

    let mut missing_required: Vec<&str> = REQUIRED_DEPS
        .iter()
        .copied()
        .filter(|name| !declared.contains(name))
        .collect();
    missing_required.sort_unstable();
    assert!(
        missing_required.is_empty(),
        "`{PACKAGE}` is missing required platform deps: {missing_required:?}"
    );

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
        "`{PACKAGE}` declares forbidden deps: {}",
        forbidden.join(", ")
    );
}
