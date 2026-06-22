//! Unit tests for the probe engine and TOML reader. Branch-complete per the
//! 100% line + MC/DC coverage policy (10.1 DEV1); the DAG1..DAG5 conformance
//! fixtures live in `tests/` as integration probes.
//!
//! # `TempDir` helper
//!
//! `TempDir` is a std-only temporary-directory helper that avoids the
//! `tempfile` crate (DAG5: no third-party deps). It allocates a unique path
//! from `std::env::temp_dir()` + process ID + an `AtomicUsize` counter
//! (guaranteed unique within a process; PID scopes across concurrent processes)
//! and removes the directory tree on `Drop`.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

use super::{
    Allowlist, AllowlistEntry, Allowlisted, Catalog, CatalogEdge, Category, Crate, DepEntry,
    DepTable, ProbeConfig, ProbeError, Report, Violation, allowed_categories, allowlist_match,
    check_edge, check_panic_profiles, classify, default_category_table, default_foundation_grants,
    has_no_std_attr, line_has_alloc_usage, line_has_std_usage, pattern_matches, run_dag6_probe,
    run_probe, strip_line_comment,
};

// ── TempDir helper ────────────────────────────────────────────────────────────

static COUNTER: AtomicUsize = AtomicUsize::new(0);

/// A temporary directory that is removed on `Drop`.
///
/// Constructed from `std::env::temp_dir()` + `"reovim-depgraph-"` + process ID
/// + an atomic counter to guarantee uniqueness within a process.
pub struct TempDir {
    path: PathBuf,
}

impl TempDir {
    /// Creates a new temporary directory.
    ///
    /// # Panics
    ///
    /// Panics when the directory cannot be created.
    #[must_use]
    pub fn new() -> Self {
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("reovim-depgraph-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&path).expect("TempDir: create_dir_all failed");
        Self { path }
    }

    /// Returns the path of the temporary directory.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.path
    }
}

impl Drop for TempDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

// ── TempDir tests ─────────────────────────────────────────────────────────────

#[test]
fn tempdir_creates_and_drop_removes() {
    let path: PathBuf;
    {
        let td = TempDir::new();
        path = td.path().to_path_buf();
        assert!(path.exists(), "TempDir should exist after creation");
    }
    assert!(!path.exists(), "TempDir should be removed after Drop");
}

#[test]
fn two_tempdirs_never_collide() {
    let a = TempDir::new();
    let b = TempDir::new();
    assert_ne!(a.path(), b.path(), "two TempDir instances must have distinct paths");
    assert!(a.path().exists());
    assert!(b.path().exists());
}

// ── helpers for engine tests ──────────────────────────────────────────────────

fn table() -> Vec<(String, Category)> {
    default_category_table()
}

fn krate(name: &str, path: &str, deps: &[(&str, DepTable, bool, bool)]) -> Crate {
    Crate {
        name: name.to_owned(),
        path: path.to_owned(),
        deps: deps
            .iter()
            .map(|(n, t, is_path, is_ws)| DepEntry {
                name: (*n).to_owned(),
                table: *t,
                is_path: *is_path,
                is_workspace_true: *is_ws,
                optional: false,
            })
            .collect(),
    }
}

fn krate_path_dep(name: &str, path: &str, dep_names: &[&str]) -> Crate {
    krate(
        name,
        path,
        &dep_names
            .iter()
            .map(|n| (*n, DepTable::Dependencies, true, false))
            .collect::<Vec<_>>(),
    )
}

fn empty_config() -> ProbeConfig {
    ProbeConfig {
        category_table: default_category_table(),
        foundation_grants: BTreeMap::new(),
        catalog: Catalog::default(),
        allowlist: Allowlist::default(),
    }
}

// ── pattern_matches tests ─────────────────────────────────────────────────────

#[test]
fn pattern_exact_prefix_matches_self_and_below() {
    assert!(pattern_matches("arch", "arch"));
    assert!(pattern_matches("arch", "arch/linux"));
    assert!(!pattern_matches("arch", "archive"));
    assert!(!pattern_matches("arch", "lib/depgraph"));
}

#[test]
fn pattern_wildcard_requires_extra_component() {
    assert!(pattern_matches("lib/*", "lib/depgraph"));
    assert!(pattern_matches("lib/*", "lib/depgraph/nested"));
    assert!(!pattern_matches("lib/*", "lib"));
    assert!(!pattern_matches("lib/*", "libs/depgraph"));
}

#[test]
fn pattern_shorter_path_never_matches() {
    assert!(!pattern_matches("server/lib/subsys/*", "server/lib"));
}

// ── category-table spec-mirror test ──────────────────────────────────────────

/// Asserts that `default_category_table()` entries exactly mirror the amended
/// 1.2 §1 category table — same path patterns and the four `ext/client/<cat>`
/// names (`platforms`, `driver`, `module`, `capabilities`). Spec drift fails
/// this test rather than a review round.
#[test]
fn category_table_mirrors_spec_1_2_section_1() {
    let table = default_category_table();

    // Helper: assert an exact pattern-category pair exists.
    let has = |pat: &str, cat: Category| table.iter().any(|(p, c)| p == pat && *c == cat);

    // Foundation paths (arch, lib/*, uapi/*).
    assert!(has("arch", Category::Foundation));
    assert!(has("lib/*", Category::Foundation));
    assert!(has("uapi/*", Category::Foundation));

    // Server tier.
    assert!(has("editor/lib/subsys/*", Category::ServerContracts));
    assert!(has("editor/lib/kernel/*", Category::ServerKernel));
    // Amended 1.2 §1: editor/lib/server/* = "Framed-protocol and dispatch glue" (ServerRuntime).
    assert!(has("editor/lib/server/*", Category::ServerRuntime));

    // Client contracts.
    assert!(has("client/lib/subsys/*", Category::ClientContracts));

    // Server extensions (four sub-paths).
    assert!(has("editor/modules/*", Category::ServerExt));
    assert!(has("editor/drivers/*", Category::ServerExt));
    assert!(has("editor/providers/*", Category::ServerExt));
    assert!(has("editor/domains/*", Category::ServerExt));

    // Client extensions — exactly four categories per amended 1.2 §1.
    assert!(has("client/platforms/*", Category::ClientExt));
    assert!(has("client/drivers/*", Category::ClientExt));
    assert!(has("client/modules/*", Category::ClientExt));
    assert!(has("client/capabilities/*", Category::ClientExt));

    // Composition roots and tools.
    assert!(has("apps/*", Category::Apps));
    assert!(has("tools/*", Category::Tools));
}

// ── default_foundation_grants tests ──────────────────────────────────────────

/// Asserts the foundation sub-DAG grants table is non-empty and contains the
/// `uapi/protocol → uapi/abi` grant added by Phase 1 of #786.
#[test]
fn foundation_grants_contains_uapi_protocol_to_abi() {
    let grants = default_foundation_grants();
    let proto_grants = grants
        .get("reovim-uapi-protocol")
        .expect("reovim-uapi-protocol must have a grants entry");
    assert!(
        proto_grants.iter().any(|g| g == "reovim-uapi-abi"),
        "reovim-uapi-protocol must be granted the reovim-uapi-abi dep"
    );
    let facade_grants = grants
        .get("reovim-uapi")
        .expect("reovim-uapi facade must have a grants entry");
    assert!(
        facade_grants.iter().any(|g| g == "reovim-uapi-net")
            && facade_grants.iter().all(|g| g != "reovim-uapi-posix"),
        "reovim-uapi facade must expose domain leaves but not reovim-uapi-posix"
    );
}

// ── classify tests ────────────────────────────────────────────────────────────

#[test]
fn classify_every_category() {
    let t = table();
    let cases = [
        ("arch", Category::Foundation),
        ("lib/depgraph", Category::Foundation),
        ("uapi", Category::Foundation),
        ("uapi/inner/protocol", Category::Foundation),
        ("editor/lib/subsys/mm", Category::ServerContracts),
        ("editor/lib/kernel/core", Category::ServerKernel),
        ("editor/lib/server/grpc", Category::ServerRuntime),
        ("client/lib/subsys/module", Category::ClientContracts),
        ("editor/modules/vim", Category::ServerExt),
        ("editor/drivers/display", Category::ServerExt),
        ("editor/providers/text", Category::ServerExt),
        ("editor/domains/text", Category::ServerExt),
        ("client/platforms/tui", Category::ClientExt),
        ("client/drivers/render", Category::ClientExt),
        ("client/modules/vim", Category::ClientExt),
        ("client/capabilities/cell", Category::ClientExt),
        ("apps/server", Category::Apps),
        ("tools/testing", Category::Tools),
    ];
    for (path, expected) in cases {
        assert_eq!(classify(path, &t).unwrap(), expected, "path {path}");
    }
}

