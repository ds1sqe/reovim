//! DAG1..DAG5 negative fixtures (spec 1.2 §4 / §5).
//!
//! Each fixture constructs a minimal temp workspace, runs the probe over
//! it, and asserts that exactly the expected `Violation` variant appears.
//! Positive controls assert that clean workspaces produce no violations.

mod common;

use std::collections::BTreeMap;

use reovim_depgraph::{
    Catalog, DepTable, ProbeConfig, Violation, default_category_table, run_probe,
};

// ── helpers ───────────────────────────────────────────────────────────────────

/// Minimal workspace manifest written at `<root>/Cargo.toml`. Members are
/// assembled from `members_list` (already-formatted TOML list items).
#[must_use]
fn root_workspace_toml(members_list: &str) -> String {
    format!("[workspace]\nresolver = \"2\"\nmembers = [{members_list}]\nexclude = [\"archive\"]\n")
}

/// A minimal `[package]` manifest with no dependencies.
#[must_use]
fn pkg_toml(name: &str) -> String {
    format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n")
}

#[must_use]
fn empty_config() -> ProbeConfig {
    ProbeConfig {
        category_table: default_category_table(),
        foundation_grants: BTreeMap::new(),
        catalog: Catalog::default(),
        allowlist: reovim_depgraph::Allowlist::default(),
    }
}

// ── DAG1: UnknownPath ─────────────────────────────────────────────────────────

/// DAG1: a crate at `mystery/x/` must produce `UnknownPath`.
/// Spec 1.2 §4 DAG1: every crate path must match exactly one §1 pattern.
#[test]
fn dag1_unknown_path_crate_produces_unknown_path_violation() {
    let td = common::TempDir::new();
    let root = td.path();
    common::write_file(root, "Cargo.toml", &root_workspace_toml("\"mystery/x\""));
    common::write_file(root, "mystery/x/Cargo.toml", &pkg_toml("mystery-x"));

    let config = empty_config();
    let report = run_probe(root, &config).expect("probe must run");

    let has_unknown = report
        .violations
        .iter()
        .any(|v| matches!(v, Violation::UnknownPath { path } if path == "mystery/x"));
    assert!(
        has_unknown,
        "DAG1: expected UnknownPath for mystery/x;\nviolations: {:?}",
        report.violations
    );
}

// ── DAG2: ForbiddenEdge ───────────────────────────────────────────────────────

