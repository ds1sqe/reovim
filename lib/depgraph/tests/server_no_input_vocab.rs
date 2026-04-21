//! Enforces that `reovim-server` has no production dependency on any
//! platform-specific input-vocabulary crate.
//!
//! Post Plan 14 Phase I, the server is a pure dispatch layer over
//! opaque `InputDescriptor` bytes. It must not reach into platform
//! input vocabularies (TUI Key enums, Web key codes, etc.) nor into
//! the input-codec driver layer. `[dev-dependencies]` are unrestricted
//! — integration tests are free to exercise any codec.
//!
//! This test is the regression guard for the input-opacity invariant
//! (Audit 2026-04-19).

use cargo_metadata::{DependencyKind, MetadataCommand};

const SERVER_CRATE: &str = "reovim-server";

const FORBIDDEN_DEPS: &[&str] = &[
    "reovim-input-codec",
    "reovim-codec-tui-input",
    "reovim-driver-text-input",
    "reovim-subsys-input-contracts",
];

#[test]
fn server_has_no_production_input_vocab_deps() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");

    let server_pkg = metadata
        .packages
        .iter()
        .find(|p| p.name == SERVER_CRATE)
        .unwrap_or_else(|| panic!("{SERVER_CRATE} not found in workspace"));

    let mut violations: Vec<String> = Vec::new();

    for dep in &server_pkg.dependencies {
        if dep.kind != DependencyKind::Normal {
            continue;
        }
        if FORBIDDEN_DEPS.contains(&dep.name.as_str()) {
            violations.push(dep.name.clone());
        }
    }

    assert!(
        violations.is_empty(),
        "{SERVER_CRATE} has production dependencies on forbidden input-vocabulary crates: {violations:?}\n\
         \n\
         The server must treat input as opaque bytes. Platform input vocabularies\n\
         (TUI Key, Web key codes, etc.) and input-codec drivers belong in the client\n\
         and driver layers — never in the server's production dep graph.\n\
         \n\
         Forbidden set: {FORBIDDEN_DEPS:?}",
    );
}
