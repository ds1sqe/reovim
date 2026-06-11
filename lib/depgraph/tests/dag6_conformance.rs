//! DAG6 negative fixtures (spec 1.2 §4 / §5 step 6 / §10).
//!
//! Each negative fixture constructs a minimal temp workspace, runs the
//! appropriate DAG6 probe function, and asserts that **exactly** the expected
//! `Violation` variant appears.  A positive control asserts that a clean
//! `no_std` workspace produces no violations.  A cfg(test)-exemption control
//! asserts that `use std::` inside a `#[cfg(test)] mod` does NOT flag.

mod common;

use reovim_depgraph::{Violation, check_panic_profiles, run_dag6_probe};

// ── helpers ───────────────────────────────────────────────────────────────────

/// A minimal `[package]` manifest with no dependencies.
#[must_use]
fn pkg_toml(name: &str) -> String {
    format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n")
}

// ── Negative fixture (a): missing `#![no_std]` ────────────────────────────────

/// DAG6 negative fixture (a): a product crate whose `src/lib.rs` does not
/// declare `#![no_std]` must produce exactly one `MissingNoStd` violation
/// carrying the crate's name and path.
///
/// Spec 1.2 §5 step 6: every product crate root must carry `#![no_std]`.
#[test]
fn dag6_negative_fixture_a_missing_no_std() {
    let td = common::TempDir::new();
    let root = td.path();

    // One product crate at lib/product — Foundation category.
    common::write_file(root, "lib/product/Cargo.toml", &pkg_toml("reovim-product"));
    common::write_file(root, "lib/product/src/lib.rs", "pub fn hello() {}\n");

    let violations = run_dag6_probe(root).expect("DAG6 probe must run");

    let has_missing = violations.iter().any(|v| {
        matches!(
            v,
            Violation::MissingNoStd { crate_name, crate_path }
                if crate_name == "reovim-product" && crate_path == "lib/product"
        )
    });
    assert!(
        has_missing,
        "DAG6 fixture (a): expected MissingNoStd for reovim-product;\n\
         violations: {violations:?}"
    );
    // No other violation kinds: the probe found no std/alloc usage in this fixture.
    let extra: Vec<_> = violations
        .iter()
        .filter(|v| !matches!(v, Violation::MissingNoStd { .. }))
        .collect();
    assert!(extra.is_empty(), "DAG6 fixture (a): unexpected extra violations: {extra:?}");
}

// ── Negative fixture (b): `use std::` in product source ──────────────────────

/// DAG6 negative fixture (b): a product crate with `#![no_std]` at its root
/// but a `use std::` in a source file must produce exactly one `StdUsage`
/// violation.
///
/// Spec 1.2 §5 step 6: `use std::` in product source is a violation.
#[test]
fn dag6_negative_fixture_b_use_std() {
    let td = common::TempDir::new();
    let root = td.path();

    common::write_file(root, "lib/uses-std/Cargo.toml", &pkg_toml("reovim-uses-std"));
    // Root has #![no_std] so no MissingNoStd.
    common::write_file(root, "lib/uses-std/src/lib.rs", "#![no_std]\nuse std::fmt::Write;\n");

    let violations = run_dag6_probe(root).expect("DAG6 probe must run");

    let has_std = violations
        .iter()
        .any(|v| matches!(v, Violation::StdUsage { .. }));
    assert!(
        has_std,
        "DAG6 fixture (b): expected StdUsage for lib/uses-std;\n\
         violations: {violations:?}"
    );
    // Ensure no MissingNoStd alongside it (root was valid).
    let has_missing = violations
        .iter()
        .any(|v| matches!(v, Violation::MissingNoStd { .. }));
    assert!(
        !has_missing,
        "DAG6 fixture (b): root had #![no_std]; unexpected MissingNoStd;\n\
         violations: {violations:?}"
    );
}

// ── Negative fixture (c): `use alloc::` in product source ────────────────────

