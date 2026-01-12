//! Path utilities and normalization.
//!
//! Linux equivalent: `fs/namei.c` (path resolution)

#![allow(clippy::missing_errors_doc)]

use {
    crate::VfsError,
    std::{
        ffi::OsStr,
        path::{Path, PathBuf},
    },
};

/// Path normalization and manipulation.
///
/// Handles cross-platform path differences and provides utilities
/// for working with filesystem paths.
pub trait PathNormalizer: Send + Sync {
    /// Normalize a path (resolve `.`, `..`, but not symlinks).
    ///
    /// Unlike `canonicalize`, this does not access the filesystem
    /// and works on paths that may not exist.
    ///
    /// # Examples
    ///
    /// ```ignore
    /// let normalizer = StandardPathNormalizer;
    /// assert_eq!(
    ///     normalizer.normalize(Path::new("./foo/../bar")),
    ///     PathBuf::from("bar")
    /// );
    /// ```
    fn normalize(&self, path: &Path) -> PathBuf;

    /// Canonicalize a path (resolve symlinks, make absolute).
    ///
    /// This accesses the filesystem and may fail if the path doesn't exist.
    fn canonicalize(&self, path: &Path) -> Result<PathBuf, VfsError>;

    /// Check if a path is absolute.
    fn is_absolute(&self, path: &Path) -> bool;

    /// Join two paths.
    ///
    /// If `relative` is absolute, it replaces `base`.
    fn join(&self, base: &Path, relative: &Path) -> PathBuf;

    /// Get the parent directory.
    ///
    /// Returns `None` for root paths or paths without a parent.
    fn parent(&self, path: &Path) -> Option<PathBuf>;

    /// Get the file name component.
    ///
    /// Returns `None` if the path ends in `..` or is empty.
    fn file_name<'a>(&self, path: &'a Path) -> Option<&'a OsStr>;

    /// Get the file extension.
    ///
    /// Returns `None` if there is no extension.
    fn extension<'a>(&self, path: &'a Path) -> Option<&'a OsStr>;

    /// Get the file stem (name without extension).
    ///
    /// Returns `None` if there is no file name.
    fn stem<'a>(&self, path: &'a Path) -> Option<&'a OsStr>;

    /// Convert to string (lossy).
    ///
    /// Non-UTF-8 sequences are replaced with the replacement character.
    fn to_string_lossy(&self, path: &Path) -> String {
        path.to_string_lossy().into_owned()
    }

    /// Check if a path starts with another path.
    fn starts_with(&self, path: &Path, prefix: &Path) -> bool {
        path.starts_with(prefix)
    }

    /// Check if a path ends with another path.
    fn ends_with(&self, path: &Path, suffix: &Path) -> bool {
        path.ends_with(suffix)
    }

    /// Get the components iterator for a path.
    fn components<'a>(&self, path: &'a Path) -> std::path::Components<'a> {
        path.components()
    }

    /// Strip a prefix from a path.
    fn strip_prefix(&self, path: &Path, prefix: &Path) -> Option<PathBuf> {
        path.strip_prefix(prefix).ok().map(Path::to_path_buf)
    }

    /// Make a path relative to a base.
    ///
    /// Returns `None` if `path` is not under `base`.
    fn relative_to(&self, path: &Path, base: &Path) -> Option<PathBuf> {
        self.strip_prefix(path, base)
    }
}

/// Standard path normalizer using `std::path`.
///
/// This is the default implementation that works with local filesystem paths.
#[derive(Debug, Default, Clone, Copy)]
pub struct StandardPathNormalizer;

impl PathNormalizer for StandardPathNormalizer {
    fn normalize(&self, path: &Path) -> PathBuf {
        let mut result = PathBuf::new();
        for component in path.components() {
            use std::path::Component;
            match component {
                Component::CurDir => {} // Skip `.`
                Component::ParentDir => {
                    // Check if last component is a regular path segment (not "..")
                    let can_pop = result.file_name().is_some_and(|name| name != "..");
                    if can_pop {
                        result.pop();
                    } else {
                        // Either empty, root, or last component is ".."
                        // Preserve the ".." for relative paths
                        result.push(component);
                    }
                }
                c => result.push(c),
            }
        }
        if result.as_os_str().is_empty() {
            result.push(".");
        }
        result
    }

    fn canonicalize(&self, path: &Path) -> Result<PathBuf, VfsError> {
        std::fs::canonicalize(path).map_err(|e| match e.kind() {
            std::io::ErrorKind::NotFound => VfsError::NotFound(path.to_path_buf()),
            _ => VfsError::Io(e),
        })
    }

    fn is_absolute(&self, path: &Path) -> bool {
        path.is_absolute()
    }

    fn join(&self, base: &Path, relative: &Path) -> PathBuf {
        base.join(relative)
    }

    fn parent(&self, path: &Path) -> Option<PathBuf> {
        path.parent().map(Path::to_path_buf)
    }

