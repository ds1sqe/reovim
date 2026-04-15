//! Enforces source-level name-leak prevention for subsys and server layers.
//!
//! Test 1: subsys source files must have ZERO driver references (hard rule).
//! Test 2: server source driver references must not increase (regression guard).

use std::{fs, path::Path};

#[test]
fn subsys_source_has_no_driver_references() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.ancestors().nth(2).expect("workspace root");
    let subsys_dir = workspace_root.join("server/lib/subsys");

    assert!(subsys_dir.exists(), "subsys directory not found: {subsys_dir:?}");

    let mut violations = Vec::new();

    // Walk all subdirectories of subsys/
    for entry in fs::read_dir(&subsys_dir).expect("read subsys dir") {
        let entry = entry.expect("dir entry");
        let crate_src = entry.path().join("src");
        if !crate_src.is_dir() {
            continue;
        }
        scan_directory(&crate_src, &mut violations);
    }

    assert!(
        violations.is_empty(),
        "Subsys source files must not reference driver crate names. \
         Found {} violation(s):\n  {}",
        violations.len(),
        violations.join("\n  ")
    );
}

fn scan_directory(dir: &Path, violations: &mut Vec<String>) {
    for entry in fs::read_dir(dir).expect("read dir") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.is_dir() {
            scan_directory(&path, violations);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            scan_file(&path, violations);
        }
    }
}

/// Baseline count of `reovim_driver_` references in server source.
/// This number must NOT increase — it can only decrease as driver deps
/// are eliminated via the session facade effort (Tier 2 decoupling).
const SERVER_DRIVER_REF_BASELINE: usize = 605;

#[test]
fn server_source_driver_references_do_not_increase() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir.ancestors().nth(2).expect("workspace root");
    let server_src = workspace_root.join("server/lib/server/src");

    assert!(server_src.exists(), "server src directory not found: {server_src:?}");

    let mut count = 0;
    let mut refs = Vec::new();
    scan_directory_count(&server_src, &mut count, &mut refs);

    assert!(
        count <= SERVER_DRIVER_REF_BASELINE,
        "Server source `reovim_driver_` reference count increased: \
         {count} > baseline {SERVER_DRIVER_REF_BASELINE}. \
         New references must be justified. Reduce driver coupling, \
         don't increase it."
    );
}

fn scan_directory_count(dir: &Path, count: &mut usize, refs: &mut Vec<String>) {
    for entry in fs::read_dir(dir).expect("read dir") {
        let entry = entry.expect("dir entry");
        let path = entry.path();
        if path.is_dir() {
            scan_directory_count(&path, count, refs);
        } else if path.extension().is_some_and(|ext| ext == "rs") {
            scan_file_count(&path, count, refs);
        }
    }
}

fn scan_file_count(path: &Path, count: &mut usize, refs: &mut Vec<String>) {
    let content = fs::read_to_string(path).expect("read file");
    for (line_num, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        if trimmed.starts_with("//") {
            continue;
        }
        if trimmed.contains("reovim_driver_") {
            *count += 1;
            refs.push(format!("{}:{}", path.display(), line_num + 1));
        }
    }
}

fn scan_file(path: &Path, violations: &mut Vec<String>) {
    let content = fs::read_to_string(path).expect("read file");
    for (line_num, line) in content.lines().enumerate() {
        let trimmed = line.trim();
        // Skip single-line comments
        if trimmed.starts_with("//") {
            continue;
        }
        if trimmed.contains("reovim_driver_") || trimmed.contains("reovim-driver-") {
            violations.push(format!("{}:{}: {}", path.display(), line_num + 1, trimmed));
        }
    }
}
