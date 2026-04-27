//! Guards the post-Plan-15 invariant that `reovim_protocol::v2` is gone.
//!
//! Two checks:
//!
//! 1. `uapi/protocol/src/lib.rs` has no `pub use v3 as v2` (and no
//!    `pub use v2`). Re-introducing the alias would silently re-enable
//!    every removed v2 consumer and defeat the migration.
//! 2. Workspace-wide source walk (`src/`, `tests/`, `benches/`,
//!    `examples/` under every crate root found via `cargo_metadata`) has
//!    no live references to `reovim_protocol::v2` or `protocol::v2` in
//!    non-comment code. Exclusions: `CHANGELOG.md`, `docs/`, any `*.md`,
//!    any `*.proto`, `archive/` (v0.8.x frozen reference). Single-line
//!    `//` and block `/* */` comments are stripped before scanning.
//!
//! No AST parsing — substring check with comment-strip, consistent with
//! Plan 14 Phase D guard style.

use {
    cargo_metadata::MetadataCommand,
    std::{
        fs,
        path::{Path, PathBuf},
    },
};

const ALIAS_LINE_A: &str = "pub use v3 as v2";
const ALIAS_LINE_B: &str = "pub use v2";
const PROTOCOL_LIB_REL: &str = "uapi/protocol/src/lib.rs";

const REF_PATTERNS: &[&str] = &["reovim_protocol::v2", "protocol::v2"];

const SCAN_SUBDIRS: &[&str] = &["src", "tests", "benches", "examples"];

fn workspace_root() -> PathBuf {
    MetadataCommand::new()
        .exec()
        .expect("cargo metadata")
        .workspace_root
        .into()
}

#[test]
fn uapi_protocol_has_no_v2_alias() {
    let root = workspace_root();
    let path = root.join(PROTOCOL_LIB_REL);
    let src = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));

    let stripped = strip_comments(&src);

    assert!(
        !stripped.contains(ALIAS_LINE_A),
        "uapi/protocol/src/lib.rs contains `{ALIAS_LINE_A}` — Plan 15 V.7 \
         deleted this alias; re-introducing it re-enables the v2 consumer \
         path and defeats the migration. Remove the alias and migrate the \
         caller to v3."
    );
    assert!(
        !stripped.contains(ALIAS_LINE_B),
        "uapi/protocol/src/lib.rs contains `{ALIAS_LINE_B}` — v2 is gone \
         post-Plan-15. If you need the v2 module for a new migration, \
         restore v2.rs + v2 proto definitions properly rather than \
         re-introducing the alias."
    );
}

#[test]
fn workspace_has_no_live_v2_references() {
    let root = workspace_root();

    let metadata = MetadataCommand::new().exec().expect("cargo metadata");
    let workspace_crates: Vec<PathBuf> = metadata
        .workspace_packages()
        .iter()
        .map(|p| {
            p.manifest_path
                .parent()
                .expect("manifest has parent")
                .to_path_buf()
                .into()
        })
        .collect();

    let mut offenders: Vec<String> = Vec::new();

    for crate_root in &workspace_crates {
        if is_archive_path(crate_root, &root) {
            continue;
        }
        for sub in SCAN_SUBDIRS {
            let dir = crate_root.join(sub);
            if dir.is_dir() {
                scan_dir(&dir, &root, &mut offenders);
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "workspace has live references to `reovim_protocol::v2` / \
         `protocol::v2` in non-comment code. Plan 15 V.7 deleted the \
         alias — every remaining reference must be migrated to v3.\n\n\
         Offending sites:\n  {}",
        offenders.join("\n  ")
    );
}

fn scan_dir(dir: &Path, workspace_root: &Path, offenders: &mut Vec<String>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            if path.file_name().and_then(|n| n.to_str()) == Some("target") {
                continue;
            }
            scan_dir(&path, workspace_root, offenders);
            continue;
        }
        if path.extension().is_none_or(|e| e != "rs") {
            continue;
        }
        let Ok(src) = fs::read_to_string(&path) else {
            continue;
        };
        let rel = path
            .strip_prefix(workspace_root)
            .unwrap_or(&path)
            .display()
            .to_string();
        // Self-exclude: this test file itself contains the patterns it scans for.
        if rel.ends_with("lib/depgraph/tests/proto_no_v2_alias.rs") {
            continue;
        }
        let stripped = strip_comments(&src);
        for pat in REF_PATTERNS {
            if stripped.contains(pat) {
                offenders.push(format!("{rel}: contains `{pat}`"));
            }
        }
    }
}

fn is_archive_path(crate_root: &Path, workspace_root: &Path) -> bool {
    crate_root
        .strip_prefix(workspace_root)
        .ok()
        .and_then(|rel| rel.components().next())
        .and_then(|c| c.as_os_str().to_str())
        .is_some_and(|first| first == "archive")
}

/// Strip `//` single-line comments and `/* */` block comments from `src`.
///
/// Replaces comment bytes with spaces so line/byte offsets survive for
/// diagnostic purposes. Does NOT track string literal state — if a
/// `reovim_protocol::v2` appears inside a raw string literal (`r"..."`)
/// it WILL trip the guard. This is an acceptable false positive per
/// the Plan 15 Phase D Telemetry review — fix by renaming the literal
/// or adding a path-based exclusion.
fn strip_comments(src: &str) -> String {
    let bytes = src.as_bytes();
    let mut out = String::with_capacity(src.len());
    let mut i = 0;
    while i < bytes.len() {
        if i + 1 < bytes.len() && &bytes[i..i + 2] == b"//" {
            while i < bytes.len() && bytes[i] != b'\n' {
                out.push(' ');
                i += 1;
            }
            continue;
        }
        if i + 1 < bytes.len() && &bytes[i..i + 2] == b"/*" {
            out.push_str("  ");
            i += 2;
            while i + 1 < bytes.len() && &bytes[i..i + 2] != b"*/" {
                out.push(if bytes[i] == b'\n' { '\n' } else { ' ' });
                i += 1;
            }
            if i + 1 < bytes.len() {
                out.push_str("  ");
                i += 2;
            }
            continue;
        }
        out.push(bytes[i] as char);
        i += 1;
    }
    out
}
