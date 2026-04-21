//! Guards the Plan 17-β.2 migration invariant.
//!
//! Counts the number of Rust files in the workspace that reference
//! `RenderSurface` and asserts the count is within the range
//! `[EXPECTED_MIN, EXPECTED_MAX]`. The probe has three jobs:
//!
//! 1. **Tripwire** — if someone reintroduces `RenderSurface` into a
//!    file that's already been migrated away, the count rises
//!    above the max and the probe fails.
//! 2. **Progress tracker** — as 17-β.2b-impl and 17-β.2c land
//!    consumer migrations, the max is lowered to match the new
//!    baseline. The commit that lowers the max is explicit, not a
//!    silent drift.
//! 3. **Deletion sentinel** — when 17-β.2c deletes the trait, the
//!    min drops to 0 and this probe is retired or repurposed into
//!    a "trait must not exist" probe.
//!
//! Phase D of Plan 20 / 17-β.2b-pre (session-budget-reduced scope):
//! this flight sets the initial range based on the current baseline
//! without migrating consumer files. 17-β.2b-impl tightens.

use cargo_metadata::MetadataCommand;
use std::{
    fs,
    path::{Path, PathBuf},
};

/// Range of acceptable `RenderSurface` file reference counts.
///
/// Timeline:
/// - 17-β.2b-pre landing: 40 files. Baseline set to `[30, 45]`.
/// - 17-β.2b-impl-a landing (Plan 21): 43+ (added 3 impl/test/probe
///   files naming `RenderSurface`). Range kept as the architectural
///   unlock files legitimately mention the trait.
/// - 17-β.2b-impl-b landing (Plan 22, pilot): 45. Pilot migrated
///   landing module test fixtures but `lib.rs` still takes
///   `&mut dyn RenderSurface` (migration is test-only; runtime
///   signatures stay until 17-β.2c trait deletion). Range widened
///   to `[25, 50]` to give room for bulk migration in 17-β.2b-impl-c
///   (lower bound drops) without flapping from incidental additions
///   (upper bound has ~5 slack).
const EXPECTED_MIN: usize = 25;
const EXPECTED_MAX: usize = 50;

/// Directory trees scanned for Rust files.
const SCAN_DIRS: &[&str] = &[
    "clients",
    "ext/client",
    "apps/bin",
    "lib",
    "server/lib",
    "tools",
    "uapi",
];

fn workspace_root() -> PathBuf {
    MetadataCommand::new()
        .exec()
        .expect("cargo metadata")
        .workspace_root
        .into()
}

fn file_mentions_rendersurface(path: &Path) -> bool {
    let Ok(src) = fs::read_to_string(path) else {
        return false;
    };
    src.contains("RenderSurface")
}

fn walk_count(dir: &Path, acc: &mut usize) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        let Ok(ft) = entry.file_type() else { continue };
        if ft.is_dir() {
            walk_count(&path, acc);
            continue;
        }
        if !ft.is_file() {
            continue;
        }
        if path.extension().and_then(|e| e.to_str()) != Some("rs") {
            continue;
        }
        if file_mentions_rendersurface(&path) {
            *acc += 1;
        }
    }
}

#[test]
fn rendersurface_file_count_within_expected_range() {
    let root = workspace_root();
    let mut count = 0usize;
    for dir_rel in SCAN_DIRS {
        let dir = root.join(dir_rel);
        if dir.is_dir() {
            walk_count(&dir, &mut count);
        }
    }

    assert!(
        (EXPECTED_MIN..=EXPECTED_MAX).contains(&count),
        "RenderSurface reference-count regression detected. \
         Counted {count} Rust files with `RenderSurface`; expected \
         range is [{EXPECTED_MIN}, {EXPECTED_MAX}]. \
         If above max: a consumer was re-added or a migration was \
         reverted. If below min: a migration landed in a flight that \
         didn't lower the bound; update EXPECTED_MAX in \
         lib/depgraph/tests/rendersurface_migration_progress.rs \
         explicitly.",
    );
}
