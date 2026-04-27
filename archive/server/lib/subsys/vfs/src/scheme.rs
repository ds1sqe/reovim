//! VFS scheme - typed key for VFS provider lookup.
//!
//! Each URI scheme maps to a specific VFS provider:
//! - `file://` or empty → local filesystem
//! - `mem://` → in-memory filesystem (testing)
//! - `ssh://` → remote SSH filesystem (future)

use reovim_kernel::api::v1::ServiceKey;

/// Typed key for VFS provider lookup.
///
/// This enum defines all supported URI schemes for VFS providers.
/// Each variant maps to a specific filesystem implementation.
///
/// # Compile-Time Safety
///
/// Using typed keys instead of strings ensures:
/// - Typos are caught at compile time (`VfsScheme::Flie` → error)
/// - Exhaustive matching in `match` statements
/// - Self-documenting API (variants show supported schemes)
///
/// # Example
///
/// ```ignore
/// use reovim_subsys_vfs::{VfsScheme, VfsProviderRegistry};
///
/// let registry = VfsProviderRegistry::new();
/// registry.register(VfsScheme::File, Arc::new(LocalFsProvider::new()));
///
/// // Lookup with typed key
/// let provider = registry.get(&VfsScheme::File);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum VfsScheme {
    /// Local filesystem (file:// or empty scheme).
    ///
    /// This is the default scheme for local file paths.
    /// Handled by `LocalFsProvider` in `ext/server/modules/vfs-local`.
    File,

    /// In-memory filesystem (mem://).
    ///
    /// Used for testing and scratch buffers that don't persist to disk.
    Memory,

    /// SSH remote filesystem (ssh://).
    ///
    /// Future extension for editing remote files over SSH.
    Ssh,
}

impl VfsScheme {
    /// Parse scheme from URI string.
    ///
    /// Returns `None` for unknown schemes.
    ///
    /// # Example
    ///
    /// ```ignore
    /// assert_eq!(VfsScheme::from_uri_scheme("file"), Some(VfsScheme::File));
    /// assert_eq!(VfsScheme::from_uri_scheme(""), Some(VfsScheme::File));
    /// assert_eq!(VfsScheme::from_uri_scheme("mem"), Some(VfsScheme::Memory));
    /// assert_eq!(VfsScheme::from_uri_scheme("unknown"), None);
    /// ```
    #[must_use]
    pub fn from_uri_scheme(scheme: &str) -> Option<Self> {
        match scheme {
            "file" | "" => Some(Self::File),
            "mem" | "memory" => Some(Self::Memory),
            "ssh" | "sftp" => Some(Self::Ssh),
            _ => None,
        }
    }

    /// Get the canonical URI scheme string.
    ///
    /// Returns the standard scheme name for this variant.
    #[must_use]
    pub const fn as_str(&self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Memory => "mem",
            Self::Ssh => "ssh",
        }
    }
}

impl ServiceKey for VfsScheme {
    fn service_name() -> &'static str {
        "VFS"
    }
}

#[cfg(test)]
#[path = "scheme_tests.rs"]
mod tests;
