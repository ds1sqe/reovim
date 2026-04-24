//! Enforces that every `reovim_*` crate referenced by
//! `apps/server/src/bootstrap.rs` is declared as a direct dependency
//! in `apps/server/Cargo.toml`.
//!
//! `apps/server/` is the server-side composition root — it wires
//! concrete server drivers, subsys contracts, and modules into a
//! runnable binary. When `bootstrap.rs` references a crate via a
//! transitive build-graph edge rather than a direct `[dependencies]`
//! entry, the build is fragile: any future trimming of the transitive
//! path breaks the binary silently. This probe catches the divergence
//! at depgraph-check time rather than at downstream build time.
//!
//! Enforcement strategy (no AST parse):
//!
//! 1. Use `cargo_metadata` to list `apps/server`'s declared production
//!    dependencies.
//! 2. Read `apps/server/src/bootstrap.rs` as a string. Existence-check
//!    fails closed if the file is missing — a guard against a
//!    silently-vacuous pass after future relocations.
//! 3. Extract every `reovim_<ident>` crate-path prefix that appears in
//!    a `use` statement or an inline fully-qualified path.
//! 4. For each extracted identifier, canonicalize
//!    `reovim_foo_bar` → `reovim-foo-bar` and assert it is in the
//!    declared dependency set.
//!
//! Baseline sentry: `checked_count >= 6`. The bootstrap import surface
//! includes at minimum `reovim_kernel`, `reovim_server`,
//! `reovim_driver_text_syntax`, `reovim_subsys_session`,
//! `reovim_driver_text_input`, `reovim_driver_command`. A detected
//! count below 6 means the extractor regressed — the probe flags a
//! broken scanner, not a clean import set.

use cargo_metadata::{DependencyKind, MetadataCommand};
use std::{
    // BTreeSet for the extracted-idents set (deterministic debug output for the
    // sentry panic message); HashSet for the Cargo.toml declared-deps set
    // (O(1) membership lookup during the per-ident check).
    collections::{BTreeSet, HashSet},
    fs,
    path::Path,
};

const APPS_SERVER_PACKAGE: &str = "reovim-app-server";
const BOOTSTRAP_REL_PATH: &str = "apps/server/src/bootstrap.rs";
const SENTRY_MIN_COUNT: usize = 6;

/// Extract every `reovim_<snake_ident>` identifier that appears as a
/// crate-path root in the source — i.e., followed by `::`.
///
/// Covers the import forms documented in the upstream probe:
/// - `use reovim_foo_bar::Thing;`
/// - `use reovim_foo_bar::{A, B};`
/// - `reovim_foo_bar::Thing::new(...)` (inline qualified path)
/// - `use reovim_foo_bar::sub::Thing;` (nested)
///
/// The trailing-`::` requirement is load-bearing: a bare identifier
/// like `reovim_version` in a field access (`foo.reovim_version`) or
/// an FFI-symbol mention in a comment is NOT a crate-path root and
/// must not match. Similarly `use reovim_foo as alias;` (bare rename)
/// is rare enough in bootstrap.rs to ignore; we only detect crate
/// roots that resolve a path.
fn extract_reovim_crate_idents(source: &str) -> BTreeSet<String> {
    let mut found: BTreeSet<String> = BTreeSet::new();
    let bytes = source.as_bytes();
    let prefix = b"reovim_";
    let mut i = 0usize;

    while i + prefix.len() <= bytes.len() {
        if &bytes[i..i + prefix.len()] != prefix {
            i += 1;
            continue;
        }

        if i > 0 && is_ident_byte(bytes[i - 1]) {
            i += 1;
            continue;
        }

        let mut end = i + prefix.len();
        while end < bytes.len() && is_ident_byte(bytes[end]) {
            end += 1;
        }

        if end == i + prefix.len() {
            i = end;
            continue;
        }

        if end + 1 < bytes.len() && bytes[end] == b':' && bytes[end + 1] == b':' {
            found.insert(source[i..end].to_string());
        }

        i = end;
    }

    found
}

const fn is_ident_byte(b: u8) -> bool {
    b.is_ascii_alphanumeric() || b == b'_'
}

fn crate_name_from_ident(ident: &str) -> String {
    ident.replace('_', "-")
}

#[test]
fn apps_server_bootstrap_imports_are_declared() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");
    let workspace_root: &Path = metadata.workspace_root.as_std_path();

    let apps_server_pkg = metadata
        .packages
        .iter()
        .find(|p| p.name.as_str() == APPS_SERVER_PACKAGE)
        .unwrap_or_else(|| {
            panic!(
                "package `{APPS_SERVER_PACKAGE}` not found in workspace — did \
                 apps/server's Cargo.toml `name` field change?",
            )
        });

    let declared: HashSet<String> = apps_server_pkg
        .dependencies
        .iter()
        .filter(|d| d.kind == DependencyKind::Normal)
        .map(|d| d.name.clone())
        .collect();

    let bootstrap_path = workspace_root.join(BOOTSTRAP_REL_PATH);
    assert!(
        bootstrap_path.exists(),
        "{BOOTSTRAP_REL_PATH} does not exist — probe cannot vacuous-pass. \
         Update this probe if apps/server/src/bootstrap.rs moved again."
    );
    let source = fs::read_to_string(&bootstrap_path)
        .unwrap_or_else(|e| panic!("read {}: {e}", bootstrap_path.display()));

    let extracted = extract_reovim_crate_idents(&source);
    let checked_count = extracted.len();

    let missing: Vec<String> = extracted
        .iter()
        .filter_map(|ident| {
            let crate_name = crate_name_from_ident(ident);
            if declared.contains(&crate_name) {
                None
            } else {
                Some(format!("{ident} (expected Cargo.toml dep: `{crate_name}`)"))
            }
        })
        .collect();

    // Spot-check log (visible with `cargo test -- --nocapture`).
    eprintln!(
        "apps_server_imports_declared: extracted {checked_count} reovim_* crate idents: {extracted:?}"
    );

    assert!(
        checked_count >= SENTRY_MIN_COUNT,
        "extracted only {checked_count} reovim_* crate identifiers from {BOOTSTRAP_REL_PATH} \
         — expected at least {SENTRY_MIN_COUNT}. The import-extraction scanner is likely \
         broken (NOT the import set). Extracted set: {extracted:?}",
    );

    assert!(
        missing.is_empty(),
        "apps/server/src/bootstrap.rs references reovim_* crates not declared in \
         apps/server/Cargo.toml `[dependencies]`:\n  {}\n\
         \n\
         Each imported crate must be a direct production dep of `{APPS_SERVER_PACKAGE}` so \
         the build graph is stable against future transitive-dep trimming.",
        missing.join("\n  ")
    );
}
