//! `reovim-pkg-runtime-loader` dependency-scope probe (Wave 3a Phase 1).
//!
//! The runtime-loader bridge sits in `lib/` so it is reachable from
//! both `server/lib/subsys/*` and `clients/lib/subsys/*`. To stay
//! reachable, its production deps must remain core-only — no
//! reovim-server, no reovim-client-*, no subsys, no apps/, no ext/.
//!
//! `core_ext_boundary.rs` already forbids ext/ edges from any `lib/*`
//! crate. This probe pins the tighter, crate-specific assertion: the
//! exact production-dep set documented in
//! `~/docs/plans/reovim/771-pkg-manager-wave-3a/01-pkg-runtime-loader.md`,
//! and the negative-space rule that no reovim-server / reovim-client*
//! / subsys edge slips in via a future refactor.

use cargo_metadata::{DependencyKind, MetadataCommand};

const CRATE: &str = "reovim-pkg-runtime-loader";

/// Production deps the plan locks. `reovim-*` deps are checked by
/// crate name (not manifest path) so a future workspace path move
/// cannot create a false negative.
const ALLOWED_PROD_DEPS: &[&str] = &[
    "reovim-pkg-lazyload",
    "reovim-pkg-lockfile",
    "reovim-dylib-loader",
    "thiserror",
    "tracing",
];

/// Production-dep names that would prove a layer violation. Matched
/// as prefixes so any future `reovim-server-foo` / `reovim-client-bar`
/// dep is rejected.
const FORBIDDEN_PROD_PREFIXES: &[&str] = &[
    "reovim-server",
    "reovim-client",
    "reovim-subsys",
    "reovim-ext",
    "reovim-tui-",
];

#[test]
fn pkg_runtime_loader_production_deps_match_locked_set() {
    let metadata = MetadataCommand::new()
        .no_deps()
        .exec()
        .expect("cargo metadata");

    let pkg = metadata
        .packages
        .iter()
        .find(|p| p.name.as_str() == CRATE)
        .unwrap_or_else(|| panic!("{CRATE} missing from workspace metadata"));

    let prod: Vec<&str> = pkg
        .dependencies
        .iter()
        .filter(|d| d.kind == DependencyKind::Normal)
        .map(|d| d.name.as_str())
        .collect();

    let mut sorted = prod.clone();
    sorted.sort_unstable();
    sorted.dedup();

    let mut expected: Vec<&str> = ALLOWED_PROD_DEPS.to_vec();
    expected.sort_unstable();

    assert_eq!(
        sorted, expected,
        "{CRATE} production deps drifted from the Wave 3a Phase 1 lock.\n\
         got      = {sorted:?}\n\
         expected = {expected:?}\n\
         If this is intentional, update the plan + this probe together."
    );
}

#[test]
fn pkg_runtime_loader_has_no_reovim_runtime_or_ext_deps() {
    let metadata = MetadataCommand::new()
        .no_deps()
        .exec()
        .expect("cargo metadata");

    let pkg = metadata
        .packages
        .iter()
        .find(|p| p.name.as_str() == CRATE)
        .unwrap_or_else(|| panic!("{CRATE} missing from workspace metadata"));

    let violations: Vec<&str> = pkg
        .dependencies
        .iter()
        .filter(|d| d.kind == DependencyKind::Normal)
        .map(|d| d.name.as_str())
        .filter(|name| FORBIDDEN_PROD_PREFIXES.iter().any(|p| name.starts_with(p)))
        .collect();

    assert!(
        violations.is_empty(),
        "{CRATE} acquired forbidden production deps: {violations:?}.\n\
         Forbidden prefixes: {FORBIDDEN_PROD_PREFIXES:?}.\n\
         The runtime-loader bridge must remain core-only so both\n\
         server/lib/subsys/* and clients/lib/subsys/* can consume it."
    );
}