/// Asserts that `editor/lib/server/*` maps to `ServerRuntime` (amended
/// 1.2 §1: "Framed-protocol and dispatch glue").
#[test]
fn server_runtime_category_is_editor_lib_server() {
    let t = table();
    assert_eq!(classify("editor/lib/server/dispatch", &t).unwrap(), Category::ServerRuntime,);
}

#[test]
fn classify_unknown_path_fails_dag1() {
    let err = classify("bogus/foo", &table()).unwrap_err();
    assert_eq!(
        err,
        Violation::UnknownPath {
            path: "bogus/foo".to_owned()
        }
    );
    assert!(err.to_string().contains("DAG1"));
}

#[test]
fn classify_ambiguous_path_fails_naming_both_candidates() {
    let mut t = table();
    t.push(("lib/depgraph".to_owned(), Category::Tools));
    let err = classify("lib/depgraph", &t).unwrap_err();
    let Violation::AmbiguousPath { path, candidates } = &err else {
        panic!("expected AmbiguousPath, got {err:?}");
    };
    assert_eq!(path, "lib/depgraph");
    assert_eq!(candidates.len(), 2);
    let rendered = err.to_string();
    assert!(rendered.contains("`lib/*` (foundation)"), "{rendered}");
    assert!(rendered.contains("`lib/depgraph` (tools)"), "{rendered}");
}

// ── empty-category and unknown-path synthetic workspace tests ─────────────────

