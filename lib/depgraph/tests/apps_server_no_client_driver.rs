//! Server-tier crates must not compile-time depend on client-tier
//! driver crates (#775 / master-plan rule M3).
//!
//! `reovim-driver-display` is a CLIENT-tier extension crate
//! (`ext/client/tui/drivers/display/`). Server-tier code that
//! type-imports from it inverts the layer arrow: in the embedded
//! launcher both halves link in one process and the inversion
//! compiles, masking the architectural fault; in subprocess
//! deployments the server bin doesn't even need a renderer yet still
//! drags the display crate (and its transitive deps) into the link.
//!
//! Sub-plan 01 of the `ABI Deferrals` mission closed the three
//! existing leaks (#775 — `apps/server/src/bootstrap.rs` plus
//! `ext/server/modules/commands/src/colorscheme.rs`) by routing those
//! call sites through the new server-tier
//! `reovim-driver-display-registry` crate. This probe is the ratchet
//! that prevents future regressions: any `[dependencies]` on a
//! client-tier driver crate from a server-tier package fails the
//! build.
//!
//! # Forbidden client-tier driver crates
//!
//! `FORBIDDEN_CLIENT_DRIVERS` lists the client-tier driver crate
//! names this probe forbids in non-dev deps of server-tier
//! packages. The list starts at `["reovim-driver-display"]` and
//! extends as new client-tier driver crates land. To add a new
//! entry: append the crate name and document the rationale in the
//! list comment, then run the test to verify no existing leak.
//!
//! # Server-tier package shape
//!
//! A package is "server-tier" if its manifest path is anywhere under
//! `apps/server/`, `server/lib/`, or `ext/server/`. The probe
//! tolerates `[dev-dependencies]` (kind = "dev") because integration
//! tests may legitimately link a renderer to drive end-to-end
//! scenarios; only `[dependencies]` and `[build-dependencies]` are
//! checked.

use std::path::PathBuf;

use cargo_metadata::{DependencyKind, MetadataCommand};

/// Client-tier driver crates server-tier packages must not depend on.
///
/// Populated initially by #775 (sub-plan 01). Extend by appending the
/// client-tier driver crate name when a new leak is discovered or
/// preempted.
const FORBIDDEN_CLIENT_DRIVERS: &[&str] = &["reovim-driver-display"];

#[test]
fn server_tier_packages_have_no_client_driver_deps() {
    let metadata = MetadataCommand::new().exec().expect("cargo metadata");
    let workspace_root = PathBuf::from(metadata.workspace_root.as_str());

    let mut violations: Vec<String> = Vec::new();
    for pkg in metadata.workspace_packages() {
        let manifest = PathBuf::from(pkg.manifest_path.as_str());
        let rel = manifest
            .strip_prefix(&workspace_root)
            .unwrap_or(&manifest)
            .to_path_buf();

        let is_server_tier = rel.starts_with("apps/server/")
            || rel.starts_with("server/lib/")
            || rel.starts_with("ext/server/");
        if !is_server_tier {
            continue;
        }

        for dep in &pkg.dependencies {
            // Allow dev-dependencies (integration tests may link a
            // renderer for end-to-end scenarios).
            if dep.kind == DependencyKind::Development {
                continue;
            }
            if FORBIDDEN_CLIENT_DRIVERS.contains(&dep.name.as_str()) {
                violations.push(format!(
                    "{} (at {}) depends on client-tier crate `{}`",
                    pkg.name,
                    rel.display(),
                    dep.name,
                ));
            }
        }
    }
    violations.sort();

    assert!(
        violations.is_empty(),
        "server-tier crates depend on forbidden client-tier driver \
         crates (#775 / master-plan M3):\n  {}\nForbidden list: {:?}",
        violations.join("\n  "),
        FORBIDDEN_CLIENT_DRIVERS,
    );
}
