//! Guards the Plan 17-β.1 cutover invariant: no hardcoded surface
//! descriptor decode patterns remain in `clients/tui/src/`.
//!
//! Before 17-β.1, `clients/tui/src/notification_handler.rs` had an
//! inline `if desc.kind == 0x0001 && desc.body.len() >= 8 { … }`
//! block decoding a cell-grid surface. 17-β.1 replaced it with a
//! registry dispatch (`surface_decoding::global_registry()`), moving
//! kind-specific policy into `reovim-tui-mod-surface-descriptor-cell-grid`.
//!
//! This probe walks `clients/tui/src/` and fails if it re-discovers
//! the old pattern (`desc.kind ==` or `desc.kind == 0x`) in non-comment
//! code. The probe is policy, not perfect: it uses a comment-strip
//! line filter. Self-file (this test) and any test file that inlines
//! a commented example are excluded.

use {
    cargo_metadata::MetadataCommand,
    std::{
        fs,
        path::{Path, PathBuf},
    },
};

const FORBIDDEN_PATTERNS: &[&str] = &["desc.kind ==", "desc.kind== ", "desc.kind==0x"];

/// Scan this directory (relative to workspace root).
const SCAN_DIR: &str = "clients/tui/src";

/// File extensions to scan.
const SCAN_EXTS: &[&str] = &["rs"];

fn workspace_root() -> PathBuf {
    MetadataCommand::new()
        .exec()
        .expect("cargo metadata")
        .workspace_root
        .into()
}

fn strip_comments(line: &str) -> &str {
    // Extremely conservative: if `//` appears anywhere on the line,
    // everything from there onward is treated as a comment. This may
    // miss string-literal `//` cases, but this probe only needs to
    // avoid flagging prose in `tracing::…("foo desc.kind == bar")`-style
    // log messages — and log messages containing the forbidden token
    // are themselves a smell.
    line.find("//").map_or(line, |idx| &line[..idx])
}

fn is_forbidden_hit(stripped: &str) -> Option<&'static &'static str> {
    FORBIDDEN_PATTERNS.iter().find(|p| stripped.contains(**p))
}

fn walk_dir(dir: &Path, offenders: &mut Vec<(PathBuf, usize, String)>, self_path: &Path) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_dir() {
            walk_dir(&path, offenders, self_path);
            continue;
        }
        if !ft.is_file() {
            continue;
        }
        if path == self_path {
            continue;
        }
        let Some(ext) = path.extension().and_then(|e| e.to_str()) else {
            continue;
        };
        if !SCAN_EXTS.contains(&ext) {
            continue;
        }
        let Ok(src) = fs::read_to_string(&path) else {
            continue;
        };
        for (line_no, line) in src.lines().enumerate() {
            let stripped = strip_comments(line);
            if is_forbidden_hit(stripped).is_some() {
                offenders.push((path.clone(), line_no + 1, line.to_string()));
            }
        }
    }
}

#[test]
fn no_hardcoded_surface_decode_in_clients_tui() {
    let root = workspace_root();
    let scan_root = root.join(SCAN_DIR);
    assert!(scan_root.is_dir(), "scan root {scan_root:?} not found — layout drifted");

    let self_path = root.join("lib/depgraph/tests/no_hardcoded_surface_decode_leaks.rs");

    let mut offenders = Vec::new();
    walk_dir(&scan_root, &mut offenders, &self_path);

    assert!(
        offenders.is_empty(),
        "hardcoded surface descriptor decode pattern(s) found in \
         clients/tui/src/ — Plan 17-β.1 Phase C replaced these with \
         registry dispatch; re-introduction reopens the tight coupling \
         the flight was designed to remove. Offenders: {offenders:#?}",
    );
}
