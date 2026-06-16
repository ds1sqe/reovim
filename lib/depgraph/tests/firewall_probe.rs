//! Direct-edge firewall probe tests (SP01, architectural invariant 1).
//!
//! ## Invariant
//!
//! No crate classified under `editor/**` or `client/**` may have a DIRECT
//! Cargo dependency edge naming an `arch`, `system/kernel`, or `*/drivers`
//! crate.  The invariant is stated at the strength it holds: *direct* edge
//! only.  Transitive World-fulness via `lib/ds → arch` is allowed — that path
//! flows through the `kabi` contract handle, not a direct kernel → arch edge.
//!
//! ## Coverage
//!
//! 1. **Positive control (real tree)** — the current real workspace has no
//!    `editor/**` or `client/**` crates yet (pre-SP04), so the probe must
//!    return zero violations vacuously.
//!
//! 2. **Negative fixture A** — a synthetic `editor/lib/kernel` crate with a
//!    direct `reovim-arch` dep must trip exactly one firewall violation.
//!
//! 3. **Negative fixture B** — a synthetic `client/platforms/tui` crate with
//!    a direct dep on a `system/kernel` crate must trip the probe.
//!
//! 4. **Negative fixture C** — a synthetic `editor/modules/vim` crate with a
//!    direct dep on a `*/drivers/*` crate must trip the probe.
//!
//! 5. **Positive fixture (transitive allowed)** — `editor/lib/kernel` → `lib/ds`
//!    → `arch` transitive path produces zero firewall violations (the firewall
//!    is direct-edge only; lib/ds is not arch).

mod common;

use reovim_depgraph::run_firewall_probe;

// ── helpers ───────────────────────────────────────────────────────────────────

/// Minimal workspace manifest.
#[must_use]
fn root_workspace_toml(members: &[&str]) -> String {
    let list = members.iter().map(|m| format!("\"{}\"", m)).collect::<Vec<_>>().join(", ");
    format!("[workspace]\nresolver = \"2\"\nmembers = [{list}]\nexclude = [\"archive\"]\n")
}

/// Minimal `[package]` manifest with no dependencies.
#[must_use]
fn pkg_toml(name: &str) -> String {
    format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n")
}

/// Minimal `[package]` manifest with one `[dependencies]` path dep.
#[must_use]
fn pkg_toml_with_path_dep(name: &str, dep_name: &str, dep_rel_path: &str) -> String {
    format!(
        "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\
         [dependencies]\n{dep_name} = {{ path = \"{dep_rel_path}\" }}\n"
    )
}

// ── 1. Positive control: real workspace ───────────────────────────────────────

/// The real workspace currently has no `editor/**` or `client/**` crates
/// (pre-SP04).  The firewall probe must return zero violations vacuously.
///
/// This is the production-enforcement test — it will start doing real work
/// once SP04 lands `editor/` and `client/` crates.
#[test]
fn firewall_real_workspace_is_clean() {
    let root = common::workspace_root();
    let violations = run_firewall_probe(&root).expect("firewall probe must run on real workspace");
    assert!(
        violations.is_empty(),
        "firewall: real workspace has unexpected violations:\n{}",
        violations.join("\n")
    );
}

// ── 2. Negative fixture A: editor/lib/kernel → reovim-arch ───────────────────

/// A synthetic `editor/lib/kernel` crate with a direct dep on `reovim-arch`
/// must trip the firewall probe.
///
/// This is the canonical post-SP04 violation shape: a Math kernel directly
/// naming the World backend.
#[test]
fn firewall_editor_kernel_to_arch_trips_probe() {
    let td = common::TempDir::new();
    let root = td.path();

    // arch crate at arch/ (Foundation).
    common::write_file(root, "arch/Cargo.toml", &pkg_toml("reovim-arch"));
    // editor/lib/kernel crate with a DIRECT dep on arch (forbidden).
    common::write_file(
        root,
        "editor/lib/kernel/Cargo.toml",
        &pkg_toml_with_path_dep(
            "reovim-editor-kernel",
            "reovim-arch",
            "../../../arch",
        ),
    );

    common::write_file(
        root,
        "Cargo.toml",
        &root_workspace_toml(&["arch", "editor/lib/kernel"]),
    );

    let violations =
        run_firewall_probe(root).expect("firewall probe must run on fixture");

    assert!(
        !violations.is_empty(),
        "firewall fixture A: expected a violation for editor/lib/kernel → reovim-arch;\n\
         got zero violations"
    );
    // The violation message must name both crates.
    let names_both = violations.iter().any(|v| {
        v.contains("reovim-editor-kernel") && v.contains("reovim-arch")
    });
    assert!(
        names_both,
        "firewall fixture A: violation message should name both crates;\n\
         violations: {:?}",
        violations
    );
}

