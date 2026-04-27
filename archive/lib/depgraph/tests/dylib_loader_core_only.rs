//! `lib/dylib-loader/` is the OS-abstraction mechanism beneath every
//! reovim cdylib consumer. It must stay in core: zero reovim-* deps,
//! zero edges into `clients/`, `server/`, `apps/`, `ext/`, or
//! `tools/`. The only permitted production deps are the external
//! crates needed for the mechanism itself: `libloading`, `rayon`,
//! `thiserror`, `tracing`.
//!
//! `[dev-dependencies]` are unrestricted (tests may pull in a Phase 0
//! `PoC` cdylib fixture for open + symbol round-trip).

use cargo_metadata::{DependencyKind, MetadataCommand};

const PERMITTED_EXTERNAL: &[&str] = &["libloading", "rayon", "thiserror", "tracing"];

#[test]
fn dylib_loader_has_no_reovim_deps() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");

    let pkg = metadata
        .packages
        .iter()
        .find(|p| p.name.as_str() == "reovim-dylib-loader")
        .expect("reovim-dylib-loader is a workspace member");

    let mut violations: Vec<String> = Vec::new();

    for dep in &pkg.dependencies {
        if dep.kind != DependencyKind::Normal {
            continue;
        }

        let name = dep.name.as_str();
        if name.starts_with("reovim-") || name.starts_with("reovim_") {
            violations
                .push(format!("reovim-dylib-loader must not depend on reovim-* crate `{name}`"));
            continue;
        }

        if !PERMITTED_EXTERNAL.contains(&name) {
            violations.push(format!(
                "reovim-dylib-loader depends on `{name}`, which is not in the \
                 permitted external set {PERMITTED_EXTERNAL:?}; \
                 extend this probe if the dep is justified"
            ));
        }
    }

    assert!(
        violations.is_empty(),
        "lib/dylib-loader/ core-only violations:\n  {}",
        violations.join("\n  "),
    );
}