/// A one-crate workspace where all categories except Foundation are empty must
/// classify clean (empty-category tolerance).
#[test]
fn single_foundation_crate_workspace_classifies_clean() {
    let td = TempDir::new();
    let root = td.path();
    let crate_dir = root.join("lib/single");
    std::fs::create_dir_all(&crate_dir).unwrap();
    std::fs::write(
        crate_dir.join("Cargo.toml"),
        "[package]\nname = \"reovim-single\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    // Create empty catalog and allowlist files.
    let probes_dir = root.join("tools/depgraph-probes");
    std::fs::create_dir_all(&probes_dir).unwrap();
    std::fs::write(probes_dir.join("composition-edges.toml"), "").unwrap();
    std::fs::write(probes_dir.join("transitional-allowlist.toml"), "").unwrap();

    let config = ProbeConfig::default_for(root).expect("config loads");
    let report = run_probe(root, &config).expect("probe runs");
    assert!(report.is_clean(), "single foundation crate must be clean: {}", report.summary());
    let count = report
        .category_counts
        .get(&Category::Foundation)
        .copied()
        .unwrap_or(0);
    assert_eq!(count, 1);
}

/// A crate at an unrecognised path must fail with `UnknownPath` (DAG1).
#[test]
fn unknown_path_crate_fails_dag1() {
    let td = TempDir::new();
    let root = td.path();
    let crate_dir = root.join("mystery/x");
    std::fs::create_dir_all(&crate_dir).unwrap();
    std::fs::write(
        crate_dir.join("Cargo.toml"),
        "[package]\nname = \"mystery-x\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();

    let config = ProbeConfig {
        category_table: default_category_table(),
        foundation_grants: BTreeMap::new(),
        catalog: Catalog::default(),
        allowlist: Allowlist::default(),
    };
    let report = run_probe(root, &config).expect("probe runs");
    assert!(!report.is_clean());
    let has_unknown = report
        .violations
        .iter()
        .any(|v| matches!(v, Violation::UnknownPath { path } if path == "mystery/x"));
    assert!(has_unknown, "expected UnknownPath for mystery/x: {}", report.summary());
}

// ── check_edge tests ──────────────────────────────────────────────────────────

#[test]
fn check_edge_foundation_without_grant_fails() {
    let config = empty_config();
    let from = krate_path_dep("reovim-depgraph", "lib/depgraph", &["reovim-arch"]);
    let violation =
        check_edge(&from, Category::Foundation, "reovim-arch", Category::Foundation, &config)
            .unwrap();
    assert_eq!(
        violation,
        Violation::UngrantedFoundationEdge {
            from: "reovim-depgraph".to_owned(),
            to: "reovim-arch".to_owned(),
        }
    );
    assert!(violation.to_string().contains("§6"));
}

#[test]
fn check_edge_foundation_with_grant_passes() {
    let mut config = empty_config();
    config
        .foundation_grants
        .insert("reovim-dylib-loader".to_owned(), vec!["reovim-arch".to_owned()]);
    let from = krate_path_dep("reovim-dylib-loader", "lib/dylib-loader", &["reovim-arch"]);
    assert!(
        check_edge(&from, Category::Foundation, "reovim-arch", Category::Foundation, &config)
            .is_none()
    );
}

#[test]
fn check_edge_foundation_grant_for_other_dep_still_fails() {
    let mut config = empty_config();
    config
        .foundation_grants
        .insert("reovim-depgraph".to_owned(), vec!["other".to_owned()]);
    let from = krate_path_dep("reovim-depgraph", "lib/depgraph", &["reovim-arch"]);
    assert!(
        check_edge(&from, Category::Foundation, "reovim-arch", Category::Foundation, &config)
            .is_some()
    );
}

#[test]
fn check_edge_allowed_matrix_cells_pass() {
    let config = empty_config();
    let cases = [
        (Category::ServerContracts, Category::Foundation),
        (Category::ClientContracts, Category::Foundation),
        (Category::ServerKernel, Category::ServerContracts),
        (Category::ServerKernel, Category::Foundation),
        (Category::ServerRuntime, Category::ServerKernel),
        (Category::ServerRuntime, Category::ServerContracts),
        (Category::ServerRuntime, Category::Foundation),
        (Category::ServerExt, Category::ServerContracts),
        (Category::ServerExt, Category::Foundation),
        (Category::ClientExt, Category::ClientContracts),
        (Category::ClientExt, Category::Foundation),
    ];
    for (from_category, to_category) in cases {
        let from = krate_path_dep("a", "x", &[]);
        assert!(
            check_edge(&from, from_category, "b", to_category, &config).is_none(),
            "{from_category} -> {to_category} should be allowed"
        );
    }
}

#[test]
fn check_edge_forbidden_matrix_cells_fail() {
    let config = empty_config();
    let cases = [
        (Category::ServerContracts, Category::ServerKernel),
        (Category::ServerContracts, Category::ServerContracts),
        (Category::ServerKernel, Category::ServerRuntime),
        (Category::ServerKernel, Category::ServerExt),
        (Category::ServerRuntime, Category::ServerExt),
        (Category::ServerRuntime, Category::Apps),
        (Category::ClientContracts, Category::ClientExt),
        (Category::ClientContracts, Category::ClientContracts),
        (Category::ServerExt, Category::ServerKernel),
        (Category::ServerExt, Category::ServerExt),
        (Category::ClientExt, Category::ServerKernel),
        (Category::ClientExt, Category::ClientExt),
    ];
    for (from_category, to_category) in cases {
        let from = krate_path_dep("a", "x", &[]);
        let violation = check_edge(&from, from_category, "b", to_category, &config)
            .unwrap_or_else(|| panic!("{from_category} -> {to_category} should fail"));
        assert!(violation.to_string().contains("DAG2"));
    }
}

#[test]
fn check_edge_composition_requires_catalog_entry() {
    let mut config = empty_config();
    let from = krate_path_dep("reovim", "apps/reovim", &["reovim-tui"]);
    let violation =
        check_edge(&from, Category::Apps, "reovim-tui", Category::Apps, &config).unwrap();
    assert!(violation.to_string().contains("DAG3"));

    // Decoys: a from-mismatch and a to-mismatch entry must not admit the edge.
    config.catalog.edge.push(CatalogEdge {
        from: "other-app".to_owned(),
        to: "reovim-tui".to_owned(),
        gate: None,
        reason: "decoy".to_owned(),
    });
    config.catalog.edge.push(CatalogEdge {
        from: "reovim".to_owned(),
        to: "other-lib".to_owned(),
        gate: None,
        reason: "decoy".to_owned(),
    });
    assert!(check_edge(&from, Category::Apps, "reovim-tui", Category::Apps, &config).is_some());
    config.catalog.edge.push(CatalogEdge {
        from: "reovim".to_owned(),
        to: "reovim-tui".to_owned(),
        gate: Some("embedded-tui".to_owned()),
        reason: "embedded default".to_owned(),
    });
    assert!(check_edge(&from, Category::Apps, "reovim-tui", Category::Apps, &config).is_none());
}

#[test]
fn check_edge_tools_use_the_same_catalog_discipline() {
    let config = empty_config();
    let from = krate_path_dep("testing", "tools/testing", &["reovim-depgraph"]);
    let violation =
        check_edge(&from, Category::Tools, "reovim-depgraph", Category::Foundation, &config)
            .unwrap();
    assert_eq!(
        violation,
        Violation::UncatalogedCompositionEdge {
            from: "testing".to_owned(),
            to: "reovim-depgraph".to_owned(),
        }
    );
}

// ── allowlist tests ───────────────────────────────────────────────────────────

#[test]
fn allowlist_suppresses_edge_violations_only() {
    let allowlist = Allowlist {
        entry: vec![
            AllowlistEntry {
                from: "z".to_owned(),
                to: "b".to_owned(),
                reason: "decoy".to_owned(),
                issue: "#0".to_owned(),
                expires: "never".to_owned(),
            },
            AllowlistEntry {
                from: "a".to_owned(),
                to: "b".to_owned(),
                reason: "transition".to_owned(),
                issue: "#0".to_owned(),
                expires: "never".to_owned(),
            },
        ],
    };
    let edge = Violation::ForbiddenEdge {
        from: "a".to_owned(),
        from_category: Category::ServerExt,
        to: "b".to_owned(),
        to_category: Category::ServerKernel,
    };
    assert!(allowlist_match(&edge, &allowlist).is_some());

    let foundation = Violation::UngrantedFoundationEdge {
        from: "a".to_owned(),
        to: "b".to_owned(),
    };
    assert!(allowlist_match(&foundation, &allowlist).is_some());

    let composition = Violation::UncatalogedCompositionEdge {
        from: "a".to_owned(),
        to: "b".to_owned(),
    };
    let matched = allowlist_match(&composition, &allowlist).unwrap();
    assert_eq!(matched.issue, "#0");
    assert_eq!(matched.expires, "never");

    let other_edge = Violation::UngrantedFoundationEdge {
        from: "a".to_owned(),
        to: "c".to_owned(),
    };
    assert!(allowlist_match(&other_edge, &allowlist).is_none());

    let unknown = Violation::UnknownPath {
        path: "a".to_owned(),
    };
    assert!(allowlist_match(&unknown, &allowlist).is_none());

    let ambiguous = Violation::AmbiguousPath {
        path: "a".to_owned(),
        candidates: vec![],
    };
    assert!(allowlist_match(&ambiguous, &allowlist).is_none());

    let sovereign = Violation::NonSovereignDep {
        crate_path: "a".to_owned(),
        table: DepTable::Dependencies,
        dep: "b".to_owned(),
    };
    assert!(allowlist_match(&sovereign, &allowlist).is_none());
}

// ── schema validation tests ───────────────────────────────────────────────────

/// §7: catalog entries must stay reviewable — blank `reason` or `gate` fields
/// fail validation instead of passing silently.
#[test]
fn catalog_schema_validation() {
    let path = Path::new("catalog.toml");
    let edge = |gate: Option<&str>, reason: &str| CatalogEdge {
        from: "a".to_owned(),
        to: "b".to_owned(),
        gate: gate.map(str::to_owned),
        reason: reason.to_owned(),
    };
    let valid = Catalog {
        edge: vec![edge(None, "wired"), edge(Some("feat"), "gated")],
    };
    assert!(valid.validate(path).is_ok());

    let blank_reason = Catalog {
        edge: vec![edge(None, "  ")],
    };
    let err = blank_reason.validate(path).unwrap_err();
    assert!(err.to_string().contains("empty reason"), "{err}");

    let blank_gate = Catalog {
        edge: vec![edge(Some(""), "wired")],
    };
    let err = blank_gate.validate(path).unwrap_err();
    assert!(err.to_string().contains("empty gate"), "{err}");
}

/// §8: an allowlist entry with a blank `reason`, `issue`, or `expires` is an
/// untracked hole — validation rejects each case.
#[test]
fn allowlist_schema_validation() {
    let path = Path::new("allowlist.toml");
    let entry = |reason: &str, issue: &str, expires: &str| AllowlistEntry {
        from: "a".to_owned(),
        to: "b".to_owned(),
        reason: reason.to_owned(),
        issue: issue.to_owned(),
        expires: expires.to_owned(),
    };
    assert!(
        Allowlist {
            entry: vec![entry("transition", "#1", "v1.0")]
        }
        .validate(path)
        .is_ok()
    );
    for (case, field) in [
        (entry("", "#1", "v1.0"), "reason"),
        (entry("transition", " ", "v1.0"), "issue"),
        (entry("transition", "#1", ""), "expires"),
    ] {
        let err = Allowlist { entry: vec![case] }.validate(path).unwrap_err();
        assert!(err.to_string().contains(&format!("empty {field}")), "{err}");
    }
}

// ── DAG5 sovereignty in run_probe ─────────────────────────────────────────────

/// A synthetic workspace with a registry dep must produce `NonSovereignDep`.
#[test]
fn run_probe_detects_registry_dep_as_non_sovereign() {
    let td = TempDir::new();
    let root = td.path();
    let crate_dir = root.join("lib/foo");
    std::fs::create_dir_all(&crate_dir).unwrap();
    std::fs::write(
        crate_dir.join("Cargo.toml"),
        "[package]\nname = \"reovim-foo\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\nserde = \"1.0\"\n",
    )
    .unwrap();

    let config = ProbeConfig {
        category_table: default_category_table(),
        foundation_grants: BTreeMap::new(),
        catalog: Catalog::default(),
        allowlist: Allowlist::default(),
    };
    let report = run_probe(root, &config).expect("probe runs");
    let has_dag5 = report.violations.iter().any(|v| {
        matches!(
            v,
            Violation::NonSovereignDep { dep, table: DepTable::Dependencies, .. }
            if dep == "serde"
        )
    });
    assert!(has_dag5, "expected NonSovereignDep for serde: {}", report.summary());
}

/// A `{ workspace = true }` dep is also a DAG5 violation.
#[test]
fn run_probe_detects_workspace_true_dep_as_non_sovereign() {
    let td = TempDir::new();
    let root = td.path();
    let crate_dir = root.join("lib/bar");
    std::fs::create_dir_all(&crate_dir).unwrap();
    std::fs::write(
        crate_dir.join("Cargo.toml"),
        "[package]\nname = \"reovim-bar\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\nserde = { workspace = true }\n",
    )
    .unwrap();

    let config = ProbeConfig {
        category_table: default_category_table(),
        foundation_grants: BTreeMap::new(),
        catalog: Catalog::default(),
        allowlist: Allowlist::default(),
    };
    let report = run_probe(root, &config).expect("probe runs");
    let has_dag5 = report
        .violations
        .iter()
        .any(|v| matches!(v, Violation::NonSovereignDep { dep, .. } if dep == "serde"));
    assert!(
        has_dag5,
        "expected NonSovereignDep for workspace=true dep: {}",
        report.summary()
    );
}

// ── lib.rs 405 / 412: Catalog::default() / Allowlist::default() when files absent ─

/// `ProbeConfig::default_for` returns empty catalog and allowlist when neither
/// `tools/depgraph-probes` file exists (lines 405, 412).
#[test]
fn default_for_absent_probe_files_gives_empty_tables() {
    let td = TempDir::new();
    let root = td.path();
    // No tools/depgraph-probes directory — both files are absent.
    let config = ProbeConfig::default_for(root).expect("default_for succeeds with absent files");
    assert!(config.catalog.edge.is_empty());
    assert!(config.allowlist.entry.is_empty());
}

// ── lib.rs 430-439: load_catalog with non-empty [[edge]] entries ──────────────

/// A catalog file with one `[[edge]]` (no `gate`) must parse all three required
/// fields (lines 430-432) and leave `gate` as `None` (line 433 None branch).
#[test]
fn load_catalog_edge_without_gate() {
    let td = TempDir::new();
    let root = td.path();
    let probes_dir = root.join("tools/depgraph-probes");
    std::fs::create_dir_all(&probes_dir).unwrap();
    std::fs::write(
        probes_dir.join("composition-edges.toml"),
        "[[edge]]\nfrom = \"reovim\"\nto = \"reovim-tui\"\nreason = \"embedded\"\n",
    )
    .unwrap();
    std::fs::write(probes_dir.join("transitional-allowlist.toml"), "").unwrap();

    let config = ProbeConfig::default_for(root).expect("catalog loads");
    assert_eq!(config.catalog.edge.len(), 1);
    let edge = &config.catalog.edge[0];
    assert_eq!(edge.from, "reovim");
    assert_eq!(edge.to, "reovim-tui");
    assert_eq!(edge.reason, "embedded");
    assert!(edge.gate.is_none());
}

/// A catalog file with one `[[edge]]` that has a `gate` field must parse it
/// into `Some(...)` (line 433 Some branch, lines 434-439).
#[test]
fn load_catalog_edge_with_gate() {
    let td = TempDir::new();
    let root = td.path();
    let probes_dir = root.join("tools/depgraph-probes");
    std::fs::create_dir_all(&probes_dir).unwrap();
    std::fs::write(
        probes_dir.join("composition-edges.toml"),
        "[[edge]]\nfrom = \"reovim\"\nto = \"reovim-tui\"\nreason = \"embedded\"\ngate = \"embedded-tui\"\n",
    )
    .unwrap();
    std::fs::write(probes_dir.join("transitional-allowlist.toml"), "").unwrap();

    let config = ProbeConfig::default_for(root).expect("catalog loads");
    assert_eq!(config.catalog.edge.len(), 1);
    assert_eq!(config.catalog.edge[0].gate.as_deref(), Some("embedded-tui"));
}

// ── lib.rs 448-459: load_allowlist with non-empty [[entry]] ──────────────────

/// A transitional-allowlist file with one `[[entry]]` (all five required
/// fields) must be loaded into `AllowlistEntry` (lines 448-459).
#[test]
fn load_allowlist_entry_all_fields() {
    let td = TempDir::new();
    let root = td.path();
    let probes_dir = root.join("tools/depgraph-probes");
    std::fs::create_dir_all(&probes_dir).unwrap();
    std::fs::write(probes_dir.join("composition-edges.toml"), "").unwrap();
    std::fs::write(
        probes_dir.join("transitional-allowlist.toml"),
        "[[entry]]\nfrom = \"ext-crate\"\nto = \"core-crate\"\nreason = \"migration\"\nissue = \"#99\"\nexpires = \"v1.0\"\n",
    )
    .unwrap();

    let config = ProbeConfig::default_for(root).expect("allowlist loads");
    assert_eq!(config.allowlist.entry.len(), 1);
    let e = &config.allowlist.entry[0];
    assert_eq!(e.from, "ext-crate");
    assert_eq!(e.to, "core-crate");
    assert_eq!(e.reason, "migration");
    assert_eq!(e.issue, "#99");
    assert_eq!(e.expires, "v1.0");
}

// ── lib.rs 464-476: get_required_str — missing-field error paths ──────────────

/// Missing `from` in a catalog `[[edge]]` must return a `ProbeError::Parse`
/// mentioning `from` (exercises `get_required_str` for the first field, line
/// 430 / 464-476).
#[test]
fn load_catalog_missing_from_field_is_parse_error() {
    let td = TempDir::new();
    let root = td.path();
    let probes_dir = root.join("tools/depgraph-probes");
    std::fs::create_dir_all(&probes_dir).unwrap();
    std::fs::write(
        probes_dir.join("composition-edges.toml"),
        "[[edge]]\nto = \"reovim-tui\"\nreason = \"embedded\"\n",
    )
    .unwrap();
    std::fs::write(probes_dir.join("transitional-allowlist.toml"), "").unwrap();

    let err = ProbeConfig::default_for(root).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("missing required field"), "{msg}");
    assert!(msg.contains("`from`"), "{msg}");
}

/// Missing `to` in a catalog `[[edge]]` must return a `ProbeError::Parse`
/// mentioning `to`.
#[test]
fn load_catalog_missing_to_field_is_parse_error() {
    let td = TempDir::new();
    let root = td.path();
    let probes_dir = root.join("tools/depgraph-probes");
    std::fs::create_dir_all(&probes_dir).unwrap();
    std::fs::write(
        probes_dir.join("composition-edges.toml"),
        "[[edge]]\nfrom = \"reovim\"\nreason = \"embedded\"\n",
    )
    .unwrap();
    std::fs::write(probes_dir.join("transitional-allowlist.toml"), "").unwrap();

    let err = ProbeConfig::default_for(root).unwrap_err();
    assert!(err.to_string().contains("`to`"), "{err}");
}

/// Missing `reason` in a catalog `[[edge]]` must return a `ProbeError::Parse`
/// mentioning `reason`.
#[test]
fn load_catalog_missing_reason_field_is_parse_error() {
    let td = TempDir::new();
    let root = td.path();
    let probes_dir = root.join("tools/depgraph-probes");
    std::fs::create_dir_all(&probes_dir).unwrap();
    std::fs::write(
        probes_dir.join("composition-edges.toml"),
        "[[edge]]\nfrom = \"reovim\"\nto = \"reovim-tui\"\n",
    )
    .unwrap();
    std::fs::write(probes_dir.join("transitional-allowlist.toml"), "").unwrap();

    let err = ProbeConfig::default_for(root).unwrap_err();
    assert!(err.to_string().contains("`reason`"), "{err}");
}

/// Missing `from` in an allowlist `[[entry]]` must return a `ProbeError::Parse`
/// mentioning `from`.
#[test]
fn load_allowlist_missing_from_field_is_parse_error() {
    let td = TempDir::new();
    let root = td.path();
    let probes_dir = root.join("tools/depgraph-probes");
    std::fs::create_dir_all(&probes_dir).unwrap();
    std::fs::write(probes_dir.join("composition-edges.toml"), "").unwrap();
    std::fs::write(
        probes_dir.join("transitional-allowlist.toml"),
        "[[entry]]\nto = \"core\"\nreason = \"r\"\nissue = \"#1\"\nexpires = \"v1\"\n",
    )
    .unwrap();

    let err = ProbeConfig::default_for(root).unwrap_err();
    assert!(err.to_string().contains("`from`"), "{err}");
}

/// Missing `issue` in an allowlist `[[entry]]` must return a `ProbeError::Parse`
/// mentioning `issue`.
#[test]
fn load_allowlist_missing_issue_field_is_parse_error() {
    let td = TempDir::new();
    let root = td.path();
    let probes_dir = root.join("tools/depgraph-probes");
    std::fs::create_dir_all(&probes_dir).unwrap();
    std::fs::write(probes_dir.join("composition-edges.toml"), "").unwrap();
    std::fs::write(
        probes_dir.join("transitional-allowlist.toml"),
        "[[entry]]\nfrom = \"ext\"\nto = \"core\"\nreason = \"r\"\nexpires = \"v1\"\n",
    )
    .unwrap();

    let err = ProbeConfig::default_for(root).unwrap_err();
    assert!(err.to_string().contains("`issue`"), "{err}");
}

/// Missing `expires` in an allowlist `[[entry]]` must return a
/// `ProbeError::Parse` mentioning `expires`.
#[test]
fn load_allowlist_missing_expires_field_is_parse_error() {
    let td = TempDir::new();
    let root = td.path();
    let probes_dir = root.join("tools/depgraph-probes");
    std::fs::create_dir_all(&probes_dir).unwrap();
    std::fs::write(probes_dir.join("composition-edges.toml"), "").unwrap();
    std::fs::write(
        probes_dir.join("transitional-allowlist.toml"),
        "[[entry]]\nfrom = \"ext\"\nto = \"core\"\nreason = \"r\"\nissue = \"#1\"\n",
    )
    .unwrap();

    let err = ProbeConfig::default_for(root).unwrap_err();
    assert!(err.to_string().contains("`expires`"), "{err}");
}

// ── lib.rs 556-558: io_error closure body ────────────────────────────────────

/// Attempting to enumerate crates rooted at a non-existent path must return
/// `ProbeError::Io` (exercises the `io_error` closure body, lines 556-558).
#[test]
fn enumerate_crates_on_missing_root_returns_io_error() {
    use super::enumerate_crates;
    let path = std::path::Path::new("/tmp/reovim-depgraph-nonexistent-dir-guaranteed-absent");
    let err = enumerate_crates(path).unwrap_err();
    assert!(matches!(err, ProbeError::Io { .. }), "expected Io error, got {err:?}");
}

// ── lib.rs 598: parse_manifest returns None for virtual workspace manifest ────

/// A `Cargo.toml` with no `[package]` section (virtual workspace manifest)
/// must be silently skipped — `parse_manifest` returns `Ok(None)` (line 598).
#[test]
fn enumerate_crates_skips_virtual_manifest_without_package_section() {
    use super::enumerate_crates;
    let td = TempDir::new();
    let root = td.path();
    let sub_dir = root.join("lib/virtual");
    std::fs::create_dir_all(&sub_dir).unwrap();
    // Virtual workspace manifest: has [workspace] but no [package].
    std::fs::write(sub_dir.join("Cargo.toml"), "[workspace]\nmembers = []\n").unwrap();

    let crates = enumerate_crates(root).expect("enumerate succeeds");
    assert!(crates.is_empty(), "virtual manifest must be skipped; got {crates:?}");
}

// ── lib.rs 604-606: parse_manifest missing [package].name parse error ─────────

/// A `[package]` section without a `name` key must yield `ProbeError::Parse`
/// mentioning "missing [package].name" (lines 604-606).
#[test]
fn enumerate_crates_manifest_without_name_is_parse_error() {
    use super::enumerate_crates;
    let td = TempDir::new();
    let root = td.path();
    let sub_dir = root.join("lib/noname");
    std::fs::create_dir_all(&sub_dir).unwrap();
    std::fs::write(sub_dir.join("Cargo.toml"), "[package]\nversion = \"0.1.0\"\n").unwrap();

    let err = enumerate_crates(root).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("missing [package].name"), "{msg}");
}

