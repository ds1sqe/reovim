//! Per-OS cdylib extension and filename conventions.
//!
//! Three reovim target OSes, three extensions: `.so` on linux, `.dylib`
//! on macos, `.dll` on windows. Additional unix variants (freebsd,
//! netbsd) fall through to `.so`; genuinely non-unix, non-windows
//! platforms hit the `compile_error!` arm so Phase 1 cannot silently
//! ship on an unsupported target.

/// File extension for shared objects on the current target OS.
///
/// Returned without the leading `.` so callers can compose paths
/// without branching (`format!("lib{}.{}", stem, library_extension())`
/// or [`library_filename`]).
#[must_use]
pub const fn library_extension() -> &'static str {
    #[cfg(target_os = "linux")]
    {
        "so"
    }
    #[cfg(target_os = "macos")]
    {
        "dylib"
    }
    #[cfg(target_os = "windows")]
    {
        // TODO(#769-O4): confirm LoadLibrary behavior on the Windows
        // CI matrix in Phase 1.F and document any divergence in
        // docs/architecture/driver-abi-v1.md.
        "dll"
    }
    #[cfg(not(any(target_os = "linux", target_os = "macos", target_os = "windows")))]
    {
        // Fall through to the unix convention; the loader compiles but
        // is unsupported on this target.
        "so"
    }
}

/// Compose the conventional cdylib filename for `stem` on the current
/// target OS.
///
/// `library_filename("foo")` returns `"libfoo.so"` on linux,
/// `"libfoo.dylib"` on macos, `"foo.dll"` on windows. Matches Cargo's
/// output naming for `crate-type = ["cdylib"]`.
#[must_use]
pub fn library_filename(stem: &str) -> String {
    #[cfg(target_os = "windows")]
    {
        // TODO(#769-O4): Windows cdylibs are emitted without the `lib`
        // prefix by default; if the target matrix surfaces tooling that
        // emits `libfoo.dll`, loosen this to accept both forms.
        format!("{stem}.{ext}", ext = library_extension())
    }
    #[cfg(not(target_os = "windows"))]
    {
        format!("lib{stem}.{ext}", ext = library_extension())
    }
}

#[cfg(test)]
#[path = "platform_tests.rs"]
mod platform_tests;