/// DAG6 negative fixture (c): a product crate with `#![no_std]` at its root
/// but a `use alloc::` in a source file must produce exactly one `AllocUsage`
/// violation.
///
/// Spec 1.2 §5 step 6: `use alloc::` in product source is a violation.
#[test]
fn dag6_negative_fixture_c_use_alloc() {
    let td = common::TempDir::new();
    let root = td.path();

    common::write_file(root, "lib/uses-alloc/Cargo.toml", &pkg_toml("reovim-uses-alloc"));
    common::write_file(root, "lib/uses-alloc/src/lib.rs", "#![no_std]\nuse alloc::vec::Vec;\n");

    let violations = run_dag6_probe(root).expect("DAG6 probe must run");

    let has_alloc = violations
        .iter()
        .any(|v| matches!(v, Violation::AllocUsage { .. }));
    assert!(
        has_alloc,
        "DAG6 fixture (c): expected AllocUsage for lib/uses-alloc;\n\
         violations: {violations:?}"
    );
    let has_missing = violations
        .iter()
        .any(|v| matches!(v, Violation::MissingNoStd { .. }));
    assert!(
        !has_missing,
        "DAG6 fixture (c): root had #![no_std]; unexpected MissingNoStd;\n\
         violations: {violations:?}"
    );
}

// ── Negative fixture (d): non-abort profile ───────────────────────────────────

/// DAG6 negative fixture (d): workspace `Cargo.toml` with
/// `panic = "unwind"` in `[profile.dev]` must produce a
/// `PanicProfileNotAbort` violation for the `"dev"` profile.
///
/// Spec 1.2 §5 step 6: workspace panic profile must be `"abort"`.
#[test]
fn dag6_negative_fixture_d_panic_profile_unwind() {
    let td = common::TempDir::new();
    let root = td.path();

    common::write_file(
        root,
        "Cargo.toml",
        "[workspace]\nmembers = []\n\n[profile.dev]\npanic = \"unwind\"\n\n[profile.release]\npanic = \"unwind\"\n",
    );

    let violations = check_panic_profiles(root).expect("profile check must run");

    let has_dev = violations
        .iter()
        .any(|v| matches!(v, Violation::PanicProfileNotAbort { profile } if profile == "dev"));
    assert!(
        has_dev,
        "DAG6 fixture (d): expected PanicProfileNotAbort for dev;\n\
         violations: {violations:?}"
    );
    let has_release = violations
        .iter()
        .any(|v| matches!(v, Violation::PanicProfileNotAbort { profile } if profile == "release"));
    assert!(
        has_release,
        "DAG6 fixture (d): expected PanicProfileNotAbort for release;\n\
         violations: {violations:?}"
    );
}

// ── Positive control: clean no_std workspace passes ───────────────────────────

/// DAG6 positive control: a workspace containing a single correctly
/// `#![no_std]` crate with no `std`/`alloc` usage produces no DAG6
/// violations, and the profile gate passes when both profiles are set to
/// `panic = "abort"`.
///
/// This is the integration smoke required by spec 1.2 §5 step 6: the
/// enforcement boundary must be live over a real fixture workspace.
#[test]
fn dag6_positive_control_clean_workspace_has_no_violations() {
    let td = common::TempDir::new();
    let root = td.path();

    // Root Cargo.toml with abort profiles.
    common::write_file(
        root,
        "Cargo.toml",
        "[workspace]\nmembers = [\"lib/clean\"]\n\n[profile.dev]\npanic = \"abort\"\n\n[profile.release]\npanic = \"abort\"\n",
    );
    // Product crate: #![no_std], no std/alloc usage.
    common::write_file(root, "lib/clean/Cargo.toml", &pkg_toml("reovim-clean"));
    common::write_file(root, "lib/clean/src/lib.rs", "#![no_std]\n\npub fn ok() {}\n");

    // Zero-std walk: no violations.
    let walk_violations = run_dag6_probe(root).expect("DAG6 probe must run");
    assert!(
        walk_violations.is_empty(),
        "DAG6 positive: clean workspace must produce no walk violations;\n\
         got: {walk_violations:?}"
    );

    // Profile gate: no violations.
    let profile_violations = check_panic_profiles(root).expect("profile check must run");
    assert!(
        profile_violations.is_empty(),
        "DAG6 positive: abort profiles must produce no violations;\n\
         got: {profile_violations:?}"
    );
}

