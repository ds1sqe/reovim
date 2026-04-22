//! Guard: `ext/client/` top-level must be exactly the four category
//! directories `{platforms, driver, module, capabilities}`.
//!
//! The #753 Client Foundation master plan (§Architecture) defines a flat
//! category tree under `ext/client/`. Exactly these four directory names are
//! permitted at the top level. Any additional directory name (e.g., a legacy
//! `tui/` after Phase F) or a missing category causes this probe to fail.
//!
//! Activation phase: Phase E.1 landing (when the flat category structure is
//! fully in place).
//!
//! This probe is `#[ignore]`-gated today (Phase A) because `ext/client/`
//! currently contains only the legacy `tui/` sub-tree. Removing the
//! `#[ignore]` is the Phase E.1 landing gate; the probe file is landed now
//! so that the rule is visible and auditable from Phase A onward.
//!
//! Unconditional assertion body: the probe's assertion code is always present.
//! Vacuous pass / ignore is a property of the activation phase, not of the
//! code. Do not add an `if input is empty { return }` short-circuit.

use {
    cargo_metadata::MetadataCommand,
    std::{collections::BTreeSet, fs, path::Path},
};

const REQUIRED_CATEGORIES: &[&str] = &["platforms", "driver", "module", "capabilities"];

#[test]
#[ignore = "activate at Phase E.1 landing when ext/client/ flat structure is complete"]
fn ext_client_top_level_is_exactly_four_categories() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");
    let workspace_root: &Path = metadata.workspace_root.as_std_path();
    let ext_client = workspace_root.join("ext/client");

    let Ok(entries) = fs::read_dir(&ext_client) else {
        panic!("ext/client/ directory does not exist at {}", ext_client.display());
    };

    let mut found: BTreeSet<String> = BTreeSet::new();
    for entry in entries.flatten() {
        let ft = entry.file_type().expect("file type");
        if ft.is_dir() {
            found.insert(entry.file_name().to_string_lossy().into_owned());
        }
    }

    let required: BTreeSet<&str> = REQUIRED_CATEGORIES.iter().copied().collect();
    let required_owned: BTreeSet<String> = required.iter().map(|s| (*s).to_string()).collect();

    let missing: BTreeSet<&String> = required_owned.difference(&found).collect();
    let extra: BTreeSet<&String> = found.difference(&required_owned).collect();

    assert!(
        missing.is_empty() && extra.is_empty(),
        "ext/client/ top-level directory structure is wrong.\n\
         Missing categories: {missing:?}\n\
         Extra/unexpected directories: {extra:?}\n\
         Expected exactly: {REQUIRED_CATEGORIES:?}\n\
         If a new category is needed, update the master plan §Architecture \
         and REQUIRED_CATEGORIES in this probe in the same commit."
    );
}
