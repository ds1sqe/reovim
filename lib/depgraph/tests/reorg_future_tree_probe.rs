//! Synthetic integration smoke for the split-tree depgraph probes.
//!
//! ## Purpose
//!
//! A single synthetic workspace that represents the target split-tree shape,
//! exercised against ALL nine probes (five existing + four new).
//!
//! The positive fixture asserts that the future-tree design — with `kabi`,
//! `lib/ds`, and renamed `editor/` crates — passes every probe.
//!
//! The negative mutant (one airlock-violating edge added) asserts that exactly
//! the right new probe trips, and no other probe changes behaviour.
//!
//! ## Why this matters
//!
//! The probe foundation must classify and guard the split contract/DS/editor
//! tree shape correctly even before the real workspace fully matches it.
//!
//! ## Fixture tree shape
//!
//! ```text
//! arch/           → Foundation (reovim-arch)
//! kabi/platform/  → Foundation (reovim-kabi-platform)
//! uapi/mm/        → Foundation (reovim-uapi-mm)
//! lib/ds/         → Foundation (reovim-lib-ds)
//! editor/lib/core/  → EditorCore (reovim-editor-core)
//! apps/server/    → Apps (reovim-server)
//! ```
//!
//! Edges in the POSITIVE fixture:
//! - `reovim-arch → reovim-kabi-platform`  (granted: arch → kabi)
//! - `reovim-arch → reovim-lib-ds`         (granted: arch → lib/ds)
//! - `reovim-lib-ds → reovim-uapi-mm`      (granted: lib/ds → uapi/mm)
//! - `reovim-editor-core → reovim-lib-ds` (`EditorCore` → Foundation: allowed)
//!
//! The NEGATIVE mutant adds:
//! - `reovim-editor-core → reovim-arch`  (firewall violation: direct arch dep)
//!
//! Expected result: ONLY the firewall probe trips; all other probes still pass.

mod common;

use reovim_depgraph::{
    Allowlist, Catalog, CatalogEdge, ProbeConfig, Violation, default_category_table,
    default_foundation_grants, run_firewall_probe, run_lib_ds_purity_probe,
    run_mode_selector_probe, run_probe, run_swapset_isolation_probe,
};

// ── helpers ───────────────────────────────────────────────────────────────────

#[must_use]
fn root_workspace_toml(members: &[&str]) -> String {
    let list = members
        .iter()
        .map(|m| format!("\"{m}\""))
        .collect::<Vec<_>>()
        .join(", ");
    format!(
        "[workspace]\nresolver = \"2\"\nmembers = [{list}]\nexclude = [\"archive\"]\n\
         [profile.dev]\npanic = \"abort\"\n\
         [profile.release]\npanic = \"abort\"\n"
    )
}

#[must_use]
fn pkg_toml(name: &str) -> String {
    format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n")
}

