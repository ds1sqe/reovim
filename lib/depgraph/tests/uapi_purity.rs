//! L11 purity probe tests — Phase 3 (plan 05-uapi-foundation.md §Phase 3).
//!
//! ## Coverage
//!
//! 1. **Positive control**: the real `uapi/protocol/src/` directory produces
//!    zero `ForbiddenExternalImport` violations.
//!
//! 2. **Negative fixture (a)**: a temp `src/` containing `use arch::foo;`
//!    produces exactly one `ForbiddenExternalImport` for `"arch"`.
//!
//! 3. **Negative fixture (b)**: a temp `src/` containing
//!    `extern crate std;` produces exactly one violation for `"std"`.
//!
//! 4. **Negative fixture (c)**: a temp `src/` with `use alloc::vec::Vec;`
//!    produces exactly one violation for `"alloc"`.
//!
//! 5. **Positive fixture (d)**: permitted imports (`use core::mem;`,
//!    `use reovim_uapi_abi::ErrorCode;`, `use crate::codec::Encoder;`)
//!    produce zero violations.
//!
//! 6. **cfg(test) exemption**: `use std::` inside a `#[cfg(test)] mod`
//!    block does NOT trigger a violation.
//!
//! Source: Documentation/07-Surfaces/03-Server-Client-Protocol.md L11.

mod common;

use reovim_depgraph::{Violation, run_l11_purity_probe};

// ---------------------------------------------------------------------------
// 1. Positive control — real uapi/protocol/src must be violation-free
// ---------------------------------------------------------------------------

/// The real `uapi/protocol/src/` directory must produce zero L11 violations.
/// This is the production-enforcement test for L11.
#[test]
fn l11_real_uapi_protocol_src_is_clean() {
    let root = common::workspace_root();
    let src = root.join("uapi").join("protocol").join("src");
    assert!(src.is_dir(), "L11: uapi/protocol/src must exist at `{}`", src.display());
    let violations = run_l11_purity_probe(&src).expect("L11 probe must run on real source");
    let forbidden: Vec<_> = violations
        .iter()
        .filter(|v| matches!(v, Violation::ForbiddenExternalImport { .. }))
        .collect();
    assert!(
        forbidden.is_empty(),
        "L11: uapi/protocol/src must have zero ForbiddenExternalImport violations;\n\
         found: {forbidden:?}"
    );
}

// ---------------------------------------------------------------------------
// 2. Negative fixture (a): `use arch::` is forbidden
// ---------------------------------------------------------------------------

/// A source file with `use arch::foo;` must produce one `ForbiddenExternalImport`
/// violation for `"arch"`.
#[test]
fn l11_negative_fixture_a_use_arch() {
    let td = common::TempDir::new();
    let src = td.path().join("src");
    common::write_file(td.path(), "src/lib.rs", "#![no_std]\nuse arch::syscall::write;\n");

    let violations = run_l11_purity_probe(&src).expect("L11 probe must run");
    let has_arch = violations.iter().any(|v| {
        matches!(
            v,
            Violation::ForbiddenExternalImport { import, .. } if import == "arch"
        )
    });
    assert!(
        has_arch,
        "L11 fixture (a): expected ForbiddenExternalImport for 'arch';\n\
         violations: {violations:?}"
    );
    // No other kinds expected from this fixture.
    let unexpected: Vec<_> = violations
        .iter()
        .filter(|v| !matches!(v, Violation::ForbiddenExternalImport { .. }))
        .collect();
    assert!(
        unexpected.is_empty(),
        "L11 fixture (a): unexpected extra violation kinds: {unexpected:?}"
    );
}

// ---------------------------------------------------------------------------
// 3. Negative fixture (b): `extern crate std;` is forbidden
// ---------------------------------------------------------------------------

