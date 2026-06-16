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

use reovim_depgraph::{run_firewall_probe, run_no_product_arch_net_probe};

// ── helpers ───────────────────────────────────────────────────────────────────

/// Minimal workspace manifest.
#[must_use]
fn root_workspace_toml(members: &[&str]) -> String {
    let list = members
        .iter()
        .map(|m| format!("\"{m}\""))
        .collect::<Vec<_>>()
        .join(", ");
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

/// Manifest with an OPTIONAL, selftest-gated `reovim-arch` path dep, declared
/// in the inline-table form (`= { path = "..", optional = true }`) with a
/// `selftest = ["dep:reovim-arch"]` feature. This is the post-SP05 shape of a
/// crate whose only arch use is the selftest test-infra — the firewall must
/// NOT flag it.
#[must_use]
fn pkg_toml_optional_arch_inline(name: &str, dep_rel_path: &str) -> String {
    format!(
        "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\
         [features]\nselftest = [\"dep:reovim-arch\"]\n\
         [dependencies]\nreovim-arch = {{ path = \"{dep_rel_path}\", optional = true }}\n"
    )
}

/// Manifest with an OPTIONAL, selftest-gated `reovim-arch` path dep declared in
/// the SECTION-table form (`[dependencies.reovim-arch]` block). Same product
/// meaning as the inline form; proves the probe's dep parser reads `optional`
/// from both manifest shapes.
#[must_use]
fn pkg_toml_optional_arch_section(name: &str, dep_rel_path: &str) -> String {
    format!(
        "[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n\
         [features]\nselftest = [\"dep:reovim-arch\"]\n\
         [dependencies.reovim-arch]\npath = \"{dep_rel_path}\"\noptional = true\n"
    )
}

// ── 1. Positive control: real workspace ───────────────────────────────────────

/// After SP05 the firewall residual set is EXACTLY `{reovim-kernel,
/// reovim-platform-tui}`: SP05 routed server-rt's + tui's net/thread through
/// the `kabi` handle and made the (now test-only) arch dep optional + selftest-
/// gated in server-rt, subsys-domain, and domain-text, so those three crates'
/// product builds carry no arch edge. The kernel (panic/time/sys) and tui
/// (panic/sys/term) keep their arch edges for SP05b/c.
///
/// The assertion is `assert_eq!` on the SET of residual crates, not
/// `is_empty()`: it fails both on an unexpected NEW Math-kernel→arch edge AND
/// on a stale residual that should have been closed — safe incremental closure
/// across SP05b/c. The probe MECHANISM (and the optional-skip
/// precision) is proven by the negative fixtures below.
#[test]
fn firewall_real_workspace_residual_equals_kernel_and_tui() {
    use std::collections::BTreeSet;

    /// The crate names the firewall is still expected to flag after SP05.
    /// Order-independent (compared as a set). SP05b/c shrink this further.
    const EXPECTED_RESIDUAL: &[&str] = &["reovim-kernel", "reovim-platform-tui"];

    let root = common::workspace_root();
    let violations = run_firewall_probe(&root).expect("firewall probe must run on real workspace");

    // Reduce each violation line to the residual crate it names. A violation
    // line names exactly one of the expected residual crates when the closure
    // is correct; an UNEXPECTED line (naming none of them) is recorded as a raw
    // string so the failure message points at the regression.
    let mut residual: BTreeSet<&str> = BTreeSet::new();
    let mut unexpected: Vec<&str> = Vec::new();
    for v in &violations {
        match EXPECTED_RESIDUAL.iter().find(|k| v.contains(**k)) {
            Some(k) => {
                residual.insert(*k);
            }
            None => unexpected.push(v.as_str()),
        }
    }

    assert!(
        unexpected.is_empty(),
        "firewall: unexpected (non-residual) Math-kernel→arch violations \
         (a NEW direct arch edge regressed):\n{}",
        unexpected.join("\n")
    );

    let expected: BTreeSet<&str> = EXPECTED_RESIDUAL.iter().copied().collect();
    assert_eq!(
        residual, expected,
        "firewall: residual set must equal exactly {{reovim-kernel, \
         reovim-platform-tui}} after SP05 — a missing entry means a residual \
         was closed without updating this assertion (update it when SP05b/c land)"
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
        &pkg_toml_with_path_dep("reovim-editor-kernel", "reovim-arch", "../../../arch"),
    );

    common::write_file(root, "Cargo.toml", &root_workspace_toml(&["arch", "editor/lib/kernel"]));

    let violations = run_firewall_probe(root).expect("firewall probe must run on fixture");

    assert!(
        !violations.is_empty(),
        "firewall fixture A: expected a violation for editor/lib/kernel → reovim-arch;\n\
         got zero violations"
    );
    // The violation message must name both crates.
    let names_both = violations
        .iter()
        .any(|v| v.contains("reovim-editor-kernel") && v.contains("reovim-arch"));
    assert!(
        names_both,
        "firewall fixture A: violation message should name both crates;\n\
         violations: {violations:?}"
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

    let violations = run_firewall_probe(root).expect("firewall probe must run on fixture");

    assert!(
        !violations.is_empty(),
        "firewall fixture B: expected a violation for client/platforms/tui → system/kernel;\n\
         got zero violations"
    );
    let names_dep = violations.iter().any(|v| v.contains("reovim-client-tui"));
    assert!(
        names_dep,
        "firewall fixture B: violation must name client/platforms/tui crate;\n\
         violations: {violations:?}"
    );
}

// ── 4. Negative fixture C: editor/modules/vim → editor/drivers/gpu ────────────

/// A synthetic `editor/modules/vim` (`ServerExt`) with a direct dep on
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

    let violations = run_firewall_probe(root).expect("firewall probe must run on fixture");

    assert!(
        !violations.is_empty(),
        "firewall fixture C: expected a violation for editor/modules/vim → editor/drivers/gpu;\n\
         got zero violations"
    );
}

// ── 5b. No product arch::net/thread in server-rt + tui ────

/// After SP05, server-rt's and tui's PRODUCT source (non-`*_tests.rs`,
/// `#[cfg(test/selftest)]`-block-skipped) must name no `arch::net`/`arch::thread`/
/// `reovim_arch::net`/`reovim_arch::thread`: the net transport flows through the
/// kabi handle (lib/ds), not arch. This is the source-level companion to the
/// manifest-level firewall close — it catches a bypass the Cargo-edge probe
/// cannot see.
#[test]
fn no_product_arch_net_in_server_rt_and_tui() {
    let root = common::workspace_root();
    let violations =
        run_no_product_arch_net_probe(&root, &["reovim-server-rt", "reovim-platform-tui"])
            .expect("no-product-arch-net probe must run on real workspace");
    assert!(
        violations.is_empty(),
        "no-product-arch-net: server-rt/tui product code still names arch net/thread:\n{}",
        violations.join("\n")
    );
}

// ── 6. Optional-dep refinement: DUAL-SIDED proof ─────────────

/// (i) A synthetic `editor/lib/subsys/domain` crate whose ONLY arch edge is an
/// OPTIONAL, selftest-gated `reovim-arch` dep (inline-table form) must NOT trip
/// the firewall: an optional dep is not a product edge.
#[test]
fn firewall_optional_selftest_arch_dep_inline_does_not_trip() {
    let td = common::TempDir::new();
    let root = td.path();

    common::write_file(root, "arch/Cargo.toml", &pkg_toml("reovim-arch"));
    common::write_file(
        root,
        "editor/lib/subsys/domain/Cargo.toml",
        &pkg_toml_optional_arch_inline("reovim-subsys-domain", "../../../../arch"),
    );
    common::write_file(
        root,
        "Cargo.toml",
        &root_workspace_toml(&["arch", "editor/lib/subsys/domain"]),
    );

    let violations = run_firewall_probe(root).expect("firewall probe must run on fixture");
    assert!(
        violations.is_empty(),
        "firewall: an optional selftest-gated arch dep (inline form) must NOT trip;\n\
         violations: {violations:?}"
    );
}

/// (i, section-table form) Same as above but the optional dep is declared as a
/// `[dependencies.reovim-arch]` section table. Proves `optional` is parsed from
/// BOTH manifest shapes.
#[test]
fn firewall_optional_selftest_arch_dep_section_does_not_trip() {
    let td = common::TempDir::new();
    let root = td.path();

    common::write_file(root, "arch/Cargo.toml", &pkg_toml("reovim-arch"));
    common::write_file(
        root,
        "editor/domains/text/Cargo.toml",
        &pkg_toml_optional_arch_section("reovim-domain-text", "../../../arch"),
    );
    common::write_file(root, "Cargo.toml", &root_workspace_toml(&["arch", "editor/domains/text"]));

    let violations = run_firewall_probe(root).expect("firewall probe must run on fixture");
    assert!(
        violations.is_empty(),
        "firewall: an optional selftest-gated arch dep (section-table form) must NOT trip;\n\
         violations: {violations:?}"
    );
}

/// (ii) The precision side: an UNCONDITIONAL `reovim-arch` dep STILL trips the
/// firewall. Pairs with the optional cases above to prove the skip is precise —
/// it suppresses ONLY `optional = true` edges, never a genuine product edge.
#[test]
fn firewall_unconditional_arch_dep_still_trips() {
    let td = common::TempDir::new();
    let root = td.path();

    common::write_file(root, "arch/Cargo.toml", &pkg_toml("reovim-arch"));
    // Same crate path as the inline optional fixture, but the arch dep is
    // unconditional (no `optional = true`) — a product edge that must trip.
    common::write_file(
        root,
        "editor/lib/subsys/domain/Cargo.toml",
        &pkg_toml_with_path_dep("reovim-subsys-domain", "reovim-arch", "../../../../arch"),
    );
    common::write_file(
        root,
        "Cargo.toml",
        &root_workspace_toml(&["arch", "editor/lib/subsys/domain"]),
    );

    let violations = run_firewall_probe(root).expect("firewall probe must run on fixture");
    assert!(
        !violations.is_empty(),
        "firewall: an UNCONDITIONAL arch dep must STILL trip the probe;\n\
         got zero violations"
    );
    let names_both = violations
        .iter()
        .any(|v| v.contains("reovim-subsys-domain") && v.contains("reovim-arch"));
    assert!(
        names_both,
        "firewall: the unconditional-dep violation must name both crates;\n\
         violations: {violations:?}"
    );
}

// ── 7. Positive fixture: transitive via lib/ds is allowed ────────────────────

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
        &pkg_toml_with_path_dep("reovim-editor-kernel", "reovim-lib-ds", "../../../lib/ds"),
    );

    common::write_file(root, "Cargo.toml", &root_workspace_toml(&["lib/ds", "editor/lib/kernel"]));

    let violations = run_firewall_probe(root).expect("firewall probe must run on fixture");

    assert!(
        violations.is_empty(),
        "firewall positive fixture: editor → lib/ds must NOT trip the firewall;\n\
         violations: {violations:?}"
    );
}