// ── 3. Negative fixture B: client/platforms/tui → system/kernel ──────────────

/// A synthetic `client/platforms/tui` crate with a direct dep on a
/// `system/kernel` crate must trip the firewall probe.
#[test]
fn firewall_client_to_system_kernel_trips_probe() {
    let td = common::TempDir::new();
    let root = td.path();

    // system/kernel stub crate (future World kernel).
    common::write_file(root, "system/kernel/Cargo.toml", &pkg_toml("reovim-system-kernel"));
    // client/platforms/tui with a DIRECT dep on system/kernel (forbidden).
    common::write_file(
        root,
        "client/platforms/tui/Cargo.toml",
        &pkg_toml_with_path_dep(
            "reovim-client-tui",
            "reovim-system-kernel",
            "../../../system/kernel",
        ),
    );

    common::write_file(
        root,
        "Cargo.toml",
        &root_workspace_toml(&["system/kernel", "client/platforms/tui"]),
    );

    let violations =
        run_firewall_probe(root).expect("firewall probe must run on fixture");

    assert!(
        !violations.is_empty(),
        "firewall fixture B: expected a violation for client/platforms/tui → system/kernel;\n\
         got zero violations"
    );
    let names_dep = violations.iter().any(|v| v.contains("reovim-client-tui"));
    assert!(
        names_dep,
        "firewall fixture B: violation must name client/platforms/tui crate;\n\
         violations: {:?}",
        violations
    );
}

// ── 4. Negative fixture C: editor/modules/vim → editor/drivers/gpu ────────────

/// A synthetic `editor/modules/vim` (ServerExt) with a direct dep on
/// `editor/drivers/gpu` (a `*/drivers/*` crate) must trip the firewall probe.
///
/// The `*/drivers/*` part of the firewall blocks Math kernels from naming ANY
/// drivers leaf directly.
#[test]
fn firewall_editor_module_to_drivers_trips_probe() {
    let td = common::TempDir::new();
    let root = td.path();

    // A driver leaf.
    common::write_file(
        root,
        "editor/drivers/gpu/Cargo.toml",
        &pkg_toml("reovim-editor-driver-gpu"),
    );
    // An editor module with a DIRECT dep on a driver (forbidden: drivers are
    // swap-set leaves, not a dependency target for sibling kernel code).
    common::write_file(
        root,
        "editor/modules/vim/Cargo.toml",
        &pkg_toml_with_path_dep(
            "reovim-editor-module-vim",
            "reovim-editor-driver-gpu",
            "../../drivers/gpu",
        ),
    );

    common::write_file(
        root,
        "Cargo.toml",
        &root_workspace_toml(&["editor/drivers/gpu", "editor/modules/vim"]),
    );

    let violations =
        run_firewall_probe(root).expect("firewall probe must run on fixture");

    assert!(
        !violations.is_empty(),
        "firewall fixture C: expected a violation for editor/modules/vim → editor/drivers/gpu;\n\
         got zero violations"
    );
}

// ── 5. Positive fixture: transitive via lib/ds is allowed ────────────────────

/// A synthetic `editor/lib/kernel` → `lib/ds` edge must NOT trip the firewall.
///
/// The firewall is a DIRECT-edge rule.  `lib/ds` is not `arch`, not
/// `system/kernel`, and not `*/drivers/*` — it is a Foundation lib.  The
/// transitive path `editor → lib/ds → arch` is the allowed pattern; it flows
/// through the kabi handle, not a direct kernel → arch edge.
#[test]
fn firewall_editor_to_lib_ds_is_allowed() {
    let td = common::TempDir::new();
    let root = td.path();

    // lib/ds Foundation crate (future SP03 DS algorithm crate; inert name here).
    common::write_file(root, "lib/ds/Cargo.toml", &pkg_toml("reovim-lib-ds"));
    // editor/lib/kernel with a dep on lib/ds (allowed: lib/ds is Foundation, not arch).
    common::write_file(
        root,
        "editor/lib/kernel/Cargo.toml",
        &pkg_toml_with_path_dep(
            "reovim-editor-kernel",
            "reovim-lib-ds",
            "../../../lib/ds",
        ),
    );

    common::write_file(
        root,
        "Cargo.toml",
        &root_workspace_toml(&["lib/ds", "editor/lib/kernel"]),
    );

    let violations =
        run_firewall_probe(root).expect("firewall probe must run on fixture");

    assert!(
        violations.is_empty(),
        "firewall positive fixture: editor → lib/ds must NOT trip the firewall;\n\
         violations: {:?}",
        violations
    );
}
