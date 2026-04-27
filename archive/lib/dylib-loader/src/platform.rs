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
        format!("{stem}.{ext}", ext = library_extension())
    }
    #[cfg(not(target_os = "windows"))]
    {
        format!("lib{stem}.{ext}", ext = library_extension())
    }
}

/// Compose the conventional cdylib filename for a reovim package
/// name, e.g. `"libreovim_pkg_my_theme.so"` for `"my-theme"` on
/// linux.
///
/// Builds the cargo `cdylib` stem `reovim_pkg_<snake_name>` (hyphens
/// replaced with underscores) and delegates to [`library_filename`]
/// for the OS-specific `lib...` prefix and `.so` / `.dylib` / `.dll`
/// extension. Used by the package manager to locate and place
/// extension cdylibs.
#[must_use]
pub fn cdylib_filename(pkg_name: &str) -> String {
    let stem = format!("reovim_pkg_{}", pkg_name.replace('-', "_"));
    library_filename(&stem)
}

/// Inverse of [`cdylib_filename`]: parse a cdylib filename into its
/// reovim package name.
///
/// Returns `Some(name)` when `filename` matches the reovim convention
/// (`libreovim_pkg_<snake>.<ext>` on linux/macos, `reovim_pkg_<snake>.dll`
/// on windows). Returns `None` for any filename outside that
/// convention — the caller should treat such cdylibs as foreign.
#[must_use]
pub fn pkg_name_from_cdylib_filename(filename: &str) -> Option<String> {
    #[cfg(target_os = "windows")]
    const PREFIX: &str = "";
    #[cfg(not(target_os = "windows"))]
    const PREFIX: &str = "lib";
    let suffix = format!(".{}", library_extension());
    let stem = filename.strip_prefix(PREFIX)?.strip_suffix(&suffix)?;
    let snake = stem.strip_prefix("reovim_pkg_")?;
    Some(snake.replace('_', "-"))
}

#[cfg(test)]
#[path = "platform_tests.rs"]
mod platform_tests;
