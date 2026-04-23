//! Safe wrapper over [`libloading::Library`].
//!
//! `Library` owns the `dlopen` handle for the duration of its lifetime
//! and drops it (calling `dlclose` on unix, `FreeLibrary` on windows)
//! via `libloading`'s `Drop` impl. Symbol handles borrow from the
//! owning `Library` so the borrow checker prevents use-after-free.

use {crate::error::LoaderError, std::path::Path};

/// Re-export of `libloading::Symbol` so callers do not need a direct
/// `libloading` dependency.
///
/// The lifetime parameter ties the symbol to the owning [`Library`]
/// so a symbol cannot outlive the dylib it came from.
pub type Symbol<'lib, T> = libloading::Symbol<'lib, T>;

/// Owned handle to a dynamically loaded shared object.
///
/// `Library` is a thin safety-preserving wrapper: it forbids exposing
/// the underlying `libloading::Library` (so consumers cannot bypass the
/// error-type abstraction) and it gates symbol lookup through an
/// `unsafe fn` whose safety contract matches the upstream crate.
#[derive(Debug)]
pub struct Library(libloading::Library);

impl Library {
    /// Open the shared object at `path`.
    ///
    /// # Errors
    ///
    /// Returns [`LoaderError::LibraryOpen`] if `dlopen` / `LoadLibrary`
    /// fails for any reason — missing file, wrong format, missing
    /// dependency, permission denied.
    pub fn open(path: &Path) -> Result<Self, LoaderError> {
        // SAFETY: `libloading::Library::new` is unsafe because loading
        // a shared object runs its static initializers, which may
        // execute arbitrary code. The caller takes responsibility by
        // invoking `Library::open` on a path they trust (in reovim,
        // the specialized loaders validate the ABI vtable header
        // immediately after `open` returns). Wrapping the call here
        // centralizes the trust boundary in one place.
        let raw = unsafe { libloading::Library::new(path) }.map_err(|e| {
            LoaderError::LibraryOpen {
                path: path.to_path_buf(),
                source_text: e.to_string(),
            }
        })?;
        Ok(Self(raw))
    }

    /// Resolve a symbol by name.
    ///
    /// # Safety
    ///
    /// The caller MUST supply a type `T` whose layout matches the
    /// symbol's true type in the shared object. Mismatched types
    /// produce undefined behavior when the returned `Symbol` is
    /// dereferenced. The returned handle borrows from `self`; the
    /// borrow checker prevents the handle from outliving the library.
    ///
    /// # Errors
    ///
    /// Returns [`LoaderError::SymbolNotFound`] if the dynamic linker
    /// cannot locate `name` in the open library.
    pub unsafe fn symbol<T>(&self, name: &[u8]) -> Result<Symbol<'_, T>, LoaderError> {
        // SAFETY: forwarded from the caller's contract above; the
        // type-matching obligation is on the caller of `symbol`, not on
        // this crate.
        unsafe { self.0.get::<T>(name) }.map_err(|e| LoaderError::SymbolNotFound {
            symbol: String::from_utf8_lossy(name).into_owned(),
            source_text: e.to_string(),
        })
    }
}