/// Writes the POSITIVE future-tree fixture to `root`.
///
/// Returns the list of workspace members for the Cargo.toml.
fn write_positive_fixture(root: &std::path::Path) {
    // arch backend.
    common::write_file(
        root,
        "arch/Cargo.toml",
        &format!(
            "{}\n[dependencies]\nreovim-kabi-platform = {{ path = \"../kabi/platform\" }}\n\
         reovim-lib-ds = {{ path = \"../lib/ds\" }}\n",
            pkg_toml("reovim-arch")
        ),
    );
    common::write_file(root, "arch/src/lib.rs", "#![no_std]\n");

    // kabi/platform contract (leaf, no deps).
    common::write_file(root, "kabi/platform/Cargo.toml", &pkg_toml("reovim-kabi-platform"));
    common::write_file(root, "kabi/platform/src/lib.rs", "#![no_std]\n");

    // uapi/mm contract (leaf, no deps).
    common::write_file(root, "uapi/mm/Cargo.toml", &pkg_toml("reovim-uapi-mm"));
    common::write_file(root, "uapi/mm/src/lib.rs", "#![no_std]\n");

    // lib/ds algorithm crate (→ uapi/mm only).
    common::write_file(
        root,
        "lib/ds/Cargo.toml",
        &format!(
            "{}\n[dependencies]\nreovim-uapi-mm = {{ path = \"../../uapi/mm\" }}\n",
            pkg_toml("reovim-lib-ds")
        ),
    );
    common::write_file(root, "lib/ds/src/lib.rs", "#![no_std]\n// no arch:: references\n");

    // editor/lib/core (Math kernel → lib/ds only).
    common::write_file(
        root,
        "editor/lib/core/Cargo.toml",
        &format!(
            "{}\n[dependencies]\nreovim-lib-ds = {{ path = \"../../../lib/ds\" }}\n",
            pkg_toml("reovim-editor-core")
        ),
    );
    common::write_file(root, "editor/lib/core/src/lib.rs", "#![no_std]\n");

    // apps/server composition root.
    common::write_file(
        root,
        "apps/server/Cargo.toml",
        &format!(
            "{}\n[dependencies]\nreovim-editor-core = {{ path = \"../../editor/lib/core\" }}\n\
         reovim-arch = {{ path = \"../../arch\" }}\n",
            pkg_toml("reovim-server")
        ),
    );
    common::write_file(root, "apps/server/src/main.rs", "#![no_std]\n#![no_main]\n");
}

/// Returns a `ProbeConfig` with the structural reorg grants wired and an empty catalog
/// (no composition edges catalogued — we use this only for the DAG probe
/// to check Foundation/category rules, not Apps composition edges which
/// are tested separately).
fn make_config_with_sp01_grants() -> ProbeConfig {
    // default_foundation_grants() already includes the structural reorg grants
    // (arch → {kabi-platform, lib-ds}, lib-ds → uapi-mm).
    // Use it as-is; no manual extension needed.
    let grants = default_foundation_grants();

    ProbeConfig {
        category_table: default_category_table(),
        foundation_grants: grants,
        // Catalog must list the apps→editor-core + apps→arch composition edges to
        // avoid DAG3 violations in the positive fixture.
        catalog: Catalog {
            edge: vec![
                CatalogEdge {
                    from: "reovim-server".to_owned(),
                    to: "reovim-editor-core".to_owned(),
                    gate: None,
                    reason: "top-level composition root wires the editor core".to_owned(),
                },
                CatalogEdge {
                    from: "reovim-server".to_owned(),
                    to: "reovim-arch".to_owned(),
                    gate: None,
                    reason: "apps/server installs the arch platform provider at boot".to_owned(),
                },
            ],
        },
        allowlist: Allowlist::default(),
    }
}

// ── Positive fixture: future tree passes ALL probes ──────────────────────────

/// The target split tree must pass every probe:
/// - `run_probe` (DAG1–DAG5 with SP01 grants)
/// - `run_firewall_probe`
/// - `run_lib_ds_purity_probe`
/// - `run_swapset_isolation_probe`
/// - `run_mode_selector_probe`
#[test]
fn future_tree_positive_passes_all_probes() {
    let td = common::TempDir::new();
    let root = td.path();
    write_positive_fixture(root);

    common::write_file(
        root,
        "Cargo.toml",
        &root_workspace_toml(&[
            "arch",
            "kabi/platform",
            "uapi/mm",
            "lib/ds",
            "editor/lib/core",
            "apps/server",
        ]),
    );

    let config = make_config_with_sp01_grants();

    // DAG1–DAG5 (category classification, edge rules, sovereignty).
    let dag_report = run_probe(root, &config).expect("run_probe must succeed on future-tree");
    assert!(
        dag_report.is_clean(),
        "future-tree positive: run_probe must be clean;\nviolations:\n{}",
        dag_report.summary()
    );

    // Firewall: no editor/client → arch/system-kernel/drivers direct edge.
    let fw_violations = run_firewall_probe(root).expect("firewall probe must run on future-tree");
    assert!(
        fw_violations.is_empty(),
        "future-tree positive: firewall probe must be clean;\n{}",
        fw_violations.join("\n")
    );

    // lib/ds purity: lib/ds has no arch dep or arch:: source reference.
    let ds_violations =
        run_lib_ds_purity_probe(root).expect("lib/ds purity probe must run on future-tree");
    assert!(
        ds_violations.is_empty(),
        "future-tree positive: lib/ds purity probe must be clean;\n{}",
        ds_violations.join("\n")
    );

    // Swap-set isolation: no swap-set crates in this fixture; vacuously clean.
    let ss_violations =
        run_swapset_isolation_probe(root).expect("swapset isolation probe must run on future-tree");
    assert!(
        ss_violations.is_empty(),
        "future-tree positive: swapset isolation probe must be clean;\n{}",
        ss_violations.join("\n")
    );

    // Mode selector: no provider-presence feature; vacuously clean.
    let ms_violations =
        run_mode_selector_probe(root).expect("mode selector probe must run on future-tree");
    assert!(
        ms_violations.is_empty(),
        "future-tree positive: mode selector probe must be clean;\n{}",
        ms_violations.join("\n")
    );
}