/// A source file with `extern crate std;` must produce one
/// `ForbiddenExternalImport` violation for `"std"`.
#[test]
fn l11_negative_fixture_b_extern_crate_std() {
    let td = common::TempDir::new();
    let src = td.path().join("src");
    common::write_file(td.path(), "src/lib.rs", "#![no_std]\nextern crate std;\n");

    let violations = run_l11_purity_probe(&src).expect("L11 probe must run");
    let has_std = violations.iter().any(|v| {
        matches!(
            v,
            Violation::ForbiddenExternalImport { import, .. } if import == "std"
        )
    });
    assert!(
        has_std,
        "L11 fixture (b): expected ForbiddenExternalImport for 'std';\n\
         violations: {violations:?}"
    );
}

// ---------------------------------------------------------------------------
// 4. Negative fixture (c): `use alloc::` is forbidden
// ---------------------------------------------------------------------------

/// A source file with `use alloc::vec::Vec;` must produce one
/// `ForbiddenExternalImport` violation for `"alloc"`.
#[test]
fn l11_negative_fixture_c_use_alloc() {
    let td = common::TempDir::new();
    let src = td.path().join("src");
    common::write_file(td.path(), "src/lib.rs", "#![no_std]\nuse alloc::vec::Vec;\n");

    let violations = run_l11_purity_probe(&src).expect("L11 probe must run");
    let has_alloc = violations.iter().any(|v| {
        matches!(
            v,
            Violation::ForbiddenExternalImport { import, .. } if import == "alloc"
        )
    });
    assert!(
        has_alloc,
        "L11 fixture (c): expected ForbiddenExternalImport for 'alloc';\n\
         violations: {violations:?}"
    );
}

// ---------------------------------------------------------------------------
// 5. Positive fixture (d): permitted imports produce zero violations
// ---------------------------------------------------------------------------

/// `use core::`, `use reovim_uapi_abi::`, and `use crate::` must not be
/// flagged by the L11 probe.
#[test]
fn l11_positive_fixture_d_permitted_imports_are_clean() {
    let td = common::TempDir::new();
    let src = td.path().join("src");
    common::write_file(
        td.path(),
        "src/lib.rs",
        "#![no_std]\n\
         use core::mem::size_of;\n\
         use reovim_uapi_abi::ErrorCode;\n\
         use crate::codec::Encoder;\n\
         use self::other::Foo;\n\
         use super::bar::Baz;\n",
    );

    let violations = run_l11_purity_probe(&src).expect("L11 probe must run");
    let forbidden: Vec<_> = violations
        .iter()
        .filter(|v| matches!(v, Violation::ForbiddenExternalImport { .. }))
        .collect();
    assert!(
        forbidden.is_empty(),
        "L11 fixture (d): permitted imports must produce zero violations;\n\
         found: {forbidden:?}"
    );
}

// ---------------------------------------------------------------------------
// 6. cfg(test) exemption: `use std::` inside #[cfg(test)] mod is skipped
// ---------------------------------------------------------------------------

/// `use std::` inside a `#[cfg(test)] mod tests { ... }` block must NOT
/// produce an L11 violation (bootstrap state 1: test code links std).
#[test]
fn l11_cfg_test_block_exemption() {
    let td = common::TempDir::new();
    let src = td.path().join("src");
    common::write_file(
        td.path(),
        "src/lib.rs",
        "#![no_std]\n\
         \n\
         #[cfg(test)]\n\
         mod tests {\n\
             use std::vec::Vec;\n\
             #[test]\n\
             fn it_works() { let _v: Vec<u8> = Vec::new(); }\n\
         }\n",
    );

    let violations = run_l11_purity_probe(&src).expect("L11 probe must run");
    let forbidden: Vec<_> = violations
        .iter()
        .filter(|v| matches!(v, Violation::ForbiddenExternalImport { .. }))
        .collect();
    assert!(
        forbidden.is_empty(),
        "L11 cfg(test) exemption: use std:: inside #[cfg(test)] mod must not be flagged;\n\
         found: {forbidden:?}"
    );
}