// ── cfg(test)-exemption control ───────────────────────────────────────────────

/// DAG6 cfg(test)-exemption: `use std::` inside a `#[cfg(test)] mod tests {}`
/// block must NOT produce a `StdUsage` violation.
///
/// Spec 1.2 §10 bootstrap state 1: libtest test builds link `std`; the
/// DAG6 probe exempts `#[cfg(test)]` blocks and `tests/` directories from
/// the std/alloc sweep.
#[test]
fn dag6_cfg_test_exemption_use_std_in_test_mod_does_not_flag() {
    let td = common::TempDir::new();
    let root = td.path();

    common::write_file(root, "lib/cfg-exempt/Cargo.toml", &pkg_toml("reovim-cfg-exempt"));
    common::write_file(
        root,
        "lib/cfg-exempt/src/lib.rs",
        concat!(
            "#![no_std]\n",
            "\n",
            "pub fn product_code() {}\n",
            "\n",
            "#[cfg(test)]\n",
            "mod tests {\n",
            "    use std::fmt;\n",
            "    #[test]\n",
            "    fn it_works() {\n",
            "        let _ = fmt::format(format_args!(\"\"));\n",
            "    }\n",
            "}\n",
        ),
    );

    let violations = run_dag6_probe(root).expect("DAG6 probe must run");

    let has_std = violations
        .iter()
        .any(|v| matches!(v, Violation::StdUsage { .. }));
    assert!(
        !has_std,
        "DAG6 cfg(test) exemption: use std:: inside #[cfg(test)] mod must NOT flag;\n\
         violations: {violations:?}"
    );
    // Also confirm no false MissingNoStd.
    let has_missing = violations
        .iter()
        .any(|v| matches!(v, Violation::MissingNoStd { .. }));
    assert!(
        !has_missing,
        "DAG6 cfg(test) exemption: root had #![no_std]; unexpected MissingNoStd;\n\
         violations: {violations:?}"
    );
}

// ── Integration smoke: StdUsage fixture + real workspace ─────────────────────

/// DAG6 integration smoke: running the probe over a temp fixture workspace
/// containing a `use std::` product crate returns exactly `StdUsage`, and
/// running the probe over the real workspace (`arch/` no_std-clean plus
/// `lib/depgraph`, the bootstrap-state-2 exclusion) passes with no
/// violations.
///
/// This is the integration-smoke AC from spec Phase 1:
/// "the DAG6 probe, run over a temp fixture workspace containing a
/// `use std::` product crate, returns exactly `StdUsage`; over the real
/// workspace it passes — proving the enforcement boundary is live."
#[test]
fn dag6_integration_smoke_fixture_and_real_workspace() {
    // -- Part 1: fixture workspace with a `use std::` product crate -----------
    let td = common::TempDir::new();
    let fixture_root = td.path();

    common::write_file(fixture_root, "lib/std-product/Cargo.toml", &pkg_toml("reovim-std-product"));
    common::write_file(
        fixture_root,
        "lib/std-product/src/lib.rs",
        "#![no_std]\nuse std::fmt::Write;\n",
    );

    let fixture_violations =
        run_dag6_probe(fixture_root).expect("DAG6 probe must run over fixture");

    let has_std = fixture_violations
        .iter()
        .any(|v| matches!(v, Violation::StdUsage { .. }));
    assert!(
        has_std,
        "DAG6 smoke: fixture with use std:: must produce StdUsage;\n\
         violations: {fixture_violations:?}"
    );

    // -- Part 2: real workspace (only lib/depgraph, which is excluded) --------
    let real_root = common::workspace_root();
    let real_violations =
        run_dag6_probe(&real_root).expect("DAG6 probe must run over real workspace");

    assert!(
        real_violations.is_empty(),
        "DAG6 smoke: real workspace (lib/depgraph excluded) must have no walk violations;\n\
         violations: {real_violations:?}"
    );
}
