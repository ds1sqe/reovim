//! Guard: no `clients/<platform>/` directory may exist after Phase F.
//!
//! The #753 Client Foundation chain deletes `clients/tui/`, `clients/cli/`,
//! and `clients/web/` (platform runner directories) by Phase F. Platform
//! runtimes move to `ext/client/platforms/<p>/`. After Phase F the only
//! content under `clients/` is `clients/lib/`.
//!
//! This probe performs a filesystem walk of `clients/` to detect any
//! directory that is a sibling of `lib/` — those are the legacy platform
//! dirs that must not exist post-Phase-F.
//!
//! Activation phase: Phase F landing.
//!
//! This probe is `#[ignore]`-gated today (Phase A) because `clients/tui/`,
//! `clients/cli/`, and `clients/web/` still exist. Removing the `#[ignore]`
//! is the Phase F landing gate.
//!
//! Unconditional assertion body: the probe's assertion code is always present.
//! Do not add an `if dirs_exist { return }` short-circuit — that would invert
//! the semantics (this probe fails when a platform dir IS present).

use {
    cargo_metadata::MetadataCommand,
    std::{fs, path::Path},
};

#[test]
#[ignore = "activate at Phase F landing when clients/<platform>/ dirs are deleted"]
fn no_client_platform_directories_exist() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");
    let workspace_root: &Path = metadata.workspace_root.as_std_path();
    let clients_dir = workspace_root.join("clients");

    let Ok(entries) = fs::read_dir(&clients_dir) else {
        // clients/ does not exist — which is unexpected but vacuously passes
        // this specific probe.
        return;
    };

    let mut platform_dirs: Vec<String> = Vec::new();
    for entry in entries.flatten() {
        let ft = entry.file_type().expect("file type");
        if !ft.is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().into_owned();
        // `lib` is the only allowed subdirectory under clients/ post-Phase-F.
        if name != "lib" {
            platform_dirs.push(name);
        }
    }

    assert!(
        platform_dirs.is_empty(),
        "Legacy clients/<platform>/ directories still exist after Phase F. \
         Only clients/lib/ should remain; platform runtimes live in \
         ext/client/platforms/<p>/ (#753). Found: {platform_dirs:?}"
    );
}