// ── lib.rs 667-670: dep_entry_from_toml_value Bool arm ───────────────────────

/// A dependency declared as a bare boolean (`dep = true`) must yield
/// `ProbeError::Parse` mentioning the dep name (lines 667-670).
#[test]
fn enumerate_crates_bool_dep_value_is_parse_error() {
    use super::enumerate_crates;
    let td = TempDir::new();
    let root = td.path();
    let sub_dir = root.join("lib/booldep");
    std::fs::create_dir_all(&sub_dir).unwrap();
    std::fs::write(
        sub_dir.join("Cargo.toml"),
        "[package]\nname = \"reovim-booldep\"\nversion = \"0.1.0\"\n\n[dependencies]\nsome-dep = true\n",
    )
    .unwrap();

    let err = enumerate_crates(root).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("some-dep"), "{msg}");
    assert!(msg.contains("unexpected scalar"), "{msg}");
}

/// A dependency declared as a bare array (`dep = ["x"]`) must yield
/// `ProbeError::Parse` mentioning the dep name (the `Array` arm of
/// `dep_entry_from_toml_value`).
#[test]
fn enumerate_crates_array_dep_value_is_parse_error() {
    use super::enumerate_crates;
    let td = TempDir::new();
    let root = td.path();
    let sub_dir = root.join("lib/arraydep");
    std::fs::create_dir_all(&sub_dir).unwrap();
    std::fs::write(
        sub_dir.join("Cargo.toml"),
        "[package]\nname = \"reovim-arraydep\"\nversion = \"0.1.0\"\n\n[dependencies]\nsome-dep = [\"x\"]\n",
    )
    .unwrap();

    let err = enumerate_crates(root).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("some-dep"), "{msg}");
    assert!(msg.contains("unexpected array"), "{msg}");
}

