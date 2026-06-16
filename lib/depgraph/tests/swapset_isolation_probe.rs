//! Swap-set isolation probe tests (SP01, architectural invariant 4 — no impl→impl).
//!
//! ## Invariant
//!
//! No Cargo dep edge may exist between two swap-set leaves that belong to
//! DIFFERENT swap-set groups.  A swap-set leaf is a crate whose workspace path
//! matches `<root>/<set>/<leaf>` where `<root>` is one of `editor`, `client`,
//! `system` and `<set>` is one of `drivers`, `modules`, `providers`,
//! `platforms`, `capabilities`.
//!
//! Two leaves in the SAME group (e.g. both under `editor/drivers/`) are
//! allowed to reference each other (rare but not architecturally forbidden by
//! this rule).  Two leaves in DIFFERENT groups must never cross-depend —
//! composition is the sole responsibility of `apps/*` composition roots.
//!
//! This is the nouveau-vs-nvidia isolation generalized to the new
//! `editor/`/`client/`/`system/` tree.
//!
//! ## Coverage
//!
//! 1. **Positive control (real tree)** — no swap-set leaves exist yet
//!    (pre-SP04), so the probe returns zero violations vacuously.
//!
//! 2. **Negative fixture A** — two driver leaves under `editor/drivers/` and
//!    `client/drivers/` cross-depending must trip the probe.
//!
//! 3. **Negative fixture B** — a module leaf depending on a platform leaf must
//!    trip the probe.
//!
//! 4. **Positive fixture (same group)** — two crates under `editor/drivers/`
//!    cross-depending produce zero violations (same swap-set group).
//!
//! 5. **Positive fixture (non-leaf dep)** — a swap-set leaf depending on a
//!    Foundation lib produces zero violations (the dep target is not a
//!    swap-set leaf).

mod common;

use reovim_depgraph::run_swapset_isolation_probe;

// ── helpers ───────────────────────────────────────────────────────────────────

#[must_use]
fn root_workspace_toml(members: &[&str]) -> String {
    let list = members.iter().map(|m| format!("\"{}\"", m)).collect::<Vec<_>>().join(", ");
    format!("[workspace]\nresolver = \"2\"\nmembers = [{list}]\nexclude = [\"archive\"]\n")
}

#[must_use]
fn pkg_toml(name: &str) -> String {
    format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n")
}

#[must_use]
fn pkg_toml_with_path_dep(name: &str, dep_name: &str, dep_rel_path: &str) -> String {
    format!(
        "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\
         [dependencies]\n{dep_name} = {{ path = \"{dep_rel_path}\" }}\n"
    )
}

// ── 1. Positive control: real workspace ───────────────────────────────────────

/// The real workspace has no swap-set leaves yet (pre-SP04).  The probe must
/// return zero violations vacuously.
///
/// This becomes the production enforcement test once SP04 lands the
/// `editor/`/`client/` swap-set directories.
#[test]
fn swapset_isolation_real_workspace_is_clean() {
    let root = common::workspace_root();
    let violations = run_swapset_isolation_probe(&root)
        .expect("swapset isolation probe must run on real workspace");
    assert!(
        violations.is_empty(),
        "swapset-isolation: real workspace has unexpected violations:\n{}",
        violations.join("\n")
    );
}

// ── 2. Negative fixture A: editor/drivers/gpu → client/drivers/usb ───────────

/// A driver leaf under `editor/drivers/` depending on a driver leaf under
/// `client/drivers/` must trip the isolation probe.
///
/// These are in different swap-set groups (`editor/drivers` vs `client/drivers`).
/// The coupling violates the nouveau-vs-nvidia isolation principle.
#[test]
fn swapset_isolation_cross_root_driver_deps_trip_probe() {
    let td = common::TempDir::new();
    let root = td.path();

    // Two driver leaves in different kernel roots.
    common::write_file(
        root,
        "client/drivers/usb/Cargo.toml",
        &pkg_toml("reovim-client-driver-usb"),
    );
    common::write_file(
        root,
        "editor/drivers/gpu/Cargo.toml",
        &pkg_toml_with_path_dep(
            "reovim-editor-driver-gpu",
            "reovim-client-driver-usb",
            "../../../client/drivers/usb",
        ),
    );

    common::write_file(
        root,
        "Cargo.toml",
        &root_workspace_toml(&["client/drivers/usb", "editor/drivers/gpu"]),
    );

    let violations = run_swapset_isolation_probe(root)
        .expect("swapset isolation probe must run on fixture A");

    assert!(
        !violations.is_empty(),
        "swapset-isolation fixture A: expected a violation for \
         editor/drivers/gpu → client/drivers/usb;\ngot zero violations"
    );
    let names_both = violations.iter().any(|v| {
        v.contains("reovim-editor-driver-gpu") && v.contains("reovim-client-driver-usb")
    });
    assert!(
        names_both,
        "swapset-isolation fixture A: violation must name both crates;\n\
         violations: {:?}",
        violations
    );
}