/// DAG2: `lib/alpha` (Foundation) → `editor/lib/core/beta` (`EditorCore`)
/// is a forbidden edge: Foundation may not depend on `EditorCore`.
/// Spec 1.2 §2: Foundation has no allowed outbound categories.
#[test]
fn dag2_lib_to_server_kernel_produces_forbidden_edge_violation() {
    let td = common::TempDir::new();
    let root = td.path();
    common::write_file(
        root,
        "Cargo.toml",
        &root_workspace_toml(r#""lib/alpha", "editor/lib/core/beta""#),
    );
    // lib/alpha depends on editor/lib/core/beta (forbidden: Foundation → EditorCore).
    common::write_file(
        root,
        "lib/alpha/Cargo.toml",
        &format!(
            "{}\n[dependencies]\nbeta = {{ path = \"../../editor/lib/core/beta\" }}\n",
            pkg_toml("alpha")
        ),
    );
    common::write_file(root, "editor/lib/core/beta/Cargo.toml", &pkg_toml("beta"));

    let config = empty_config();
    let report = run_probe(root, &config).expect("probe must run");

    // The edge shows as either ForbiddenEdge or UngrantedFoundationEdge depending on
    // check_edge dispatch. For Foundation→non-Foundation the check produces
    // UngrantedFoundationEdge (no §6 grant). Both are DAG2 violations.
    let has_dag2 = report.violations.iter().any(|v| {
        matches!(
            v,
            Violation::ForbiddenEdge { from, to, .. }
                if from == "alpha" && to == "beta"
        ) || matches!(
            v,
            Violation::UngrantedFoundationEdge { from, to }
                if from == "alpha" && to == "beta"
        )
    });
    assert!(
        has_dag2,
        "DAG2: expected ForbiddenEdge or UngrantedFoundationEdge for alpha→beta;\n\
         violations: {:?}",
        report.violations
    );
}

// ── DAG3: UncatalogedCompositionEdge ─────────────────────────────────────────

/// DAG3: `apps/cli` (Apps) → `client/drivers/foo` (`ClientExt`) with an
/// empty composition catalog must produce `UncatalogedCompositionEdge`.
/// Spec 1.2 §3/§7: every composition root edge must be cataloged.
#[test]
fn dag3_uncataloged_composition_edge_produces_violation() {
    let td = common::TempDir::new();
    let root = td.path();
    common::write_file(
        root,
        "Cargo.toml",
        &root_workspace_toml(r#""apps/cli", "client/drivers/foo""#),
    );
    common::write_file(
        root,
        "apps/cli/Cargo.toml",
        &format!(
            "{}\n[dependencies]\nreovim-driver-foo = {{ path = \"../../client/drivers/foo\" }}\n",
            pkg_toml("reovim-cli")
        ),
    );
    common::write_file(root, "client/drivers/foo/Cargo.toml", &pkg_toml("reovim-driver-foo"));

    // Empty composition catalog — no edge granted.
    let config = ProbeConfig {
        category_table: default_category_table(),
        foundation_grants: BTreeMap::new(),
        catalog: Catalog::default(),
        allowlist: reovim_depgraph::Allowlist::default(),
    };
    let report = run_probe(root, &config).expect("probe must run");

    let has_dag3 = report.violations.iter().any(|v| {
        matches!(
            v,
            Violation::UncatalogedCompositionEdge { from, to }
                if from == "reovim-cli" && to == "reovim-driver-foo"
        )
    });
    assert!(
        has_dag3,
        "DAG3: expected UncatalogedCompositionEdge for reovim-cli→reovim-driver-foo;\n\
         violations: {:?}",
        report.violations
    );
}

// ── DAG5: NonSovereignDep — per-table fixtures ────────────────────────────────

/// Writes a minimal workspace with a single `lib/probe` crate that has one
/// registry dependency in the specified dependency table, then runs the probe
/// and returns the report.
#[must_use]
fn dag5_registry_dep_fixture(table_header: &str, dep_line: &str) -> reovim_depgraph::Report {
    let td = common::TempDir::new();
    let root = td.path();
    common::write_file(root, "Cargo.toml", &root_workspace_toml("\"lib/probe\""));
    common::write_file(
        root,
        "lib/probe/Cargo.toml",
        &format!("{}\n[{table_header}]\n{dep_line}\n", pkg_toml("reovim-probe")),
    );

    let config = empty_config();
    // We need root to stay alive; return from owned copy.
    let report = run_probe(root, &config).expect("probe must run");
    // td drops here — that is fine, the root is no longer needed.
    report
}

/// DAG5 ×1: registry dep in `[dependencies]` → `NonSovereignDep`.
#[test]
fn dag5_registry_dep_in_dependencies_produces_non_sovereign() {
    let report = dag5_registry_dep_fixture("dependencies", r#"serde = "1.0""#);
    let has = report.violations.iter().any(|v| {
        matches!(
            v,
            Violation::NonSovereignDep {
                table: DepTable::Dependencies,
                dep,
                ..
            } if dep == "serde"
        )
    });
    assert!(
        has,
        "DAG5 [dependencies]: expected NonSovereignDep for serde;\n\
         violations: {:?}",
        report.violations
    );
}

/// DAG5 ×2: registry dep in `[build-dependencies]` → `NonSovereignDep`.
#[test]
fn dag5_registry_dep_in_build_dependencies_produces_non_sovereign() {
    let report = dag5_registry_dep_fixture("build-dependencies", r#"cc = "1.0""#);
    let has = report.violations.iter().any(|v| {
        matches!(
            v,
            Violation::NonSovereignDep {
                table: DepTable::BuildDependencies,
                dep,
                ..
            } if dep == "cc"
        )
    });
    assert!(
        has,
        "DAG5 [build-dependencies]: expected NonSovereignDep for cc;\n\
         violations: {:?}",
        report.violations
    );
}

/// DAG5 ×3: registry dep in `[dev-dependencies]` → `NonSovereignDep`.
#[test]
fn dag5_registry_dep_in_dev_dependencies_produces_non_sovereign() {
    let report = dag5_registry_dep_fixture("dev-dependencies", r#"tempfile = "3.0""#);
    let has = report.violations.iter().any(|v| {
        matches!(
            v,
            Violation::NonSovereignDep {
                table: DepTable::DevDependencies,
                dep,
                ..
            } if dep == "tempfile"
        )
    });
    assert!(
        has,
        "DAG5 [dev-dependencies]: expected NonSovereignDep for tempfile;\n\
         violations: {:?}",
        report.violations
    );
}

// ── DAG5: workspace = true (distinct shape) ───────────────────────────────────

/// DAG5 workspace=true: `dep = { workspace = true }` is a distinct manifest
/// shape from a bare registry string. Because `[workspace.dependencies]` is
/// intentionally empty (L9 marker), any `workspace = true` dep must produce
/// `NonSovereignDep` independently.
///
/// This fixture uses `[dependencies]` to keep the table field verifiable.
#[test]
fn dag5_workspace_true_dep_produces_non_sovereign() {
    let td = common::TempDir::new();
    let root = td.path();
    common::write_file(root, "Cargo.toml", &root_workspace_toml("\"lib/probe\""));
    common::write_file(
        root,
        "lib/probe/Cargo.toml",
        &format!("{}\n[dependencies]\nserde = {{ workspace = true }}\n", pkg_toml("reovim-probe")),
    );

    let config = empty_config();
    let report = run_probe(root, &config).expect("probe must run");

    let has = report.violations.iter().any(|v| {
        matches!(
            v,
            Violation::NonSovereignDep {
                table: DepTable::Dependencies,
                dep,
                ..
            } if dep == "serde"
        )
    });
    assert!(
        has,
        "DAG5 workspace=true: expected NonSovereignDep for serde;\n\
         violations: {:?}",
        report.violations
    );
}

// ── DAG5: positive control ────────────────────────────────────────────────────

/// DAG5 positive control: a crate with ONLY `path = "..."` deps on an
/// in-repo sibling must produce NO `NonSovereignDep`.
///
/// Both crates sit under `lib/` (Foundation), and the probe is configured
/// with a §6 grant so the Foundation→Foundation edge is also clean.
#[test]
fn dag5_path_only_deps_are_sovereign() {
    let td = common::TempDir::new();
    let root = td.path();
    common::write_file(root, "Cargo.toml", &root_workspace_toml(r#""lib/alpha", "lib/beta""#));
    common::write_file(
        root,
        "lib/alpha/Cargo.toml",
        &format!(
            "{}\n[dependencies]\nreovim-beta = {{ path = \"../beta\" }}\n",
            pkg_toml("reovim-alpha")
        ),
    );
    common::write_file(root, "lib/beta/Cargo.toml", &pkg_toml("reovim-beta"));

    let mut grants = BTreeMap::new();
    grants.insert("reovim-alpha".to_owned(), vec!["reovim-beta".to_owned()]);
    let config = ProbeConfig {
        category_table: default_category_table(),
        foundation_grants: grants,
        catalog: Catalog::default(),
        allowlist: reovim_depgraph::Allowlist::default(),
    };
    let report = run_probe(root, &config).expect("probe must run");

    let has_dag5 = report
        .violations
        .iter()
        .any(|v| matches!(v, Violation::NonSovereignDep { .. }));
    assert!(
        !has_dag5,
        "DAG5 positive: path-only deps must not produce NonSovereignDep;\n\
         violations: {:?}",
        report.violations
    );
}

// ── DAG4: install-layout paths produce no crate row ──────────────────────────

/// DAG4: `enumerate_crates` reads manifest files only. A non-`Cargo.toml`
/// install-layout path (e.g. `module/server/foo.so`) must produce no crate
/// entry and therefore no violation.
///
/// Spec 1.2 §4 DAG4: the probe is manifest-only; runtime install layouts
/// (`$ROOT/{module,driver,capability}/...`) cannot grant a Cargo edge.
/// The probe does not reach outside `Cargo.toml` files, so install-path
/// inputs simply do not exist in the probe's data model.
#[test]
fn dag4_non_cargo_toml_paths_produce_no_crate_entry() {
    let td = common::TempDir::new();
    let root = td.path();
    // Workspace with one valid crate.
    common::write_file(root, "Cargo.toml", &root_workspace_toml("\"lib/depgraph\""));
    common::write_file(root, "lib/depgraph/Cargo.toml", &pkg_toml("reovim-depgraph"));
    // Install-layout files that are NOT Cargo manifests: the walk must ignore them.
    common::write_file(root, "module/server/foo.so", "ELF placeholder");
    common::write_file(root, "driver/client/bar.so", "ELF placeholder");
    common::write_file(root, "driver/server/baz.dll", "PE placeholder");

    let crates = reovim_depgraph::enumerate_crates(root).expect("enumeration must succeed");
    // Only the one real crate must appear.
    assert_eq!(
        crates.len(),
        1,
        "DAG4: install-layout .so/.dll files must not appear as crates; got {crates:?}"
    );
    assert_eq!(crates[0].name, "reovim-depgraph");
}