// ── lib.rs 800-804: allowlist suppression path (Allowlisted records) ──────────

/// An uncatalogued composition edge covered by an allowlist entry must appear
/// in `report.allowlisted` (not `report.violations`), and the `Allowlisted`
/// record must carry the entry's `issue` and `expires` (lines 800-804).
#[test]
fn run_probe_allowlist_suppresses_uncataloged_composition_edge() {
    let td = TempDir::new();
    let root = td.path();

    // One Apps crate depending on another Apps crate — an uncataloged
    // composition edge (DAG3 violation without a catalog entry).
    let crate_a = root.join("apps/alpha");
    let crate_b = root.join("apps/beta");
    std::fs::create_dir_all(&crate_a).unwrap();
    std::fs::create_dir_all(&crate_b).unwrap();
    std::fs::write(
        crate_a.join("Cargo.toml"),
        "[package]\nname = \"reovim-alpha\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\nreovim-beta = { path = \"../beta\" }\n",
    )
    .unwrap();
    std::fs::write(
        crate_b.join("Cargo.toml"),
        "[package]\nname = \"reovim-beta\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();

    // Allowlist entry suppressing exactly that edge.
    let allowlist = Allowlist {
        entry: vec![AllowlistEntry {
            from: "reovim-alpha".to_owned(),
            to: "reovim-beta".to_owned(),
            reason: "transitional wiring".to_owned(),
            issue: "#999".to_owned(),
            expires: "v2.0".to_owned(),
        }],
    };
    let config = ProbeConfig {
        category_table: default_category_table(),
        foundation_grants: BTreeMap::new(),
        catalog: Catalog::default(),
        allowlist,
    };

    let report = run_probe(root, &config).expect("probe runs");

    // The violation must be suppressed, not live.
    assert!(
        report.is_clean(),
        "allowlisted violation must not appear in violations: {}",
        report.summary()
    );
    // It must appear in allowlisted with the correct tracking data.
    assert_eq!(report.allowlisted.len(), 1, "expected one allowlisted record");
    let suppressed = &report.allowlisted[0];
    assert_eq!(suppressed.issue, "#999");
    assert_eq!(suppressed.expires, "v2.0");
    assert!(
        matches!(
            &suppressed.violation,
            Violation::UncatalogedCompositionEdge { from, to }
            if from == "reovim-alpha" && to == "reovim-beta"
        ),
        "unexpected suppressed violation: {:?}",
        suppressed.violation
    );
}

/// An in-repo `path = "..."` dep is sovereign and must NOT produce a violation.
#[test]
fn run_probe_path_dep_is_sovereign() {
    let td = TempDir::new();
    let root = td.path();
    let crate_a = root.join("lib/a");
    let crate_b = root.join("lib/b");
    std::fs::create_dir_all(&crate_a).unwrap();
    std::fs::create_dir_all(&crate_b).unwrap();
    std::fs::write(
        crate_a.join("Cargo.toml"),
        "[package]\nname = \"reovim-a\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\nreovim-b = { path = \"../b\" }\n",
    )
    .unwrap();
    std::fs::write(
        crate_b.join("Cargo.toml"),
        "[package]\nname = \"reovim-b\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();

    // Give reovim-a a foundation grant for reovim-b so the edge is clean.
    let mut grants = BTreeMap::new();
    grants.insert("reovim-a".to_owned(), vec!["reovim-b".to_owned()]);
    let config = ProbeConfig {
        category_table: default_category_table(),
        foundation_grants: grants,
        catalog: Catalog::default(),
        allowlist: Allowlist::default(),
    };
    let report = run_probe(root, &config).expect("probe runs");
    let has_dag5 = report
        .violations
        .iter()
        .any(|v| matches!(v, Violation::NonSovereignDep { .. }));
    assert!(!has_dag5, "path dep must not trigger DAG5: {}", report.summary());
}

// ── misc display/debug tests ──────────────────────────────────────────────────

#[test]
fn matrix_rows_outside_check_edge_dispatch_are_empty() {
    for category in [Category::Foundation, Category::Apps, Category::Tools] {
        assert!(allowed_categories(category).is_empty(), "{category}");
    }
}

#[test]
fn category_display_names_are_stable() {
    let all = [
        (Category::Foundation, "foundation"),
        (Category::ServerContracts, "server-contracts"),
        (Category::ServerKernel, "server-kernel"),
        (Category::ServerRuntime, "server-runtime"),
        (Category::ClientContracts, "client-contracts"),
        (Category::ServerExt, "server-ext"),
        (Category::ClientExt, "client-ext"),
        (Category::Apps, "apps"),
        (Category::Tools, "tools"),
    ];
    for (category, name) in all {
        assert_eq!(category.to_string(), name);
    }
}

#[test]
fn dep_table_display_names_are_stable() {
    assert_eq!(DepTable::Dependencies.to_string(), "dependencies");
    assert_eq!(DepTable::DevDependencies.to_string(), "dev-dependencies");
    assert_eq!(DepTable::BuildDependencies.to_string(), "build-dependencies");
}

#[test]
fn report_summary_renders_counts_allowlisted_and_violations() {
    let mut report = Report::default();
    assert!(report.is_clean());
    report.category_counts.insert(Category::Foundation, 1);
    report.allowlisted.push(Allowlisted {
        violation: Violation::UngrantedFoundationEdge {
            from: "a".to_owned(),
            to: "b".to_owned(),
        },
        issue: "#0".to_owned(),
        expires: "v1".to_owned(),
    });
    report.violations.push(Violation::UnknownPath {
        path: "bogus/foo".to_owned(),
    });
    assert!(!report.is_clean());
    let summary = report.summary();
    assert!(summary.contains("foundation: 1 crate(s)"));
    assert!(summary.contains("allowlisted (#0, expires v1):"));
    assert!(summary.contains("VIOLATION: DAG1"));
}

#[test]
fn probe_error_display_covers_both_variants() {
    let io = ProbeError::Io {
        path: PathBuf::from("/x"),
        source: std::io::Error::other("boom"),
    };
    assert!(io.to_string().contains("io error at `/x`"));
    let parse = ProbeError::Parse {
        path: PathBuf::from("/y"),
        message: "bad".to_owned(),
    };
    assert!(parse.to_string().contains("parse error at `/y`"));
}

#[test]
fn violation_dag5_display_contains_dag5() {
    let v = Violation::NonSovereignDep {
        crate_path: "lib/foo".to_owned(),
        table: DepTable::DevDependencies,
        dep: "tempfile".to_owned(),
    };
    let s = v.to_string();
    assert!(s.contains("DAG5"), "{s}");
    assert!(s.contains("tempfile"), "{s}");
    assert!(s.contains("dev-dependencies"), "{s}");
}

#[test]
fn dep_table_all_variants_are_covered() {
    // Confirm the three DepTable variants have distinct Debug representations.
    let tables = [
        DepTable::Dependencies,
        DepTable::DevDependencies,
        DepTable::BuildDependencies,
    ];
    let names: Vec<_> = tables.iter().map(DepTable::to_string).collect();
    assert_eq!(names, ["dependencies", "dev-dependencies", "build-dependencies"]);
}

// ── Catalog / Allowlist validate() with non-empty entries ─────────────────────

fn catalog_edge(reason: &str, gate: Option<&str>) -> Catalog {
    Catalog {
        edge: vec![CatalogEdge {
            from: "a".to_owned(),
            to: "b".to_owned(),
            gate: gate.map(str::to_owned),
            reason: reason.to_owned(),
        }],
    }
}

#[test]
fn catalog_validate_accepts_well_formed_edge() {
    let p = Path::new("composition-edges.toml");
    catalog_edge("embedded boot", Some("embedded-tui"))
        .validate(p)
        .unwrap();
    catalog_edge("embedded boot", None).validate(p).unwrap();
}

#[test]
fn catalog_validate_rejects_blank_reason() {
    let p = Path::new("composition-edges.toml");
    let msg = catalog_edge("  ", None)
        .validate(p)
        .unwrap_err()
        .to_string();
    assert!(msg.contains("empty reason"), "{msg}");
}

#[test]
fn catalog_validate_rejects_blank_gate() {
    let p = Path::new("composition-edges.toml");
    let msg = catalog_edge("ok", Some(" "))
        .validate(p)
        .unwrap_err()
        .to_string();
    assert!(msg.contains("empty gate"), "{msg}");
}

fn allowlist_entry(reason: &str, issue: &str, expires: &str) -> Allowlist {
    Allowlist {
        entry: vec![AllowlistEntry {
            from: "a".to_owned(),
            to: "b".to_owned(),
            reason: reason.to_owned(),
            issue: issue.to_owned(),
            expires: expires.to_owned(),
        }],
    }
}

#[test]
fn allowlist_validate_accepts_well_formed_entry() {
    let p = Path::new("transitional-allowlist.toml");
    allowlist_entry("pending move", "#775", "v4.0-rc1")
        .validate(p)
        .unwrap();
}

#[test]
fn allowlist_validate_rejects_each_blank_field() {
    let p = Path::new("transitional-allowlist.toml");
    for (case, field) in [
        (allowlist_entry(" ", "#1", "v1"), "empty reason"),
        (allowlist_entry("r", " ", "v1"), "empty issue"),
        (allowlist_entry("r", "#1", " "), "empty expires"),
    ] {
        let msg = case.validate(p).unwrap_err().to_string();
        assert!(msg.contains(field), "{msg}");
    }
}

// ── DAG6 unit tests ───────────────────────────────────────────────────────────

// strip_line_comment ──────────────────────────────────────────────────────────

#[test]
fn strip_line_comment_no_comment() {
    assert_eq!(strip_line_comment("use std::fmt;"), "use std::fmt;");
}

#[test]
fn strip_line_comment_with_comment() {
    assert_eq!(strip_line_comment("use std::fmt; // comment"), "use std::fmt; ");
}

#[test]
fn strip_line_comment_double_slash_inside_string_not_stripped() {
    // A `//` inside a string literal is NOT a comment.
    let line = r#"let s = "url://example"; // real comment"#;
    let result = strip_line_comment(line);
    assert!(result.contains("url://example"), "string content preserved: {result}");
    assert!(!result.contains("real comment"), "trailing comment stripped: {result}");
}

#[test]
fn strip_line_comment_escaped_char_in_string() {
    // A `\"` inside a string; the `//` after the closing quote is the comment.
    let line = r#"let s = "a\"b"; // note"#;
    let result = strip_line_comment(line);
    assert!(!result.contains("note"), "comment stripped: {result}");
    assert!(result.contains("a\\\"b"), "string preserved: {result}");
}

#[test]
fn strip_line_comment_single_slash_not_a_comment() {
    // A single `/` is not a comment start.
    assert_eq!(strip_line_comment("a/b"), "a/b");
}

// has_no_std_attr ─────────────────────────────────────────────────────────────

#[test]
fn has_no_std_attr_present() {
    assert!(has_no_std_attr("#![no_std]\n\npub mod foo;\n"));
}

#[test]
fn has_no_std_attr_absent() {
    assert!(!has_no_std_attr("// no_std missing\npub mod foo;\n"));
}

#[test]
fn has_no_std_attr_commented_out_is_absent() {
    // `#![no_std]` inside a line comment must NOT count.
    assert!(!has_no_std_attr("// #![no_std]\npub mod foo;\n"));
}

#[test]
fn has_no_std_attr_not_at_top_still_found() {
    // The scanner accepts `#![no_std]` anywhere in the file.
    assert!(has_no_std_attr("// preamble\n\n#![no_std]\n"));
}

// line_has_std_usage / line_has_alloc_usage ───────────────────────────────────

#[test]
fn line_has_std_usage_extern_crate() {
    assert!(line_has_std_usage("extern crate std;"));
}

#[test]
fn line_has_std_usage_use_std() {
    assert!(line_has_std_usage("use std::fmt::Write;"));
}

#[test]
fn line_has_std_usage_negative() {
    assert!(!line_has_std_usage("use core::fmt::Write;"));
}

#[test]
fn line_has_alloc_usage_extern_crate() {
    assert!(line_has_alloc_usage("extern crate alloc;"));
}

#[test]
fn line_has_alloc_usage_use_alloc() {
    assert!(line_has_alloc_usage("use alloc::vec::Vec;"));
}

#[test]
fn line_has_alloc_usage_negative() {
    assert!(!line_has_alloc_usage("use core::alloc::Layout;"));
}

// check_panic_profiles ────────────────────────────────────────────────────────

/// Both profiles set `panic = "abort"` → no violations.
#[test]
fn check_panic_profiles_both_abort_is_clean() {
    let td = TempDir::new();
    let root = td.path();
    std::fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = []\n\n[profile.dev]\npanic = \"abort\"\n\n[profile.release]\npanic = \"abort\"\n",
    )
    .unwrap();
    let v = check_panic_profiles(root).expect("check runs");
    assert!(v.is_empty(), "both abort → clean; got {v:?}");
}

/// `[profile.dev]` absent → `PanicProfileNotAbort { profile: "dev" }`.
#[test]
fn check_panic_profiles_dev_absent_is_violation() {
    let td = TempDir::new();
    let root = td.path();
    std::fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = []\n\n[profile.release]\npanic = \"abort\"\n",
    )
    .unwrap();
    let v = check_panic_profiles(root).expect("check runs");
    let has = v
        .iter()
        .any(|v| matches!(v, Violation::PanicProfileNotAbort { profile } if profile == "dev"));
    assert!(has, "missing [profile.dev] → violation; got {v:?}");
}

