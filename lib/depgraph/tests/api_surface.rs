//! API surface exercise from the rlib (integration) build context.
//!
//! # Why this file exists
//!
//! The 100% line + MC/DC coverage policy for `lib/depgraph` measures BOTH the
//! unit-test build (compiled with `#[cfg(test)]`) AND the rlib linked by each
//! integration-test binary.  Profiles merge per symbol, and a symbol compiled
//! into two builds must be exercised in EACH build where it is instantiated.
//!
//! Several internal functions and closures are exercised by unit tests but not
//! by any integration-test binary (their rlib instances are zero-hit), and two
//! closures are exercised only by integration tests (their unit instances are
//! zero-hit).  This file closes both gaps by exercising the complete public API
//! surface from the integration (rlib) build, acting simultaneously as an API
//! contract test and as a coverage complement to the unit suite.

mod common;

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use reovim_depgraph::{
    Allowlist, AllowlistEntry, Catalog, CatalogEdge, Category, DepTable, ProbeConfig, ProbeError,
    Report, Violation, classify, default_category_table, enumerate_crates, run_probe,
    toml::{TomlValue, parse_file, parse_text},
};

// ── workspace-root locator (mirrors workspace_probe.rs) ──────────────────────

fn workspace_root() -> PathBuf {
    common::workspace_root()
}

// ── 1. Report::summary over the live workspace ────────────────────────────────

#[test]
fn report_summary_contains_crate_count() {
    let root = workspace_root();
    let config = ProbeConfig::default_for(&root).expect("config loads");
    let report = run_probe(&root, &config).expect("probe runs");

    let summary = report.summary();
    // Foundation always has at least one crate (reovim-depgraph itself).
    assert!(
        summary.contains("foundation"),
        "summary must list foundation category; got:\n{summary}"
    );
    // The classified count across all categories matches the enumerated count.
    let classified: usize = report.category_counts.values().sum();
    let enumerated = enumerate_crates(&root).expect("enumeration runs").len();
    assert_eq!(
        classified, enumerated,
        "summary classified count must equal enumerated count; summary:\n{summary}"
    );
}

// ── 2. Display impls ──────────────────────────────────────────────────────────