// ── 3. Negative fixture B: editor/modules/vim → editor/platforms/x ───────────

/// A module leaf depending on a platforms leaf (different swap-set groups
/// within the same kernel root) must trip the probe.
///
/// `editor/modules` and `editor/platforms` are distinct swap-set groups even
/// though both are under `editor/`.
#[test]
fn swapset_isolation_cross_set_module_to_platform_trips_probe() {
    let td = common::TempDir::new();
    let root = td.path();

    // A platform leaf and a module leaf in the same kernel root but different groups.
    common::write_file(
        root,
        "editor/platforms/x/Cargo.toml",
        &pkg_toml("reovim-editor-platform-x"),
    );
    common::write_file(
        root,
        "editor/modules/vim/Cargo.toml",
        &pkg_toml_with_path_dep(
            "reovim-editor-module-vim",
            "reovim-editor-platform-x",
            "../../platforms/x",
        ),
    );

    common::write_file(
        root,
        "Cargo.toml",
        &root_workspace_toml(&["editor/platforms/x", "editor/modules/vim"]),
    );

    let violations = run_swapset_isolation_probe(root)
        .expect("swapset isolation probe must run on fixture B");

    assert!(
        !violations.is_empty(),
        "swapset-isolation fixture B: expected a violation for \
         editor/modules/vim → editor/platforms/x;\ngot zero violations"
    );
}

// ── 4. Positive fixture: same swap-set group ──────────────────────────────────

/// Two crates under the SAME swap-set group (`editor/drivers/`) are not
/// cross-group and must NOT be flagged by the isolation probe.
///
/// The isolation rule only forbids cross-group deps; within a group the probe
/// is silent.
#[test]
fn swapset_isolation_same_group_is_clean() {
    let td = common::TempDir::new();
    let root = td.path();

    // Two driver leaves in the same group.
    common::write_file(
        root,
        "editor/drivers/base/Cargo.toml",
        &pkg_toml("reovim-editor-driver-base"),
    );
    common::write_file(
        root,
        "editor/drivers/gpu/Cargo.toml",
        &pkg_toml_with_path_dep(
            "reovim-editor-driver-gpu",
            "reovim-editor-driver-base",
            "../base",
        ),
    );

    common::write_file(
        root,
        "Cargo.toml",
        &root_workspace_toml(&["editor/drivers/base", "editor/drivers/gpu"]),
    );

    let violations = run_swapset_isolation_probe(root)
        .expect("swapset isolation probe must run on same-group fixture");

    assert!(
        violations.is_empty(),
        "swapset-isolation: same-group dep must NOT trip the probe;\n\
         violations: {:?}",
        violations
    );
}

// ── 5. Positive fixture: swap-set leaf → Foundation lib ───────────────────────

/// A swap-set leaf depending on a Foundation library must NOT be flagged.
///
/// The isolation rule is between TWO swap-set leaves.  A dep from a swap-set
/// leaf to `lib/ds` (Foundation) is categorically different and is governed by
/// the firewall probe (not this probe).
#[test]
fn swapset_isolation_leaf_to_foundation_is_clean() {
    let td = common::TempDir::new();
    let root = td.path();

    // Foundation lib (not a swap-set leaf).
    common::write_file(root, "lib/ds/Cargo.toml", &pkg_toml("reovim-lib-ds"));
    // A swap-set leaf with a dep on a Foundation lib.
    common::write_file(
        root,
        "editor/drivers/gpu/Cargo.toml",
        &pkg_toml_with_path_dep(
            "reovim-editor-driver-gpu",
            "reovim-lib-ds",
            "../../../lib/ds",
        ),
    );

    common::write_file(
        root,
        "Cargo.toml",
        &root_workspace_toml(&["lib/ds", "editor/drivers/gpu"]),
    );

    let violations = run_swapset_isolation_probe(root)
        .expect("swapset isolation probe must run on Foundation-dep fixture");

    assert!(
        violations.is_empty(),
        "swapset-isolation: swap-set leaf → Foundation lib must NOT trip the probe;\n\
         violations: {:?}",
        violations
    );
}
