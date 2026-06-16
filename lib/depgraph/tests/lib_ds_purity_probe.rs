//! lib/ds ⊄ arch purity probe tests (SP01, architectural invariant 2).
//!
//! ## Invariant
//!
//! `lib/ds` crates must name no `arch` crate in any Cargo dependency table
//! AND contain no `arch::` path reference in any source file (outside
//! `#[cfg(test)]` blocks).  `lib/ds` reaches arch primitives (alloc, park)
//! ONLY through the `kabi` platform handle — never by naming `arch` directly.
//! The global handle static and `AllocError` live in `kabi`, not `arch`.
//!
//! ## Coverage
//!
//! 1. **Positive control (real tree)** — no `lib/ds` crate exists yet
//!    (pre-SP03), so the probe must return zero violations vacuously.
//!
//! 2. **Negative fixture A (Cargo dep)** — a synthetic `lib/ds` crate whose
//!    `[dependencies]` lists `reovim-arch` must trip the probe.
//!
//! 3. **Negative fixture B (source `arch::`)** — a synthetic `lib/ds` crate
//!    with a source file containing `use arch::alloc;` must trip the probe.
//!
//! 4. **Positive fixture** — a `lib/ds` crate with a dep on `reovim-kabi-platform`
//!    and a source file using `kabi::` (not `arch::`) must produce zero violations.
//!
//! 5. **cfg(test) exemption** — `arch::` inside a `#[cfg(test)] mod` does NOT
//!    trip the source-grep rule (test code may reference arch in tests).

mod common;

use reovim_depgraph::run_lib_ds_purity_probe;

// ── helpers ───────────────────────────────────────────────────────────────────

#[must_use]
fn root_workspace_toml(members: &[&str]) -> String {
    let list = members.iter().map(|m| format!("\"{}\"", m)).collect::<Vec<_>>().join(", ");
    format!("[workspace]\nresolver = \"2\"\nmembers = [{list}]\nexclude = [\"archive\"]\n")
}

#[must_use]
fn pkg_toml(name: &str) -> String {
    format!("[package]\nname = \"{name}\"\nversion = \"0.1.0\"\nedition = \"2024\"\n")
}

// ── 1. Positive control: real workspace ───────────────────────────────────────

/// The real workspace has no `lib/ds` crate yet (pre-SP03).  The probe must
/// return zero violations vacuously.
///
/// This becomes the production-enforcement test once SP03 lands `lib/ds`.
#[test]
fn lib_ds_purity_real_workspace_is_clean() {
    let root = common::workspace_root();
    let violations =
        run_lib_ds_purity_probe(&root).expect("lib/ds purity probe must run on real workspace");
    assert!(
        violations.is_empty(),
        "lib-ds-purity: real workspace has unexpected violations:\n{}",
        violations.join("\n")
    );
}

// ── 2. Negative fixture A: Cargo dep on reovim-arch ──────────────────────────

/// A synthetic `lib/ds` crate whose `[dependencies]` table lists `reovim-arch`
/// must trip the lib/ds purity probe.
///
/// This is the compile-time half of the invariant: even if no source file names
/// `arch::`, the Cargo edge itself violates the contract.
#[test]
fn lib_ds_purity_cargo_dep_on_arch_trips_probe() {
    let td = common::TempDir::new();
    let root = td.path();

    // arch backend (Foundation).
    common::write_file(root, "arch/Cargo.toml", &pkg_toml("reovim-arch"));
    // lib/ds crate with a DIRECT Cargo dep on arch (forbidden).
    common::write_file(
        root,
        "lib/ds/Cargo.toml",
        &format!(
            "{}\n[dependencies]\nreovim-arch = {{ path = \"../../arch\" }}\n",
            pkg_toml("reovim-lib-ds")
        ),
    );
    // A clean source file — the violation is in the manifest, not the source.
    common::write_file(root, "lib/ds/src/lib.rs", "#![no_std]\n// no arch:: here\n");

    common::write_file(root, "Cargo.toml", &root_workspace_toml(&["arch", "lib/ds"]));

    let violations =
        run_lib_ds_purity_probe(root).expect("lib/ds purity probe must run on fixture A");

    assert!(
        !violations.is_empty(),
        "lib-ds-purity fixture A: expected a violation for reovim-lib-ds dep on reovim-arch;\n\
         got zero violations"
    );
    let mentions_arch = violations.iter().any(|v| v.contains("reovim-arch"));
    assert!(
        mentions_arch,
        "lib-ds-purity fixture A: violation must mention reovim-arch;\n\
         violations: {:?}",
        violations
    );
}