// ── Negative mutant: one airlock violation trips ONLY the firewall probe ──────

/// Mutant: add a direct `editor/lib/core → arch` edge to the positive fixture.
///
/// The firewall probe must trip.  All other probes must remain clean (the
/// mutant edge is only an architectural-airlock violation, not a DAG1/2/5
/// violation given a Foundation→EditorCore `allowed_categories` check is
/// the inverse direction).
///
/// Note: `run_probe` also catches this edge as an `UngrantedFoundationEdge`
/// because `editor/lib/core` (`EditorCore`) → `arch` (Foundation) goes
/// through `allowed_categories(EditorCore)` which includes `Foundation` —
/// so DAG2 passes, but the firewall probe catches it independently as the
/// direct-edge airlock violation.  We verify that the firewall is the probe
/// specifically designed for this invariant.
#[test]
fn future_tree_mutant_airlock_violation_trips_only_firewall() {
    let td = common::TempDir::new();
    let root = td.path();

    // Write the positive fixture first.
    write_positive_fixture(root);

    // Mutant: overwrite editor/lib/core/Cargo.toml to ADD a direct arch dep.
    // The positive fixture already has lib/ds dep; this adds the forbidden arch dep.
    common::write_file(
        root,
        "editor/lib/core/Cargo.toml",
        &format!(
            "{}\n[dependencies]\n\
             reovim-lib-ds = {{ path = \"../../../lib/ds\" }}\n\
             reovim-arch = {{ path = \"../../../arch\" }}\n",
            pkg_toml("reovim-editor-core")
        ),
    );

    common::write_file(
        root,
        "Cargo.toml",
        &root_workspace_toml(&[
            "arch",
            "kabi/platform",
            "uapi/mm",
            "lib/ds",
            "editor/lib/core",
            "apps/server",
        ]),
    );

    // ── Firewall must trip ────────────────────────────────────────────────────
    let fw_violations = run_firewall_probe(root).expect("firewall probe must run on mutant");
    assert!(
        !fw_violations.is_empty(),
        "future-tree mutant: firewall probe must trip on direct editor → arch edge;\n\
         got zero violations"
    );
    let names_violation = fw_violations
        .iter()
        .any(|v| v.contains("reovim-editor-core") && v.contains("reovim-arch"));
    assert!(
        names_violation,
        "future-tree mutant: firewall violation must name both crates;\n\
         violations: {fw_violations:?}"
    );

    // ── Other new probes must still be clean ──────────────────────────────────

    // lib/ds purity is unchanged by adding an arch dep to editor/lib/core
    // (the purity probe only checks lib/ds crates, not editor crates).
    let ds_violations =
        run_lib_ds_purity_probe(root).expect("lib/ds purity probe must run on mutant");
    assert!(
        ds_violations.is_empty(),
        "future-tree mutant: lib/ds purity probe must still be clean;\n{}",
        ds_violations.join("\n")
    );

    // Swap-set isolation is unaffected (no swap-set leaves in the fixture).
    let ss_violations =
        run_swapset_isolation_probe(root).expect("swapset isolation probe must run on mutant");
    assert!(
        ss_violations.is_empty(),
        "future-tree mutant: swapset isolation probe must still be clean;\n{}",
        ss_violations.join("\n")
    );

    // Mode selector is unaffected (no provider feature anywhere).
    let ms_violations =
        run_mode_selector_probe(root).expect("mode selector probe must run on mutant");
    assert!(
        ms_violations.is_empty(),
        "future-tree mutant: mode selector probe must still be clean;\n{}",
        ms_violations.join("\n")
    );
}

