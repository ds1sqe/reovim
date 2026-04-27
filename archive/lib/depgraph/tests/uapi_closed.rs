//! Enforces that `uapi/` membership is a closed, deliberately bounded set.
//!
//! `uapi/` crates define the stable cross-process wire contracts shared
//! by server and clients. Adding a new member must be an explicit,
//! reviewed decision — not a side effect of unrelated work. This test
//! asserts the workspace members rooted under `uapi/` exactly match
//! the allowlist below; any drift (addition or removal) forces an edit
//! here so the check becomes a review checkpoint.

use {
    cargo_metadata::MetadataCommand,
    std::{collections::BTreeSet, path::Path},
};

const UAPI_ALLOWLIST: &[&str] = &[
    "reovim-content-codec",
    "reovim-driver-macros",
    "reovim-input-codec",
    "reovim-module-macros",
    "reovim-protocol",
];

#[test]
fn uapi_membership_matches_allowlist() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");
    let workspace_root: &Path = metadata.workspace_root.as_std_path();

    let mut actual: BTreeSet<String> = BTreeSet::new();
    for pkg in &metadata.packages {
        let manifest = pkg.manifest_path.as_std_path();
        let Ok(rel) = manifest.strip_prefix(workspace_root) else {
            continue;
        };
        let dir = rel.parent().expect("manifest has parent");
        let rel_str = dir.to_string_lossy().replace('\\', "/");
        if rel_str == "uapi" || rel_str.starts_with("uapi/") {
            actual.insert(pkg.name.to_string());
        }
    }

    let expected: BTreeSet<String> = UAPI_ALLOWLIST.iter().map(|s| (*s).to_string()).collect();

    let added: Vec<&String> = actual.difference(&expected).collect();
    let removed: Vec<&String> = expected.difference(&actual).collect();

    assert!(
        added.is_empty() && removed.is_empty(),
        "uapi/ membership drift detected.\n\
         Added (new crates under uapi/ not in allowlist): {added:?}\n\
         Removed (allowlisted crates no longer present under uapi/): {removed:?}\n\
         \n\
         If this change is intended, update UAPI_ALLOWLIST in\n\
         lib/depgraph/tests/uapi_closed.rs to reflect the new closed set.",
    );
}