// ── 3. Negative fixture B: source `arch::` reference ────────────────────────

/// A synthetic `lib/ds` crate with no Cargo dep on arch but with a source file
/// containing `use arch::alloc;` must trip the source-grep half of the probe.
///
/// The source check catches cases where the Cargo dep is absent but an
/// (incorrect) `extern crate arch;` or `use arch::` sneaks in.
#[test]
fn lib_ds_purity_source_arch_ref_trips_probe() {
    let td = common::TempDir::new();
    let root = td.path();

    // lib/ds crate with NO Cargo dep on arch, but with an arch:: source reference.
    common::write_file(root, "lib/ds/Cargo.toml", &pkg_toml("reovim-lib-ds"));
    common::write_file(
        root,
        "lib/ds/src/lib.rs",
        "#![no_std]\nuse arch::alloc::Layout;\n",
    );

    common::write_file(root, "Cargo.toml", &root_workspace_toml(&["lib/ds"]));

    let violations =
        run_lib_ds_purity_probe(root).expect("lib/ds purity probe must run on fixture B");

    assert!(
        !violations.is_empty(),
        "lib-ds-purity fixture B: expected a violation for `arch::` in lib/ds source;\n\
         got zero violations"
    );
    let mentions_source = violations.iter().any(|v| v.contains("arch::"));
    assert!(
        mentions_source,
        "lib-ds-purity fixture B: violation must mention the arch:: reference;\n\
         violations: {:?}",
        violations
    );
}

// ── 4. Positive fixture: kabi dep + kabi:: source is allowed ─────────────────

/// A `lib/ds` crate with a dep on `reovim-kabi-platform` (the correct
/// Foundation peer) and source using `kabi::` paths must produce zero
/// violations.
///
/// This is the intended post-SP02/SP03 shape: lib/ds reaches the allocator
/// through the kabi handle, not by naming arch directly.
#[test]
fn lib_ds_purity_kabi_dep_is_clean() {
    let td = common::TempDir::new();
    let root = td.path();

    // kabi/platform crate (future SP02 deliverable; inert name here).
    common::write_file(root, "kabi/platform/Cargo.toml", &pkg_toml("reovim-kabi-platform"));
    // lib/ds crate with a dep on kabi (allowed) and kabi:: source (allowed).
    common::write_file(
        root,
        "lib/ds/Cargo.toml",
        &format!(
            "{}\n[dependencies]\nreovim-kabi-platform = {{ path = \"../../kabi/platform\" }}\n",
            pkg_toml("reovim-lib-ds")
        ),
    );
    common::write_file(
        root,
        "lib/ds/src/lib.rs",
        "#![no_std]\n// Uses kabi handle, not arch directly.\n",
    );

    common::write_file(
        root,
        "Cargo.toml",
        &root_workspace_toml(&["kabi/platform", "lib/ds"]),
    );

    let violations =
        run_lib_ds_purity_probe(root).expect("lib/ds purity probe must run on positive fixture");

    assert!(
        violations.is_empty(),
        "lib-ds-purity positive fixture: kabi dep must NOT trip the probe;\n\
         violations: {:?}",
        violations
    );
}

// ── 5. cfg(test) exemption ────────────────────────────────────────────────────

/// `arch::` inside a `#[cfg(test)] mod tests { ... }` block must NOT trigger
/// a lib/ds purity violation.
///
/// The exemption mirrors the L11 and DAG6 exemptions: test code may reference
/// arch for test-only setup without violating the production invariant.
#[test]
fn lib_ds_purity_cfg_test_arch_ref_is_exempt() {
    let td = common::TempDir::new();
    let root = td.path();

    // lib/ds crate with arch:: ONLY inside #[cfg(test)] — must not trip.
    common::write_file(root, "lib/ds/Cargo.toml", &pkg_toml("reovim-lib-ds"));
    common::write_file(
        root,
        "lib/ds/src/lib.rs",
        "#![no_std]\n\
         #[cfg(test)]\n\
         mod tests {\n\
             // test-only: reference arch for integration test scaffolding.\n\
             use arch::alloc::Layout;\n\
             #[test]\n\
             fn it_works() { let _ = Layout::new::<u8>(); }\n\
         }\n",
    );

    common::write_file(root, "Cargo.toml", &root_workspace_toml(&["lib/ds"]));

    let violations =
        run_lib_ds_purity_probe(root)
            .expect("lib/ds purity probe must run on cfg(test) fixture");

    assert!(
        violations.is_empty(),
        "lib-ds-purity cfg(test) exemption: arch:: inside #[cfg(test)] mod must not be flagged;\n\
         violations: {:?}",
        violations
    );
}