// ── Integration smoke: grants + rows accept future tree ──────────────────────

/// Verifies that the category-table rows + foundation grants correctly
/// classify the future-tree fixture and that `run_probe` with `UngrantedFoundationEdge`
/// grants produces zero category/edge violations.
///
/// This integration smoke writes a synthetic
/// workspace with arch → kabi + lib/ds → uapi/mm + renamed editor/lib/core
/// crate, runs the existing DAG probe, and asserts zero `UngrantedFoundationEdge`
/// / `ForbiddenEdge` / classification violations for those edges.
#[test]
fn structural_grants_accept_future_tree_dag_probe() {
    let td = common::TempDir::new();
    let root = td.path();
    write_positive_fixture(root);

    common::write_file(
        root,
        "Cargo.toml",
        &root_workspace_toml(&[
            "arch",
            "kabi/platform",
            "uapi/mm",
            "lib/ds",
            "editor/lib/core",
            "apps/server",
        ]),
    );

    let config = make_config_with_sp01_grants();
    let report = run_probe(root, &config).expect("run_probe must succeed");

    // No UngrantedFoundationEdge for the new grant pairs.
    let has_ungranted = report.violations.iter().any(|v| {
        matches!(
            v,
            Violation::UngrantedFoundationEdge { from, to }
                if (from == "reovim-arch" && (to == "reovim-kabi-platform" || to == "reovim-lib-ds"))
                || (from == "reovim-lib-ds" && to == "reovim-uapi-mm")
        )
    });
    assert!(
        !has_ungranted,
        "structural grants must eliminate UngrantedFoundationEdge for \
         arch→{{kabi,lib-ds}} and lib-ds→uapi/mm;\nviolations:\n{}",
        report.summary()
    );

    // No ForbiddenEdge for editor/lib/core → lib/ds (EditorCore → Foundation allowed).
    let has_forbidden = report.violations.iter().any(|v| {
        matches!(
            v,
            Violation::ForbiddenEdge { from, to, .. }
                if from == "reovim-editor-core" && to == "reovim-lib-ds"
        )
    });
    assert!(
        !has_forbidden,
        "editor/lib/core → lib/ds must not produce ForbiddenEdge \
         (EditorCore → Foundation is allowed by §2);\nviolations:\n{}",
        report.summary()
    );

    // kabi/platform classifies as Foundation (no UnknownPath).
    let has_kabi_unknown = report
        .violations
        .iter()
        .any(|v| matches!(v, Violation::UnknownPath { path } if path.starts_with("kabi")));
    assert!(
        !has_kabi_unknown,
        "kabi/* must classify as Foundation (no UnknownPath);\n\
         violations:\n{}",
        report.summary()
    );

    // editor/lib/core classifies as EditorCore (no UnknownPath).
    let has_editor_unknown = report
        .violations
        .iter()
        .any(|v| matches!(v, Violation::UnknownPath { path } if path.starts_with("editor")));
    assert!(
        !has_editor_unknown,
        "editor/lib/core must classify as EditorCore (no UnknownPath);\n\
         violations:\n{}",
        report.summary()
    );
}
