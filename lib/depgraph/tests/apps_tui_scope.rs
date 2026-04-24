//! Dep-scope guard for `apps/tui/` (#769).
//!
//! The standalone `reovim-tui` bin (`apps/tui/`, package
//! `reovim-app-tui`) is a thin launcher over CLM v7's
//! `ext/client/platforms/tui/`. It MUST pull in exactly
//! `reovim-client-ext-platform-tui` as its only client-side dep, and
//! MUST NOT pull in any server-side crate (`reovim-server`,
//! `reovim-kernel`, `reovim-subsys-*`, `reovim-driver-*`, or
//! `reovim-module-*`).
//!
//! Server-side concerns belong with `apps/server/`; the TUI bin's job
//! is to parse args, initialize logging via the platform crate, and
//! delegate into `reovim_client_ext_platform_tui::run`.

use {
    cargo_metadata::{DependencyKind, MetadataCommand},
    std::collections::HashSet,
};

const PACKAGE: &str = "reovim-app-tui";

const REQUIRED_DEPS: &[&str] = &["reovim-client-ext-platform-tui"];

/// Dep-name prefixes that MUST NOT appear on `reovim-app-tui`.
const FORBIDDEN_PREFIXES: &[&str] = &[
    "reovim-server",
    "reovim-kernel",
    "reovim-subsys-",
    "reovim-driver-",
    "reovim-module-",
];

#[test]
fn apps_tui_dep_surface_is_correct() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");

    let pkg = metadata
        .packages
        .iter()
        .find(|p| p.name.as_str() == PACKAGE)
        .unwrap_or_else(|| {
            panic!(
                "package `{PACKAGE}` not found in workspace — did apps/tui/Cargo.toml \
                 `name` change, or was apps/tui/ deleted?"
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

    let mut forbidden: Vec<String> = Vec::new();
    for dep in &declared {
        for prefix in FORBIDDEN_PREFIXES {
            // `reovim-server` appears as a literal match (no trailing
            // `-`) on its standalone crate name; the prefix check
            // handles both `reovim-server` and `reovim-server-*` shapes.
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
        forbidden.is_empty(),
        "`{PACKAGE}` declares forbidden production deps (TUI bin must not pull in \
         server-side crates):\n  {}",
        forbidden.join("\n  "),
    );
}