/// `[profile.release]` absent → `PanicProfileNotAbort { profile: "release" }`.
#[test]
fn check_panic_profiles_release_absent_is_violation() {
    let td = TempDir::new();
    let root = td.path();
    std::fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = []\n\n[profile.dev]\npanic = \"abort\"\n",
    )
    .unwrap();
    let v = check_panic_profiles(root).expect("check runs");
    let has = v
        .iter()
        .any(|v| matches!(v, Violation::PanicProfileNotAbort { profile } if profile == "release"));
    assert!(has, "missing [profile.release] → violation; got {v:?}");
}

/// `panic = "unwind"` in both → two violations.
#[test]
fn check_panic_profiles_unwind_produces_violations() {
    let td = TempDir::new();
    let root = td.path();
    std::fs::write(
        root.join("Cargo.toml"),
        "[workspace]\nmembers = []\n\n[profile.dev]\npanic = \"unwind\"\n\n[profile.release]\npanic = \"unwind\"\n",
    )
    .unwrap();
    let v = check_panic_profiles(root).expect("check runs");
    assert_eq!(v.len(), 2, "unwind in both → 2 violations; got {v:?}");
}

/// Missing root Cargo.toml → `ProbeError::Io`.
#[test]
fn check_panic_profiles_missing_file_returns_io_error() {
    let td = TempDir::new();
    let root = td.path();
    // No Cargo.toml created.
    let err = check_panic_profiles(root).unwrap_err();
    assert!(matches!(err, ProbeError::Io { .. }), "missing file → Io; got {err:?}");
}

