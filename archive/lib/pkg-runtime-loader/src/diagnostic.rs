//! Generic helper that enriches a per-entry load error with the
//! package name recovered from the cdylib filename, when the
//! filename matches the reovim convention.
//!
//! Both client (`ValidationError`) and server (`ModuleError`)
//! enrichment paths consume this helper. The wrapping shape is
//! caller-supplied via the `wrap` closure so neither side imports
//! the other side's error type.

use std::path::Path;

use crate::package_lookup::package_name_for_path;

/// Wrap `err` in `E::AbiMismatchAtPackage`-shaped variant if the
/// cdylib at `path` follows the reovim package-naming convention.
///
/// - When `package_name_for_path(path)` returns `Some(name)`, the
///   `wrap` closure is invoked with `(name, err)` and its result is
///   returned.
/// - When the filename does not match the convention OR the recovered
///   name is empty (e.g. `libreovim_pkg_.so`), the original error is
///   returned unchanged via `E::from(err)`. No panic, no silent
///   rewrite.
///
/// Generic over `Src` so the same helper services
/// `ValidationError` (client side) and `ModuleError` (server side).
pub fn enrich_validation_error<Src, E>(
    path: &Path,
    err: Src,
    wrap: impl FnOnce(String, Src) -> E,
) -> E
where
    E: From<Src>,
{
    match package_name_for_path(path) {
        Some(name) if !name.is_empty() => wrap(name, err),
        _ => E::from(err),
    }
}

#[cfg(test)]
#[path = "diagnostic_tests.rs"]
mod diagnostic_tests;
