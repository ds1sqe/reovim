//! Smoke tests for structural invariants in phase 3.
//!
//! Ensures the `raw_bytes` identifier is not present in the codec / provider /
//! commands source trees now that canonical bytes are represented as
//! `Inode`/`ByteSource` state.

use std::{
    fs::{self, DirEntry},
    path::{Path, PathBuf},
};

#[test]
fn no_raw_bytes_identifier_in_source_tree() {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let workspace_root = manifest_dir
        .ancestors()
        .nth(4)
        .expect("codec crate should live under ext/server/drivers/codec");
    let scan_roots = [
        manifest_dir.join("src"),
        workspace_root.join("ext/server/providers/text/src"),
        workspace_root.join("ext/server/modules/commands/src"),
    ];

    let matches = scan_roots
        .iter()
        .map(|root| scan_for_raw_bytes_identifier(root))
        .collect::<Result<Vec<_>, _>>()
        .expect("filesystem scan for phase-3 source roots should not fail")
        .into_iter()
        .flatten()
        .collect::<Vec<_>>();

    assert!(
        matches.is_empty(),
        "found 'raw_bytes' in source tree; this indicates an unsupported parallel byte cache path\n{matches:?}",
    );
}

fn scan_for_raw_bytes_identifier(root: &Path) -> std::io::Result<Vec<PathBuf>> {
    let mut todo: Vec<PathBuf> = vec![root.to_path_buf()];
    let mut found = Vec::new();

    while let Some(dir) = todo.pop() {
        for entry in fs::read_dir(&dir)? {
            let entry = entry?;
            if should_skip(&entry) {
                continue;
            }

            let path = entry.path();
            if path.is_dir() {
                todo.push(path);
            } else if path.is_file()
                && let Some(ext) = path.extension()
                && (ext == "rs")
            {
                let contents = fs::read_to_string(&path)?;
                if contains_raw_bytes_identifier(&contents) {
                    found.push(path);
                }
            }
        }
    }

    Ok(found)
}

fn should_skip(entry: &DirEntry) -> bool {
    entry
        .path()
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with('.'))
}

fn contains_raw_bytes_identifier(contents: &str) -> bool {
    let mut in_block_comment = false;

    for line in contents.lines() {
        let trimmed = line.trim_start();

        if in_block_comment {
            if trimmed.contains("*/") {
                in_block_comment = false;
            }
            continue;
        }

        if trimmed.starts_with("/*") {
            if !trimmed.contains("*/") {
                in_block_comment = true;
            }
            continue;
        }

        if trimmed.starts_with("//") {
            continue;
        }

        if trimmed.contains("raw_bytes") {
            return true;
        }
    }

    false
}