// run_dag6_probe ──────────────────────────────────────────────────────────────

/// A product crate with `#![no_std]` and no std/alloc usage → clean.
#[test]
fn dag6_clean_no_std_crate_produces_no_violations() {
    let td = TempDir::new();
    let root = td.path();
    std::fs::create_dir_all(root.join("lib/no-std-clean/src")).unwrap();
    std::fs::write(
        root.join("lib/no-std-clean/Cargo.toml"),
        "[package]\nname = \"reovim-no-std-clean\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    std::fs::write(root.join("lib/no-std-clean/src/lib.rs"), "#![no_std]\n\npub fn hello() {}\n")
        .unwrap();
    let v = run_dag6_probe(root).expect("probe runs");
    assert!(v.is_empty(), "clean no_std crate → no violations; got {v:?}");
}

/// A product crate missing `#![no_std]` → `MissingNoStd`.
#[test]
fn dag6_missing_no_std_produces_violation() {
    let td = TempDir::new();
    let root = td.path();
    std::fs::create_dir_all(root.join("lib/std-crate/src")).unwrap();
    std::fs::write(
        root.join("lib/std-crate/Cargo.toml"),
        "[package]\nname = \"reovim-std-crate\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    std::fs::write(root.join("lib/std-crate/src/lib.rs"), "pub fn hello() {}\n").unwrap();
    let v = run_dag6_probe(root).expect("probe runs");
    let has = v.iter().any(|v| {
        matches!(v, Violation::MissingNoStd { crate_name, .. } if crate_name == "reovim-std-crate")
    });
    assert!(has, "missing #![no_std] → MissingNoStd; got {v:?}");
}

/// `use std::` in product source → `StdUsage`.
#[test]
fn dag6_use_std_produces_violation() {
    let td = TempDir::new();
    let root = td.path();
    std::fs::create_dir_all(root.join("lib/use-std/src")).unwrap();
    std::fs::write(
        root.join("lib/use-std/Cargo.toml"),
        "[package]\nname = \"reovim-use-std\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    std::fs::write(root.join("lib/use-std/src/lib.rs"), "#![no_std]\nuse std::fmt::Write;\n")
        .unwrap();
    let v = run_dag6_probe(root).expect("probe runs");
    let has = v.iter().any(|v| matches!(v, Violation::StdUsage { .. }));
    assert!(has, "use std:: → StdUsage; got {v:?}");
}

/// `use alloc::` in product source → `AllocUsage`.
#[test]
fn dag6_use_alloc_produces_violation() {
    let td = TempDir::new();
    let root = td.path();
    std::fs::create_dir_all(root.join("lib/use-alloc/src")).unwrap();
    std::fs::write(
        root.join("lib/use-alloc/Cargo.toml"),
        "[package]\nname = \"reovim-use-alloc\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    std::fs::write(root.join("lib/use-alloc/src/lib.rs"), "#![no_std]\nuse alloc::vec::Vec;\n")
        .unwrap();
    let v = run_dag6_probe(root).expect("probe runs");
    let has = v.iter().any(|v| matches!(v, Violation::AllocUsage { .. }));
    assert!(has, "use alloc:: → AllocUsage; got {v:?}");
}

/// `use std::` inside `#[cfg(test)] mod tests { ... }` → NOT a violation.
#[test]
fn dag6_std_inside_cfg_test_mod_is_exempt() {
    let td = TempDir::new();
    let root = td.path();
    std::fs::create_dir_all(root.join("lib/cfg-test-exempt/src")).unwrap();
    std::fs::write(
        root.join("lib/cfg-test-exempt/Cargo.toml"),
        "[package]\nname = \"reovim-cfg-test-exempt\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    std::fs::write(
        root.join("lib/cfg-test-exempt/src/lib.rs"),
        concat!(
            "#![no_std]\n",
            "\n",
            "pub fn hello() {}\n",
            "\n",
            "#[cfg(test)]\n",
            "mod tests {\n",
            "    use std::fmt;\n",
            "    fn it_works() { let _ = fmt::format(format_args!(\"\"));\n",
            "    }\n",
            "}\n",
        ),
    )
    .unwrap();
    let v = run_dag6_probe(root).expect("probe runs");
    let has_std = v.iter().any(|v| matches!(v, Violation::StdUsage { .. }));
    assert!(!has_std, "std inside #[cfg(test)] mod → no StdUsage; got {v:?}");
}

/// `lib/depgraph` is excluded from the DAG6 walk (bootstrap-state-2).
#[test]
fn dag6_bootstrap_exclusion_skips_depgraph() {
    let td = TempDir::new();
    let root = td.path();
    // Place a crate at lib/depgraph with no #![no_std].
    std::fs::create_dir_all(root.join("lib/depgraph/src")).unwrap();
    std::fs::write(
        root.join("lib/depgraph/Cargo.toml"),
        "[package]\nname = \"reovim-depgraph\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    std::fs::write(root.join("lib/depgraph/src/lib.rs"), "// no #![no_std] here\n").unwrap();
    let v = run_dag6_probe(root).expect("probe runs");
    assert!(v.is_empty(), "lib/depgraph excluded → no violations; got {v:?}");
}

/// `use std::` in a `tests/` subdirectory → NOT a violation (test target).
#[test]
fn dag6_std_in_tests_subdir_is_exempt() {
    let td = TempDir::new();
    let root = td.path();
    std::fs::create_dir_all(root.join("lib/has-tests-dir/src/tests")).unwrap();
    std::fs::write(
        root.join("lib/has-tests-dir/Cargo.toml"),
        "[package]\nname = \"reovim-has-tests-dir\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    std::fs::write(root.join("lib/has-tests-dir/src/lib.rs"), "#![no_std]\npub fn ok() {}\n")
        .unwrap();
    std::fs::write(
        root.join("lib/has-tests-dir/src/tests/mod.rs"),
        "use std::collections::HashMap;\n",
    )
    .unwrap();
    let v = run_dag6_probe(root).expect("probe runs");
    let has_std = v.iter().any(|v| matches!(v, Violation::StdUsage { .. }));
    assert!(!has_std, "std in tests/ subdir → no StdUsage; got {v:?}");
}

/// Violation Display impls cover DAG6 variants.
#[test]
fn dag6_violation_display_contains_dag6() {
    use std::path::PathBuf;

    let v1 = Violation::MissingNoStd {
        crate_name: "reovim-foo".to_owned(),
        crate_path: "lib/foo".to_owned(),
    };
    let s = v1.to_string();
    assert!(s.contains("DAG6"), "MissingNoStd display: {s}");
    assert!(s.contains("reovim-foo"), "MissingNoStd crate_name: {s}");
    assert!(s.contains("lib/foo"), "MissingNoStd crate_path: {s}");

    let v2 = Violation::StdUsage {
        file: PathBuf::from("lib/foo/src/lib.rs"),
    };
    let s = v2.to_string();
    assert!(s.contains("DAG6"), "StdUsage display: {s}");
    assert!(s.contains("lib/foo/src/lib.rs"), "StdUsage file: {s}");

    let v3 = Violation::AllocUsage {
        file: PathBuf::from("lib/foo/src/bar.rs"),
    };
    let s = v3.to_string();
    assert!(s.contains("DAG6"), "AllocUsage display: {s}");
    assert!(s.contains("lib/foo/src/bar.rs"), "AllocUsage file: {s}");

    let v4 = Violation::PanicProfileNotAbort {
        profile: "dev".to_owned(),
    };
    let s = v4.to_string();
    assert!(s.contains("DAG6"), "PanicProfileNotAbort display: {s}");
    assert!(s.contains("dev"), "PanicProfileNotAbort profile: {s}");
    assert!(s.contains("abort"), "PanicProfileNotAbort abort mention: {s}");
}

/// `allowlist_match` returns `None` for all new DAG6 variants.
#[test]
fn dag6_violations_are_never_allowlisted() {
    use std::path::PathBuf;
    let allowlist = Allowlist {
        entry: vec![AllowlistEntry {
            from: "a".to_owned(),
            to: "b".to_owned(),
            reason: "decoy".to_owned(),
            issue: "#0".to_owned(),
            expires: "never".to_owned(),
        }],
    };
    for v in [
        Violation::MissingNoStd {
            crate_name: "x".to_owned(),
            crate_path: "lib/x".to_owned(),
        },
        Violation::StdUsage {
            file: PathBuf::from("lib/x/src/lib.rs"),
        },
        Violation::AllocUsage {
            file: PathBuf::from("lib/x/src/lib.rs"),
        },
        Violation::PanicProfileNotAbort {
            profile: "dev".to_owned(),
        },
    ] {
        assert!(
            allowlist_match(&v, &allowlist).is_none(),
            "DAG6 violations must never be allowlisted: {v:?}"
        );
    }
}

/// Crate with no `src/` directory → `MissingNoStd`.
#[test]
fn dag6_crate_without_src_dir_produces_missing_no_std() {
    let td = TempDir::new();
    let root = td.path();
    std::fs::create_dir_all(root.join("lib/nosrc")).unwrap();
    std::fs::write(
        root.join("lib/nosrc/Cargo.toml"),
        "[package]\nname = \"reovim-nosrc\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    let v = run_dag6_probe(root).expect("probe runs");
    let has = v.iter().any(
        |v| matches!(v, Violation::MissingNoStd { crate_name, .. } if crate_name == "reovim-nosrc"),
    );
    assert!(has, "crate with no src/ → MissingNoStd; got {v:?}");
}

/// `#![no_std]` commented out → treated as missing.
#[test]
fn dag6_commented_no_std_is_treated_as_missing() {
    let td = TempDir::new();
    let root = td.path();
    std::fs::create_dir_all(root.join("lib/commented-no-std/src")).unwrap();
    std::fs::write(
        root.join("lib/commented-no-std/Cargo.toml"),
        "[package]\nname = \"reovim-commented-no-std\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    std::fs::write(
        root.join("lib/commented-no-std/src/lib.rs"),
        "// #![no_std]\npub fn hello() {}\n",
    )
    .unwrap();
    let v = run_dag6_probe(root).expect("probe runs");
    let has = v.iter().any(|v| {
        matches!(v, Violation::MissingNoStd { crate_name, .. } if crate_name == "reovim-commented-no-std")
    });
    assert!(has, "commented-out #![no_std] → MissingNoStd; got {v:?}");
}

/// `use std::` in a line comment → NOT a violation.
#[test]
fn dag6_std_in_line_comment_is_not_a_violation() {
    let td = TempDir::new();
    let root = td.path();
    std::fs::create_dir_all(root.join("lib/comment-std/src")).unwrap();
    std::fs::write(
        root.join("lib/comment-std/Cargo.toml"),
        "[package]\nname = \"reovim-comment-std\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    std::fs::write(
        root.join("lib/comment-std/src/lib.rs"),
        "#![no_std]\n// use std::fmt; this is a comment\npub fn ok() {}\n",
    )
    .unwrap();
    let v = run_dag6_probe(root).expect("probe runs");
    let has_std = v.iter().any(|v| matches!(v, Violation::StdUsage { .. }));
    assert!(!has_std, "std in line comment → no StdUsage; got {v:?}");
}

/// `src/main.rs` is accepted as the crate root for binary crates.
#[test]
fn dag6_main_rs_accepted_as_crate_root() {
    let td = TempDir::new();
    let root = td.path();
    std::fs::create_dir_all(root.join("apps/my-bin/src")).unwrap();
    std::fs::write(
        root.join("apps/my-bin/Cargo.toml"),
        "[package]\nname = \"reovim-my-bin\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    std::fs::write(
        root.join("apps/my-bin/src/main.rs"),
        "#![no_std]\n#![no_main]\nfn my_main() -> ! { loop {} }\n",
    )
    .unwrap();
    let v = run_dag6_probe(root).expect("probe runs");
    let has_missing = v
        .iter()
        .any(|v| matches!(v, Violation::MissingNoStd { .. }));
    assert!(!has_missing, "main.rs with #![no_std] → no MissingNoStd; got {v:?}");
}

// ── DAG6 cfg(test)-skip and brace-match edge cases ───────────────────────────

/// A `#[cfg(test)] mod t { … }` with the opening brace on the attribute
/// line itself is skipped to its matching close.
#[test]
fn cfg_test_skip_same_line_brace() {
    let lines = [
        "#[cfg(test)] mod t {",
        "    use std::fmt;",
        "}",
        "pub fn after() {}",
    ];
    assert_eq!(super::cfg_test_skip(&lines, 0), 3);
}

/// Blank lines between the attribute and its item are absorbed by the
/// lookahead before the item is classified.
#[test]
fn cfg_test_skip_blank_lines_before_item() {
    let lines = ["#[cfg(test)]", "", "", "mod t {", "    use std::fmt;", "}"];
    assert_eq!(super::cfg_test_skip(&lines, 0), 6);
}

/// A single non-block item after the attribute skips exactly through that
/// item line.
#[test]
fn cfg_test_skip_single_item() {
    let lines = ["#[cfg(test)]", "use std::fmt;", "pub fn after() {}"];
    assert_eq!(super::cfg_test_skip(&lines, 0), 2);
}

/// An attribute at end-of-file (nothing to decorate) advances one line.
#[test]
fn cfg_test_skip_attribute_at_eof() {
    let lines = ["#[cfg(test)]"];
    assert_eq!(super::cfg_test_skip(&lines, 0), 1);
}

/// An unclosed brace run falls back to the last line (fail-safe guard).
#[test]
fn brace_match_unclosed_returns_last_line() {
    let lines = ["mod t {", "    use std::fmt;", "    // never closed"];
    assert_eq!(super::brace_match_from_line(&lines, 0), 2);
}

/// An unreadable crate root (a directory where `lib.rs` should be) fails
/// closed as `MissingNoStd`.
#[test]
fn dag6_unreadable_crate_root_fails_closed() {
    let td = TempDir::new();
    let root = td.path();
    std::fs::create_dir_all(root.join("lib/dirroot/src/lib.rs")).unwrap();
    std::fs::write(
        root.join("lib/dirroot/Cargo.toml"),
        "[package]\nname = \"reovim-dirroot\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    let v = run_dag6_probe(root).expect("probe runs");
    let has_missing = v
        .iter()
        .any(|v| matches!(v, Violation::MissingNoStd { .. }));
    assert!(has_missing, "unreadable root must fail closed as MissingNoStd; got {v:?}");
}

/// A synthetic workspace where a `ServerContracts` crate depends on the kernel
/// must surface a DAG2 `ForbiddenEdge` through the full probe — the
/// contract tier is downstream of nothing but Foundation (1.2 §2). This is
/// the probe-level negative for the tier the walking skeleton added; the
/// matrix-cell unit negatives above cover the remaining new-tier cells.
#[test]
fn synthetic_contracts_to_kernel_dep_is_forbidden_edge() {
    let td = TempDir::new();
    let root = td.path();
    let kernel_dir = root.join("editor/lib/kernel");
    std::fs::create_dir_all(&kernel_dir).unwrap();
    std::fs::write(
        kernel_dir.join("Cargo.toml"),
        "[package]\nname = \"reovim-kernel\"\nversion = \"0.1.0\"\nedition = \"2024\"\n",
    )
    .unwrap();
    let contracts_dir = root.join("editor/lib/subsys/bad");
    std::fs::create_dir_all(&contracts_dir).unwrap();
    std::fs::write(
        contracts_dir.join("Cargo.toml"),
        "[package]\nname = \"reovim-subsys-bad\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\n[dependencies]\nreovim-kernel = { path = \"../../kernel\" }\n",
    )
    .unwrap();
    let probes_dir = root.join("tools/depgraph-probes");
    std::fs::create_dir_all(&probes_dir).unwrap();
    std::fs::write(probes_dir.join("composition-edges.toml"), "").unwrap();
    std::fs::write(probes_dir.join("transitional-allowlist.toml"), "").unwrap();

    let config = ProbeConfig::default_for(root).expect("config loads");
    let report = run_probe(root, &config).expect("probe runs");
    assert!(
        report
            .violations
            .iter()
            .any(|v| matches!(v, Violation::ForbiddenEdge { from, to, .. }
                if from == "reovim-subsys-bad" && to == "reovim-kernel")),
        "contracts → kernel must be a ForbiddenEdge; got: {}",
        report.summary()
    );
}