#[test]
fn category_display_all_variants() {
    let pairs: &[(Category, &str)] = &[
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
    for (cat, expected) in pairs {
        assert_eq!(cat.to_string(), *expected, "Category::{cat:?} Display must be {expected}");
    }
}

#[test]
fn dep_table_display_all_variants() {
    assert_eq!(DepTable::Dependencies.to_string(), "dependencies");
    assert_eq!(DepTable::DevDependencies.to_string(), "dev-dependencies");
    assert_eq!(DepTable::BuildDependencies.to_string(), "build-dependencies");
}

#[test]
fn violation_display_all_variants() {
    let td = common::TempDir::new();
    let root = td.path();
    // Build a fixture workspace that generates all interesting Violation variants.

    // UnknownPath — crate at an unclassified path.
    let unknown = Violation::UnknownPath {
        path: "mystery/x".to_owned(),
    };
    let s = unknown.to_string();
    assert!(s.contains("DAG1"), "UnknownPath: {s}");
    assert!(s.contains("mystery/x"), "UnknownPath path: {s}");

    // AmbiguousPath.
    let ambig = Violation::AmbiguousPath {
        path: "lib/foo".to_owned(),
        candidates: vec![
            "`lib/*` (foundation)".to_owned(),
            "`lib/foo` (foundation)".to_owned(),
        ],
    };
    let s = ambig.to_string();
    assert!(s.contains("DAG1"), "AmbiguousPath: {s}");
    assert!(s.contains("lib/foo"), "AmbiguousPath path: {s}");
    assert!(s.contains("more than one"), "AmbiguousPath message: {s}");

    // ForbiddenEdge.
    let forbidden = Violation::ForbiddenEdge {
        from: "alpha".to_owned(),
        from_category: Category::Foundation,
        to: "beta".to_owned(),
        to_category: Category::ServerKernel,
    };
    let s = forbidden.to_string();
    assert!(s.contains("DAG2"), "ForbiddenEdge: {s}");
    assert!(s.contains("alpha"), "ForbiddenEdge from: {s}");
    assert!(s.contains("beta"), "ForbiddenEdge to: {s}");

    // UngrantedFoundationEdge.
    let ungranted = Violation::UngrantedFoundationEdge {
        from: "lib-a".to_owned(),
        to: "lib-b".to_owned(),
    };
    let s = ungranted.to_string();
    assert!(s.contains("DAG2"), "UngrantedFoundationEdge: {s}");
    assert!(s.contains("lib-a"), "UngrantedFoundationEdge from: {s}");

    // UncatalogedCompositionEdge.
    let uncataloged = Violation::UncatalogedCompositionEdge {
        from: "apps-bin".to_owned(),
        to: "ext-crate".to_owned(),
    };
    let s = uncataloged.to_string();
    assert!(s.contains("DAG3"), "UncatalogedCompositionEdge: {s}");
    assert!(s.contains("apps-bin"), "UncatalogedCompositionEdge from: {s}");

    // NonSovereignDep.
    let nonsov = Violation::NonSovereignDep {
        crate_path: "lib/probe".to_owned(),
        table: DepTable::Dependencies,
        dep: "serde".to_owned(),
    };
    let s = nonsov.to_string();
    assert!(s.contains("DAG5"), "NonSovereignDep: {s}");
    assert!(s.contains("serde"), "NonSovereignDep dep: {s}");
    assert!(s.contains("dependencies"), "NonSovereignDep table: {s}");

    // ProbeError::Parse.
    let parse_err = ProbeError::Parse {
        path: root.join("x.toml"),
        message: "bad field".to_owned(),
    };
    let s = parse_err.to_string();
    assert!(s.contains("parse error"), "ProbeError::Parse: {s}");
    assert!(s.contains("bad field"), "ProbeError::Parse message: {s}");

    // ProbeError::Io — trigger a real I/O error by opening a missing file.
    let missing = root.join("no-such-file.toml");
    let io_err = parse_file(&missing).unwrap_err();
    let s = io_err.to_string();
    assert!(s.contains("io error"), "ProbeError::Io: {s}");
}

// ── 3. classify AmbiguousPath via custom two-pattern table ───────────────────

#[test]
fn classify_ambiguous_path_via_custom_table() {
    // Two overlapping patterns both match "lib/foo": the candidates-mapping
    // closure inside `classify` collects both into the `AmbiguousPath`.
    let table = vec![
        ("lib/*".to_owned(), Category::Foundation),
        ("lib/foo".to_owned(), Category::Foundation),
    ];
    let result = classify("lib/foo", &table);
    match result {
        Err(Violation::AmbiguousPath { path, candidates }) => {
            assert_eq!(path, "lib/foo");
            assert_eq!(
                candidates.len(),
                2,
                "must report both matching patterns; got {candidates:?}"
            );
        }
        other => panic!("expected AmbiguousPath, got {other:?}"),
    }
}

// ── 4. check_edge foundation-grants closure (both polarities) ────────────────

fn pkg_toml(name: &str) -> String {
    format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n")
}

fn root_workspace_toml(members: &str) -> String {
    format!("[workspace]\nresolver = \"2\"\nmembers = [{members}]\nexclude = [\"archive\"]\n")
}

/// Foundation→Foundation with a §6 grant: no `UngrantedFoundationEdge`.
#[test]
fn foundation_edge_with_grant_produces_no_violation() {
    let td = common::TempDir::new();
    let root = td.path();
    common::write_file(root, "Cargo.toml", &root_workspace_toml(r#""lib/a", "lib/b""#));
    common::write_file(
        root,
        "lib/a/Cargo.toml",
        &format!("{}\n[dependencies]\ncrate-b = {{ path = \"../b\" }}\n", pkg_toml("crate-a")),
    );
    common::write_file(root, "lib/b/Cargo.toml", &pkg_toml("crate-b"));

    let mut grants = BTreeMap::new();
    grants.insert("crate-a".to_owned(), vec!["crate-b".to_owned()]);
    let config = ProbeConfig {
        category_table: default_category_table(),
        foundation_grants: grants,
        catalog: Catalog::default(),
        allowlist: Allowlist::default(),
    };
    let report = run_probe(root, &config).expect("probe runs");
    let has_ungranted = report
        .violations
        .iter()
        .any(|v| matches!(v, Violation::UngrantedFoundationEdge { .. }));
    assert!(
        !has_ungranted,
        "granted foundation edge must not produce UngrantedFoundationEdge;\nviolations: {:?}",
        report.violations
    );
}

/// Foundation→Foundation without a §6 grant: produces `UngrantedFoundationEdge`.
#[test]
fn foundation_edge_without_grant_produces_ungranted_violation() {
    let td = common::TempDir::new();
    let root = td.path();
    common::write_file(root, "Cargo.toml", &root_workspace_toml(r#""lib/a", "lib/b""#));
    common::write_file(
        root,
        "lib/a/Cargo.toml",
        &format!("{}\n[dependencies]\ncrate-b = {{ path = \"../b\" }}\n", pkg_toml("crate-a")),
    );
    common::write_file(root, "lib/b/Cargo.toml", &pkg_toml("crate-b"));

    // Empty foundation_grants: no grant for crate-a → crate-b.
    let config = ProbeConfig {
        category_table: default_category_table(),
        foundation_grants: BTreeMap::new(),
        catalog: Catalog::default(),
        allowlist: Allowlist::default(),
    };
    let report = run_probe(root, &config).expect("probe runs");
    let has_ungranted = report.violations.iter().any(|v| {
        matches!(v, Violation::UngrantedFoundationEdge { from, to } if from == "crate-a" && to == "crate-b")
    });
    assert!(
        has_ungranted,
        "missing grant must produce UngrantedFoundationEdge for crate-a→crate-b;\nviolations: {:?}",
        report.violations
    );
}

// ── 5. allowlist_match closure via run_probe ──────────────────────────────────

#[test]
fn allowlisted_violation_appears_in_report_allowlisted_list() {
    let td = common::TempDir::new();
    let root = td.path();
    // apps/cli (Apps) → ext/client/driver/foo (ClientExt): uncataloged composition edge.
    common::write_file(
        root,
        "Cargo.toml",
        &root_workspace_toml(r#""apps/cli", "ext/client/driver/foo""#),
    );
    common::write_file(
        root,
        "apps/cli/Cargo.toml",
        &format!(
            "{}\n[dependencies]\nreovim-driver-foo = {{ path = \"../../ext/client/driver/foo\" }}\n",
            pkg_toml("reovim-cli")
        ),
    );
    common::write_file(root, "ext/client/driver/foo/Cargo.toml", &pkg_toml("reovim-driver-foo"));

    // Allowlist entry covering the uncataloged edge.
    let allowlist = Allowlist {
        entry: vec![AllowlistEntry {
            from: "reovim-cli".to_owned(),
            to: "reovim-driver-foo".to_owned(),
            reason: "transitional composition".to_owned(),
            issue: "#999".to_owned(),
            expires: "v0.17".to_owned(),
        }],
    };
    let config = ProbeConfig {
        category_table: default_category_table(),
        foundation_grants: BTreeMap::new(),
        catalog: Catalog::default(),
        allowlist,
    };
    let report = run_probe(root, &config).expect("probe runs");

    let covered = report.allowlisted.iter().any(|a| {
        matches!(&a.violation, Violation::UncatalogedCompositionEdge { from, to }
            if from == "reovim-cli" && to == "reovim-driver-foo")
            && a.issue == "#999"
    });
    assert!(
        covered,
        "allowlisted entry must suppress the violation; allowlisted: {:?}",
        report.allowlisted
    );
    let still_live = report.violations.iter().any(|v| {
        matches!(v, Violation::UncatalogedCompositionEdge { from, to }
            if from == "reovim-cli" && to == "reovim-driver-foo")
    });
    assert!(!still_live, "allowlisted violation must not appear in live violations");
}

// ── 6. Catalog::validate gate closure ────────────────────────────────────────

#[test]
fn catalog_validate_accepts_edge_with_non_blank_gate() {
    let catalog = Catalog {
        edge: vec![CatalogEdge {
            from: "a".to_owned(),
            to: "b".to_owned(),
            gate: Some("my-feature".to_owned()),
            reason: "needed".to_owned(),
        }],
    };
    catalog
        .validate(Path::new("composition-edges.toml"))
        .expect("non-blank gate must pass");
}

#[test]
fn catalog_validate_rejects_blank_gate() {
    let catalog = Catalog {
        edge: vec![CatalogEdge {
            from: "a".to_owned(),
            to: "b".to_owned(),
            gate: Some("  ".to_owned()),
            reason: "needed".to_owned(),
        }],
    };
    let err = catalog
        .validate(Path::new("composition-edges.toml"))
        .unwrap_err();
    let s = err.to_string();
    assert!(s.contains("empty gate"), "blank gate must be rejected; got: {s}");
}

// ── 7. ProbeConfig::default_for + get_required_str + schema_error ─────────────

/// Catalog with a [[edge]] missing `reason` returns a Parse error naming the field.
#[test]
fn default_for_catalog_missing_reason_returns_parse_error() {
    let td = common::TempDir::new();
    let root = td.path();
    // Write the catalog file with a missing `reason` field.
    common::write_file(
        root,
        "tools/depgraph-probes/composition-edges.toml",
        "[[edge]]\nfrom = \"apps/x\"\nto = \"lib/y\"\n",
    );
    let err = ProbeConfig::default_for(root).unwrap_err();
    let s = err.to_string();
    assert!(s.contains("reason"), "error must mention missing 'reason' field; got: {s}");
}

/// Well-formed catalog + no allowlist file → `default_for` succeeds.
#[test]
fn default_for_with_valid_catalog_succeeds() {
    let td = common::TempDir::new();
    let root = td.path();
    common::write_file(
        root,
        "tools/depgraph-probes/composition-edges.toml",
        "[[edge]]\nfrom = \"reovim\"\nto = \"reovim-tui\"\nreason = \"embedded default\"\n",
    );
    let config = ProbeConfig::default_for(root).expect("well-formed catalog must load");
    assert_eq!(config.catalog.edge.len(), 1);
    assert_eq!(config.catalog.edge[0].from, "reovim");
}

// ── 8. io_error closure via enumerate_crates on a missing root ────────────────

#[test]
fn enumerate_crates_returns_io_error_for_missing_root() {
    // A nonexistent root makes the directory walk's read_dir fail, which
    // exercises the Io-error conversion closure.
    let td = common::TempDir::new();
    let missing = td.path().join("does-not-exist");

    let result = enumerate_crates(&missing);
    assert!(
        matches!(result, Err(ProbeError::Io { .. })),
        "enumerate_crates on a missing root must return ProbeError::Io"
    );
}

// ── 9. parse_manifest missing-name closure ────────────────────────────────────

#[test]
fn enumerate_crates_returns_parse_error_for_manifest_without_package_name() {
    let td = common::TempDir::new();
    let root = td.path();
    // A manifest with [package] but no `name` key triggers the missing-name error.
    common::write_file(root, "lib/nameless/Cargo.toml", "[package]\nversion = \"0.1.0\"\n");
    let result = enumerate_crates(root);
    assert!(result.is_err(), "missing [package].name must produce an error");
    let msg = result.unwrap_err().to_string();
    assert!(msg.contains("missing [package].name"), "error message: {msg}");
}

// ── 10. toml public API from integration ─────────────────────────────────────

// 10a. Single-line string array: multi-element and trailing comma.
#[test]
fn toml_parse_text_multi_element_array() {
    let doc = parse_text(
        "[workspace]\nmembers = [\"lib/a\", \"lib/b\", \"lib/c\"]\n",
        Path::new("Cargo.toml"),
    )
    .unwrap();
    let v = doc
        .sections
        .get("workspace")
        .and_then(|s| s.get("members"))
        .unwrap();
    assert_eq!(
        *v,
        TomlValue::Array(vec!["lib/a".to_owned(), "lib/b".to_owned(), "lib/c".to_owned()])
    );
}

#[test]
fn toml_parse_text_array_trailing_comma() {
    let doc = parse_text("[w]\nm = [\"x\", \"y\",]\n", Path::new("t.toml")).unwrap();
    let v = doc.sections.get("w").and_then(|s| s.get("m")).unwrap();
    assert_eq!(*v, TomlValue::Array(vec!["x".to_owned(), "y".to_owned()]));
}

// 10b. Array error: unclosed [, empty interior element, non-string element.
#[test]
fn toml_array_unclosed_bracket_is_error() {
    let err = parse_text("[w]\nm = [\"a\"\n", Path::new("t.toml")).unwrap_err();
    assert!(err.to_string().contains("unclosed"), "unclosed array: {err}");
}

#[test]
fn toml_array_empty_interior_element_is_error() {
    let err = parse_text("[w]\nm = [\"a\",, \"b\"]\n", Path::new("t.toml")).unwrap_err();
    assert!(err.to_string().contains("empty array element"), "empty elem: {err}");
}

#[test]
fn toml_array_non_string_element_is_error() {
    let err = parse_text("[w]\nm = [42]\n", Path::new("t.toml")).unwrap_err();
    // 42 doesn't start with `"` so parse_string errors with "unclosed `"`
    assert!(err.to_string().contains("unclosed"), "non-string elem: {err}");
}

// 10c. [[entry]] array-of-tables: parse_array_header + malformed unclosed header.
#[test]
fn toml_array_of_tables_parses() {
    let doc =
        parse_text("[[entry]]\nfrom = \"a\"\nto = \"b\"\nreason = \"why\"\n", Path::new("t.toml"))
            .unwrap();
    let entries = doc.array("entry");
    assert_eq!(entries.len(), 1);
    assert_eq!(entries[0].get("from").map(String::as_str), Some("a"));
}

#[test]
fn toml_malformed_array_of_tables_header_is_error() {
    let err = parse_text("[[entry\nfrom = \"a\"\n", Path::new("t.toml")).unwrap_err();
    assert!(err.to_string().contains("line 1"), "malformed [[: {err}");
}

// 10d. Section-header error: 3-dotted depth.
#[test]
fn toml_section_header_three_dots_is_error() {
    let err = parse_text("[a.b.c]\nk = \"v\"\n", Path::new("t.toml")).unwrap_err();
    assert!(err.to_string().contains("depth"), "3-dot header: {err}");
}

// 10e. parse_kv error: line with = but empty key.
#[test]
fn toml_empty_key_is_error() {
    let err = parse_text("[s]\n= \"v\"\n", Path::new("t.toml")).unwrap_err();
    assert!(err.to_string().contains("empty key"), "empty key: {err}");
}

// 10f. parse_string unclosed-quote error.
#[test]
fn toml_unclosed_quote_string_is_error() {
    let err = parse_text("[s]\nk = \"unclosed\n", Path::new("t.toml")).unwrap_err();
    assert!(err.to_string().contains("unclosed"), "unclosed quote: {err}");
}

// 10g. parse_inline_table errors: unclosed {, nested { depth, non-string-non-bool value.
#[test]
fn toml_inline_table_unclosed_brace_is_error() {
    let err = parse_text("[s]\nk = { a = \"x\"\n", Path::new("t.toml")).unwrap_err();
    assert!(err.to_string().contains("unclosed"), "unclosed inline table: {err}");
}

#[test]
fn toml_inline_table_nested_brace_is_error() {
    let err = parse_text("[s]\nk = { a = { b = \"c\" } }\n", Path::new("t.toml")).unwrap_err();
    assert!(err.to_string().contains("nested"), "nested inline table: {err}");
}

#[test]
fn toml_inline_table_non_string_non_bool_value_is_error() {
    // An integer inside an inline table is not string or bool.
    let err = parse_text("[s]\nk = { a = 42 }\n", Path::new("t.toml")).unwrap_err();
    assert!(err.to_string().contains("not supported"), "int in inline table: {err}");
}

// 10h. parse_inline_table missing `=` inside entry (exercises the
//      `expected = inside inline table` ok_or_else closure that has no unit test).
#[test]
fn toml_inline_table_entry_without_equals_is_error() {
    // `{ noequalssign }` triggers the `split_once('=')` ok_or_else path inside
    // parse_inline_table, which is not reachable from the unit-test build of
    // parse_text (the outer parse_kv guard `line.contains('=')` is true for the
    // whole line, but the individual entry inside the inline table has no `=`).
    let err = parse_text("[s]\nk = { noequalssign }\n", Path::new("t.toml")).unwrap_err();
    let s = err.to_string();
    assert!(
        s.contains("expected `=` inside inline table"),
        "missing = in inline table entry must error; got: {s}"
    );
}

// 10i. TomlDoc::get_str hit + miss.
#[test]
fn toml_doc_get_str_hit_and_miss() {
    let doc = parse_text("[package]\nname = \"reovim-x\"\n", Path::new("t.toml")).unwrap();
    assert_eq!(doc.get_str("package", "name"), Some("reovim-x"));
    assert_eq!(doc.get_str("package", "missing"), None);
    assert_eq!(doc.get_str("nosection", "name"), None);
}

// 10j. TomlValue::as_table Some + None.
#[test]
fn toml_value_as_table_some_and_none() {
    let tbl: TomlValue = TomlValue::InlineTable(BTreeMap::new());
    assert!(tbl.as_table().is_some(), "InlineTable must return Some from as_table");
    assert!(
        TomlValue::Bool(true).as_table().is_none(),
        "Bool must return None from as_table"
    );
    assert!(
        TomlValue::String("x".to_owned()).as_table().is_none(),
        "String must return None"
    );
}

// 10k. parse_file on a real temp file (success) and missing path (Io error).
#[test]
fn toml_parse_file_success_and_io_error() {
    let td = common::TempDir::new();
    let root = td.path();
    let path = root.join("good.toml");
    std::fs::write(&path, "[package]\nname = \"x\"\n").expect("write good.toml");
    let doc = parse_file(&path).expect("parse_file must succeed on a real file");
    assert_eq!(doc.get_str("package", "name"), Some("x"));

    let missing = root.join("no-such.toml");
    let err = parse_file(&missing).unwrap_err();
    assert!(
        matches!(err, ProbeError::Io { .. }),
        "missing file must produce ProbeError::Io; got {err:?}"
    );
}

// ── Report summary includes allowlisted entry ─────────────────────────────────

#[test]
fn report_summary_includes_allowlisted_entry() {
    // Build a minimal report with one allowlisted item and verify summary output.
    let mut report = Report::default();
    report.allowlisted.push(reovim_depgraph::Allowlisted {
        violation: Violation::UncatalogedCompositionEdge {
            from: "apps-x".to_owned(),
            to: "ext-y".to_owned(),
        },
        issue: "#42".to_owned(),
        expires: "v1.0".to_owned(),
    });
    let summary = report.summary();
    assert!(
        summary.contains("allowlisted"),
        "summary must mention allowlisted; got:\n{summary}"
    );
    assert!(summary.contains("#42"), "summary must include issue ref; got:\n{summary}");
    assert!(summary.contains("v1.0"), "summary must include expiry; got:\n{summary}");
}

// ── string-edge cases in the rlib build: comment stripping + entry splitting ──

/// A `#` inside a quoted string is not a comment, an escaped character
/// inside a string is skipped, and a trailing real comment is stripped.
#[test]
fn parse_text_handles_hash_and_escape_inside_strings() {
    let doc = parse_text(
        "[package]\nname = \"a\\\\b#c\" # trailing comment\n",
        std::path::Path::new("inline"),
    )
    .expect("string with escaped char and hash must parse");
    assert_eq!(doc.get_str("package", "name"), Some("a\\\\b#c"));
}

/// An inline table whose string values contain commas, escapes, and a
/// multi-element array splits into the right entries (quote-, escape-,
/// and bracket-aware splitting).
#[test]
fn parse_text_splits_inline_entries_with_quoted_commas_and_arrays() {
    let doc = parse_text(
        "[dependencies]\nfoo = { path = \"a,b\\\\c\", features = [\"x\", \"y\"] }\n",
        std::path::Path::new("inline"),
    )
    .expect("inline table with quoted comma and array must parse");
    let foo = doc
        .sections
        .get("dependencies")
        .and_then(|s| s.get("foo"))
        .and_then(TomlValue::as_table)
        .expect("foo must be an inline table");
    assert_eq!(foo.get("path").and_then(TomlValue::as_str), Some("a,b\\\\c"));
    assert_eq!(
        foo.get("features"),
        Some(&TomlValue::Array(vec!["x".to_owned(), "y".to_owned()]))
    );
}

/// Byte-class matrix for the comment stripper and entry splitter: a bare
/// backslash outside a string, and brackets/backslash/comma/hash inside a
/// quoted inline-table string, must all pass through untouched.
#[test]
fn parse_text_byte_class_matrix() {
    let doc = parse_text(
        "[s]\nk\\ = \"v\"\nt = { p = \"a[1]\\\\,b#c\", q\\w = \"z\" }\n",
        std::path::Path::new("inline"),
    )
    .expect("byte-class matrix must parse");
    let sec = doc.sections.get("s").expect("section s");
    assert!(sec.contains_key("k\\"), "bare backslash stays part of the key");
    let t = sec
        .get("t")
        .and_then(TomlValue::as_table)
        .expect("t is a table");
    assert_eq!(t.get("p").and_then(TomlValue::as_str), Some("a[1]\\\\,b#c"));
    assert_eq!(t.get("q\\w").and_then(TomlValue::as_str), Some("z"));
}

// ── final rlib-side branch parity: cataloged edge, allowed edge, allowlist ────

/// A cataloged composition edge passes; near-miss catalog entries (wrong
/// `to`, wrong `from`) do not satisfy the lookup.
#[test]
fn cataloged_composition_edge_passes() {
    let td = common::TempDir::new();
    let root = td.path();
    common::write_file(
        root,
        "apps/cli/Cargo.toml",
        &format!(
            "{}\n[dependencies]\nreovim-ext-thing = {{ path = \"../../ext/client/driver/thing\" }}\n",
            pkg_toml("reovim-cli")
        ),
    );
    common::write_file(root, "ext/client/driver/thing/Cargo.toml", &pkg_toml("reovim-ext-thing"));

    let mut config = ProbeConfig {
        category_table: default_category_table(),
        foundation_grants: BTreeMap::new(),
        catalog: Catalog {
            edge: vec![
                // Wrong `from`: never matches.
                CatalogEdge {
                    from: "someone-else".to_owned(),
                    to: "reovim-ext-thing".to_owned(),
                    gate: None,
                    reason: "decoy".to_owned(),
                },
                // Right `from`, wrong `to`.
                CatalogEdge {
                    from: "reovim-cli".to_owned(),
                    to: "another-dep".to_owned(),
                    gate: None,
                    reason: "decoy".to_owned(),
                },
                // The real grant.
                CatalogEdge {
                    from: "reovim-cli".to_owned(),
                    to: "reovim-ext-thing".to_owned(),
                    gate: None,
                    reason: "cli uses the driver".to_owned(),
                },
            ],
        },
        allowlist: Allowlist::default(),
    };
    let report = run_probe(root, &config).expect("probe runs");
    assert!(
        report.violations.is_empty(),
        "cataloged edge must pass; got {:?}",
        report.violations
    );

    // Same fixture without the real grant → violation.
    config.catalog.edge.pop();
    let report = run_probe(root, &config).expect("probe runs");
    assert!(
        report
            .violations
            .iter()
            .any(|v| matches!(v, Violation::UncatalogedCompositionEdge { .. })),
        "missing grant must produce UncatalogedCompositionEdge; got {:?}",
        report.violations
    );
}

/// An allowed non-foundation category edge (`ServerKernel` →
/// `ServerContracts` per 1.2 §2) produces no violation.
#[test]
fn allowed_category_edge_passes() {
    let td = common::TempDir::new();
    let root = td.path();
    common::write_file(
        root,
        "server/lib/kernel/core/Cargo.toml",
        &format!(
            "{}\n[dependencies]\nreovim-subsys-x = {{ path = \"../../subsys/x\" }}\n",
            pkg_toml("reovim-kernel-core")
        ),
    );
    common::write_file(root, "server/lib/subsys/x/Cargo.toml", &pkg_toml("reovim-subsys-x"));

    let config = ProbeConfig {
        category_table: default_category_table(),
        foundation_grants: BTreeMap::new(),
        catalog: Catalog::default(),
        allowlist: Allowlist::default(),
    };
    let report = run_probe(root, &config).expect("probe runs");
    assert!(
        report.violations.is_empty(),
        "kernel → contracts is an allowed §2 edge; got {:?}",
        report.violations
    );
}

/// `Allowlist::validate` from the rlib build: a well-formed entry passes,
/// a blank `expires` fails.
#[test]
fn allowlist_validate_from_rlib() {
    let p = Path::new("transitional-allowlist.toml");
    let good = Allowlist {
        entry: vec![AllowlistEntry {
            from: "a".to_owned(),
            to: "b".to_owned(),
            reason: "pending move".to_owned(),
            issue: "#775".to_owned(),
            expires: "v4.0-rc1".to_owned(),
        }],
    };
    good.validate(p).expect("well-formed entry must validate");

    let bad = Allowlist {
        entry: vec![AllowlistEntry {
            expires: String::new(),
            ..good.entry[0].clone()
        }],
    };
    let msg = bad.validate(p).unwrap_err().to_string();
    assert!(msg.contains("empty expires"), "{msg}");
}