    fn file_name<'a>(&self, path: &'a Path) -> Option<&'a OsStr> {
        path.file_name()
    }

    fn extension<'a>(&self, path: &'a Path) -> Option<&'a OsStr> {
        path.extension()
    }

    fn stem<'a>(&self, path: &'a Path) -> Option<&'a OsStr> {
        path.file_stem()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_normalize_current_dir() {
        let normalizer = StandardPathNormalizer;
        assert_eq!(normalizer.normalize(Path::new("./foo/bar")), PathBuf::from("foo/bar"));
        assert_eq!(normalizer.normalize(Path::new("./foo/./bar")), PathBuf::from("foo/bar"));
    }

    #[test]
    fn test_normalize_parent_dir() {
        let normalizer = StandardPathNormalizer;
        assert_eq!(normalizer.normalize(Path::new("foo/../bar")), PathBuf::from("bar"));
        assert_eq!(normalizer.normalize(Path::new("foo/baz/../bar")), PathBuf::from("foo/bar"));
    }

    #[test]
    fn test_normalize_absolute() {
        let normalizer = StandardPathNormalizer;
        assert_eq!(normalizer.normalize(Path::new("/a/b/../c/./d")), PathBuf::from("/a/c/d"));
    }

    #[test]
    fn test_normalize_empty() {
        let normalizer = StandardPathNormalizer;
        assert_eq!(normalizer.normalize(Path::new("")), PathBuf::from("."));
        assert_eq!(normalizer.normalize(Path::new(".")), PathBuf::from("."));
    }

    #[test]
    fn test_normalize_relative_parent() {
        let normalizer = StandardPathNormalizer;
        // Going up from empty should preserve ..
        assert_eq!(normalizer.normalize(Path::new("../foo")), PathBuf::from("../foo"));
        assert_eq!(normalizer.normalize(Path::new("../../foo")), PathBuf::from("../../foo"));
    }

    #[test]
    fn test_is_absolute() {
        let normalizer = StandardPathNormalizer;
        assert!(normalizer.is_absolute(Path::new("/absolute/path")));
        assert!(!normalizer.is_absolute(Path::new("relative/path")));
        assert!(!normalizer.is_absolute(Path::new("./relative")));
    }

    #[test]
    fn test_join() {
        let normalizer = StandardPathNormalizer;
        assert_eq!(
            normalizer.join(Path::new("/base"), Path::new("rel")),
            PathBuf::from("/base/rel")
        );
        // Absolute relative replaces base
        assert_eq!(normalizer.join(Path::new("/base"), Path::new("/abs")), PathBuf::from("/abs"));
    }

    #[test]
    fn test_parent() {
        let normalizer = StandardPathNormalizer;
        assert_eq!(normalizer.parent(Path::new("/foo/bar")), Some(PathBuf::from("/foo")));
        assert_eq!(normalizer.parent(Path::new("/foo")), Some(PathBuf::from("/")));
        // Root path has no parent (terminates in root)
        assert_eq!(normalizer.parent(Path::new("/")), None);
    }

    #[test]
    fn test_file_name() {
        let normalizer = StandardPathNormalizer;
        assert_eq!(normalizer.file_name(Path::new("/foo/bar.txt")), Some(OsStr::new("bar.txt")));
        assert_eq!(normalizer.file_name(Path::new("/foo/")), Some(OsStr::new("foo")));
    }

    #[test]
    fn test_extension() {
        let normalizer = StandardPathNormalizer;
        assert_eq!(normalizer.extension(Path::new("/foo/bar.txt")), Some(OsStr::new("txt")));
        assert_eq!(normalizer.extension(Path::new("/foo/bar.tar.gz")), Some(OsStr::new("gz")));
        assert_eq!(normalizer.extension(Path::new("/foo/bar")), None);
    }

    #[test]
    fn test_stem() {
        let normalizer = StandardPathNormalizer;
        assert_eq!(normalizer.stem(Path::new("/foo/bar.txt")), Some(OsStr::new("bar")));
        assert_eq!(normalizer.stem(Path::new("/foo/bar.tar.gz")), Some(OsStr::new("bar.tar")));
    }

    #[test]
    fn test_to_string_lossy() {
        let normalizer = StandardPathNormalizer;
        assert_eq!(normalizer.to_string_lossy(Path::new("/foo/bar")), "/foo/bar");
    }

    #[test]
    fn test_starts_with() {
        let normalizer = StandardPathNormalizer;
        assert!(normalizer.starts_with(Path::new("/foo/bar"), Path::new("/foo")));
        assert!(!normalizer.starts_with(Path::new("/foo/bar"), Path::new("/bar")));
    }

    #[test]
    fn test_ends_with() {
        let normalizer = StandardPathNormalizer;
        assert!(normalizer.ends_with(Path::new("/foo/bar"), Path::new("bar")));
        assert!(!normalizer.ends_with(Path::new("/foo/bar"), Path::new("foo")));
    }

    #[test]
    fn test_strip_prefix() {
        let normalizer = StandardPathNormalizer;
        assert_eq!(
            normalizer.strip_prefix(Path::new("/foo/bar/baz"), Path::new("/foo")),
            Some(PathBuf::from("bar/baz"))
        );
        assert_eq!(normalizer.strip_prefix(Path::new("/foo/bar"), Path::new("/other")), None);
    }

    #[test]
    fn test_relative_to() {
        let normalizer = StandardPathNormalizer;
        assert_eq!(
            normalizer.relative_to(Path::new("/foo/bar/baz"), Path::new("/foo")),
            Some(PathBuf::from("bar/baz"))
        );
        assert_eq!(normalizer.relative_to(Path::new("/foo/bar"), Path::new("/other")), None);
    }
}
