//! Mode/provider selector probe tests (SP01, architectural invariant 5).
//!
//! ## Invariant
//!
//! The provider-presence Cargo feature (the feature that gates whether a
//! specific platform provider is included in the build) must appear in:
//! - AT MOST ONE `apps/*` manifest  (0 is legal while direct dependencies bind providers)
//! - NO non-`apps/*` manifest
//!
//! The DAG edge-walker cannot see feature *definitions* (only dep edges), so
//! this probe reads manifests directly via a targeted grep.
//!
//! The current workspace binds providers through direct composition-root
//! dependencies, so count 0 is expected. If a selector feature is introduced,
//! this probe confines it to one auditable app root.
//!
//! The feature name searched for is `PROVIDER_PRESENCE_FEATURE` (see
//! `lib/depgraph/src/lib.rs`).
//!
//! ## Coverage
//!
//! 1. **Positive control (real tree)** — the selector feature is not used today,
//!    so the probe returns zero violations.
//!
//! 2. **Negative fixture A** — a non-`apps/*` manifest declaring the feature
//!    must trip the probe.
//!
//! 3. **Negative fixture B** — two `apps/*` manifests both declaring the
//!    feature (second count) must trip the probe.
//!
//! 4. **Positive fixture** — exactly one `apps/*` manifest declaring the
//!    feature must produce zero violations.
//!
//! 5. **Positive fixture (zero occurrences)** — a workspace with no crate
//!    declaring the feature produces zero violations.

mod common;

use reovim_depgraph::{PROVIDER_PRESENCE_FEATURE, run_mode_selector_probe};

// ── helpers ───────────────────────────────────────────────────────────────────

#[must_use]
fn root_workspace_toml(members: &[&str]) -> String {
    let list = members
        .iter()
        .map(|m| format!("\"{m}\""))
        .collect::<Vec<_>>()
        .join(", ");
    format!("[workspace]\nresolver = \"2\"\nmembers = [{list}]\nexclude = [\"archive\"]\n")
}

#[must_use]
fn pkg_toml(name: &str) -> String {
    format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n")
}

/// A manifest that declares `PROVIDER_PRESENCE_FEATURE` in its `[features]` section.
#[must_use]
fn pkg_toml_with_provider_feature(name: &str) -> String {
    format!(
        "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\
         [features]\n{PROVIDER_PRESENCE_FEATURE} = []\n"
    )
}

// ── 1. Positive control: real workspace ───────────────────────────────────────

/// The real workspace currently binds providers through direct dependencies, so
/// no provider-presence feature is declared. The probe must return zero
/// violations.
#[test]
fn mode_selector_real_workspace_is_clean() {
    let root = common::workspace_root();
    let violations =
        run_mode_selector_probe(&root).expect("mode selector probe must run on real workspace");
    assert!(
        violations.is_empty(),
        "mode-selector: real workspace has unexpected violations:\n{}",
        violations.join("\n")
    );
}

// ── 2. Negative fixture A: non-apps crate declares the feature ────────────────

/// A non-`apps/*` crate (e.g. `lib/ds`) that declares the provider-presence
/// feature must trip the mode-selector probe.
///
/// The provider-presence feature is a composition-root concern and must stay
/// confined to `apps/*`.  A library crate declaring it would mean the provider
/// selection leaks into the library contract — the opposite of the swappability
/// invariant this reorg builds.
#[test]
fn mode_selector_non_apps_feature_trips_probe() {
    let td = common::TempDir::new();
    let root = td.path();

    // A Foundation lib that incorrectly declares the provider feature.
    common::write_file(root, "lib/ds/Cargo.toml", &pkg_toml_with_provider_feature("reovim-lib-ds"));

    common::write_file(root, "Cargo.toml", &root_workspace_toml(&["lib/ds"]));

    let violations =
        run_mode_selector_probe(root).expect("mode selector probe must run on fixture A");

    assert!(
        !violations.is_empty(),
        "mode-selector fixture A: expected a violation for non-apps crate declaring \
         `{PROVIDER_PRESENCE_FEATURE}`;\ngot zero violations"
    );
    let mentions_crate = violations.iter().any(|v| v.contains("reovim-lib-ds"));
    assert!(
        mentions_crate,
        "mode-selector fixture A: violation must mention the offending crate;\n\
         violations: {violations:?}"
    );
}

// ── 3. Negative fixture B: two apps/* crates declare the feature ──────────────

/// Two `apps/*` crates both declaring the provider-presence feature must trip
/// the mode-selector probe.
///
/// AT MOST ONE `apps/*` manifest may declare the feature.  A second
/// declaration means the provider is being selected at more than one
/// composition point — the provider-binding is no longer a single, auditable
/// root.
#[test]
fn mode_selector_two_apps_crates_trip_probe() {
    let td = common::TempDir::new();
    let root = td.path();

    // Two apps crates, both declaring the provider feature.
    common::write_file(
        root,
        "apps/server/Cargo.toml",
        &pkg_toml_with_provider_feature("reovim-server"),
    );
    common::write_file(root, "apps/tui/Cargo.toml", &pkg_toml_with_provider_feature("reovim-tui"));

    common::write_file(root, "Cargo.toml", &root_workspace_toml(&["apps/server", "apps/tui"]));

    let violations =
        run_mode_selector_probe(root).expect("mode selector probe must run on fixture B");

    assert!(
        !violations.is_empty(),
        "mode-selector fixture B: expected a violation for two apps crates declaring \
         `{PROVIDER_PRESENCE_FEATURE}`;\ngot zero violations"
    );
}

// ── 4. Positive fixture: exactly one apps/* crate declares the feature ────────

/// Exactly one `apps/*` crate declaring the provider-presence feature must
/// produce zero violations.
///
/// The composition root is the sole point where the provider is selected.
#[test]
fn mode_selector_single_apps_feature_is_clean() {
    let td = common::TempDir::new();
    let root = td.path();

    // One apps crate with the feature (valid), one without.
    common::write_file(
        root,
        "apps/server/Cargo.toml",
        &pkg_toml_with_provider_feature("reovim-server"),
    );
    common::write_file(root, "apps/tui/Cargo.toml", &pkg_toml("reovim-tui"));

    common::write_file(root, "Cargo.toml", &root_workspace_toml(&["apps/server", "apps/tui"]));

    let violations =
        run_mode_selector_probe(root).expect("mode selector probe must run on single-apps fixture");

    assert!(
        violations.is_empty(),
        "mode-selector: single apps/* feature declaration must NOT trip the probe;\n\
         violations: {violations:?}"
    );
}

// ── 5. Positive fixture: zero occurrences ─────────────────────────────────────

/// A workspace with no crate declaring the provider-presence feature must
/// produce zero violations.
///
/// Count 0 is explicitly allowed; direct provider dependencies bind the chosen
/// provider in the current workspace.
#[test]
fn mode_selector_zero_occurrences_is_clean() {
    let td = common::TempDir::new();
    let root = td.path();

    // Workspace with no provider-presence feature anywhere.
    common::write_file(root, "lib/ds/Cargo.toml", &pkg_toml("reovim-lib-ds"));
    common::write_file(root, "apps/server/Cargo.toml", &pkg_toml("reovim-server"));

    common::write_file(root, "Cargo.toml", &root_workspace_toml(&["lib/ds", "apps/server"]));

    let violations = run_mode_selector_probe(root)
        .expect("mode selector probe must run on zero-occurrences fixture");

    assert!(
        violations.is_empty(),
        "mode-selector: zero occurrences of the provider feature must NOT trip the probe;\n\
         violations: {violations:?}"
    );
}
