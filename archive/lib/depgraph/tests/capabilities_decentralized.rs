//! Guards the post-Plan-16 invariant that `reovim-capabilities` is gone.
//!
//! Three checks:
//!
//! 1. `no_reovim_capabilities_workspace_member`: `cargo_metadata` shows
//!    zero workspace packages named `reovim-capabilities` (the god
//!    crate was deleted in Plan 16 Phase E).
//! 2. `no_reovim_capabilities_dep_edge`: every workspace package's
//!    `Cargo.toml` dependency tables (normal + dev) have no edge
//!    to `reovim-capabilities`.
//! 3. `no_reovim_capabilities_rust_reference`: workspace source walk
//!    (`src/`, `tests/`, `benches/`, `examples/`) with comment-strip
//!    has no `reovim_capabilities::` or `use reovim_capabilities`
//!    references in non-comment code. Exclusions: `archive/`, any
//!    `*.md`, any `*.proto`, self-file.
//!
//! Pattern mirrors `proto_no_v2_alias.rs` (Plan 15 Phase D).

use {
    cargo_metadata::{DependencyKind, MetadataCommand},
    std::{
        fs,
        path::{Path, PathBuf},
    },
};

const TARGET_PACKAGE: &str = "reovim-capabilities";

const REF_PATTERNS: &[&str] = &["reovim_capabilities::", "use reovim_capabilities"];

const SCAN_SUBDIRS: &[&str] = &["src", "tests", "benches", "examples"];

fn workspace_root() -> PathBuf {
    MetadataCommand::new()
        .exec()
        .expect("cargo metadata")
        .workspace_root
        .into()
}

#[test]
fn no_reovim_capabilities_workspace_member() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");

    let offender = metadata
        .workspace_packages()
        .into_iter()
        .find(|p| p.name.as_str() == TARGET_PACKAGE);

    assert!(
        offender.is_none(),
        "workspace has a package named `{TARGET_PACKAGE}` — Plan 16 Phase E \
         deleted this crate. Remove the workspace member entry and the \
         `[workspace.dependencies]` line from the root Cargo.toml."
    );
}

#[test]
fn no_reovim_capabilities_dep_edge() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");

    let mut offenders: Vec<String> = Vec::new();

    for pkg in metadata.workspace_packages() {
        for dep in &pkg.dependencies {
            if dep.name == TARGET_PACKAGE
                && matches!(dep.kind, DependencyKind::Normal | DependencyKind::Development)
            {
                let kind = match dep.kind {
                    DependencyKind::Normal => "normal",
                    DependencyKind::Development => "dev",
                    _ => "other",
                };
                offenders.push(format!("{}: {} dependency", pkg.name, kind));
            }
        }
    }

    assert!(
        offenders.is_empty(),
        "workspace packages declare `{TARGET_PACKAGE}` as a Cargo \
         dependency — Plan 16 Phase E removed this crate. Migrate \
         to the owning subsys (e.g. `reovim-subsys-input`) or \
         `reovim-domain-text-capabilities`.\n\n\
         Offending packages:\n  {}",
        offenders.join("\n  ")
    );
}

#[test]
fn no_reovim_capabilities_rust_reference() {
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
        "workspace has live references to `reovim_capabilities::` / \
         `use reovim_capabilities` in non-comment code. Plan 16 Phase E \
         deleted the crate — every remaining reference must be migrated \
         to the owning subsys or domain-capabilities crate.\n\n\
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
        if rel.ends_with("lib/depgraph/tests/capabilities_decentralized.rs") {
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
