//! Version management and compatibility checking.
//!
//! Provides API version constants and compatibility utilities for ensuring
//! module-kernel version compatibility.

use std::fmt;

// ============================================================================
// Version Type
// ============================================================================

/// Semantic version representation.
///
/// Provides type-safe version handling with named fields instead of tuples.
///
/// # FFI Safety
///
/// This type is `#[repr(C)]` for predictable memory layout across dynamic
/// library boundaries. This ensures the struct can be safely passed to/from
/// dynamically loaded modules.
///
/// # Ordering
///
/// Versions are ordered lexicographically by (major, minor, patch):
/// - `1.0.0 < 1.0.1 < 1.1.0 < 2.0.0`
///
/// This allows version comparison and sorting:
///
/// ```
/// use reovim_kernel::api::v1::Version;
///
/// assert!(Version::new(1, 0, 0) < Version::new(1, 0, 1));
/// assert!(Version::new(1, 2, 0) < Version::new(2, 0, 0));
///
/// let mut versions = vec![
///     Version::new(2, 0, 0),
///     Version::new(1, 0, 1),
///     Version::new(1, 0, 0),
/// ];
/// versions.sort();
/// assert_eq!(versions[0], Version::new(1, 0, 0));
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Version {
    /// Major version - breaking changes increment this.
    pub major: u32,
    /// Minor version - backwards-compatible additions increment this.
    pub minor: u32,
    /// Patch version - backwards-compatible fixes increment this.
    pub patch: u32,
}

impl Version {
    /// Create a new version.
    #[inline]
    #[must_use]
    pub const fn new(major: u32, minor: u32, patch: u32) -> Self {
        Self {
            major,
            minor,
            patch,
        }
    }
}

impl fmt::Display for Version {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}.{}.{}", self.major, self.minor, self.patch)
    }
}

// ============================================================================
// Version Constants
// ============================================================================

/// Current API version.
///
/// This is the first stable release of the kernel API.
pub const API_VERSION: Version = Version::new(1, 0, 0);

/// Current API version as a string.
pub const API_VERSION_STR: &str = "1.0.0";

// ============================================================================
// Error Types
// ============================================================================

/// Error returned when version compatibility check fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct VersionError {
    /// The kind of version error.
    pub kind: VersionErrorKind,
    /// The version that was required.
    pub required: Version,
    /// The version that was provided/available.
    pub provided: Version,
}

/// Kinds of version compatibility errors.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionErrorKind {
    /// Major version mismatch (breaking change).
    MajorMismatch,
    /// Required minor version is newer than provided.
    MinorTooNew,
}

impl fmt::Display for VersionError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            VersionErrorKind::MajorMismatch => {
                write!(
                    f,
                    "API major version mismatch: required {}.x.x, provided {}.x.x",
                    self.required.major, self.provided.major
                )
            }
            VersionErrorKind::MinorTooNew => {
                write!(
                    f,
                    "API minor version too new: required {}.{}.x, provided {}.{}.x",
                    self.required.major,
                    self.required.minor,
                    self.provided.major,
                    self.provided.minor
                )
            }
        }
    }
}

impl std::error::Error for VersionError {}

// ============================================================================
// Compatibility Functions
// ============================================================================

/// Check if the current API version is compatible with the required version.
///
/// Returns `Ok(())` if compatible, `Err(VersionError)` otherwise.
///
/// # Compatibility Rules (semver)
///
/// - Major version must match exactly
/// - Module can require an older minor version than kernel provides
/// - Patch version is ignored for compatibility
///
/// # Errors
///
/// Returns `Err(VersionError)` if:
/// - Major version mismatch (`VersionErrorKind::MajorMismatch`)
/// - Required minor version is newer than provided (`VersionErrorKind::MinorTooNew`)
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::{check_api_version, Version};
///
/// // Check if kernel provides compatible API
/// let result = check_api_version(Version::new(1, 0, 0));
/// assert!(result.is_ok());
/// ```
pub const fn check_api_version(required: Version) -> Result<(), VersionError> {
    if is_compatible(required, API_VERSION) {
        Ok(())
    } else if required.major != API_VERSION.major {
        Err(VersionError {
            kind: VersionErrorKind::MajorMismatch,
            required,
            provided: API_VERSION,
        })
    } else {
        Err(VersionError {
            kind: VersionErrorKind::MinorTooNew,
            required,
            provided: API_VERSION,
        })
    }
}

/// Check if `required` version is compatible with `provided` version.
///
/// # Compatibility Rules
///
/// - Major version must match exactly
/// - Required minor must be <= provided minor (backwards compatible)
/// - Patch version is ignored
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::{is_compatible, Version};
///
/// // Same version is compatible
/// assert!(is_compatible(Version::new(1, 0, 0), Version::new(1, 0, 0)));
///
/// // Older required minor is compatible
/// assert!(is_compatible(Version::new(1, 0, 0), Version::new(1, 1, 0)));
///
/// // Newer required minor is NOT compatible
/// assert!(!is_compatible(Version::new(1, 2, 0), Version::new(1, 1, 0)));
///
/// // Different major is NOT compatible
/// assert!(!is_compatible(Version::new(2, 0, 0), Version::new(1, 0, 0)));
/// ```
#[must_use]
pub const fn is_compatible(required: Version, provided: Version) -> bool {
    // Major must match exactly
    if required.major != provided.major {
        return false;
    }

    // Required minor must be <= provided minor
    required.minor <= provided.minor
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_version_new() {
        let v = Version::new(1, 2, 3);
        assert_eq!(v.major, 1);
        assert_eq!(v.minor, 2);
        assert_eq!(v.patch, 3);
    }

    #[test]
    fn test_version_display() {
        let v = Version::new(1, 2, 3);
        assert_eq!(v.to_string(), "1.2.3");
    }

    #[test]
    fn test_api_version_constants() {
        assert_eq!(API_VERSION, Version::new(1, 0, 0));
        assert_eq!(API_VERSION_STR, "1.0.0");
    }

    #[test]
    fn test_is_compatible_same_version() {
        assert!(is_compatible(Version::new(1, 0, 0), Version::new(1, 0, 0)));
    }

    #[test]
    fn test_is_compatible_older_minor() {
        assert!(is_compatible(Version::new(1, 0, 0), Version::new(1, 1, 0)));
        assert!(is_compatible(Version::new(1, 1, 0), Version::new(1, 5, 0)));
    }

    #[test]
    fn test_is_compatible_newer_minor_not_compatible() {
        assert!(!is_compatible(Version::new(1, 2, 0), Version::new(1, 1, 0)));
    }

    #[test]
    fn test_is_compatible_different_major_not_compatible() {
        assert!(!is_compatible(Version::new(2, 0, 0), Version::new(1, 0, 0)));
        assert!(!is_compatible(Version::new(1, 0, 0), Version::new(2, 0, 0)));
    }

    #[test]
    fn test_is_compatible_patch_ignored() {
        assert!(is_compatible(Version::new(1, 0, 0), Version::new(1, 0, 5)));
        assert!(is_compatible(Version::new(1, 0, 10), Version::new(1, 0, 0)));
    }

    #[test]
    fn test_check_api_version_compatible() {
        assert!(check_api_version(Version::new(1, 0, 0)).is_ok());
    }

    #[test]
    fn test_check_api_version_major_mismatch() {
        let result = check_api_version(Version::new(2, 0, 0));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind, VersionErrorKind::MajorMismatch);
    }

    #[test]
    fn test_check_api_version_minor_too_new() {
        let result = check_api_version(Version::new(1, 5, 0));
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert_eq!(err.kind, VersionErrorKind::MinorTooNew);
    }

    #[test]
    fn test_version_error_display() {
        let err = VersionError {
            kind: VersionErrorKind::MajorMismatch,
            required: Version::new(2, 0, 0),
            provided: Version::new(1, 0, 0),
        };
        assert!(err.to_string().contains("major version mismatch"));

        let err = VersionError {
            kind: VersionErrorKind::MinorTooNew,
            required: Version::new(1, 5, 0),
            provided: Version::new(1, 0, 0),
        };
        assert!(err.to_string().contains("minor version too new"));
    }

    #[test]
    fn test_version_ordering() {
        // Patch ordering
        assert!(Version::new(1, 0, 0) < Version::new(1, 0, 1));
        assert!(Version::new(1, 0, 1) < Version::new(1, 0, 2));

        // Minor ordering
        assert!(Version::new(1, 0, 9) < Version::new(1, 1, 0));
        assert!(Version::new(1, 1, 0) < Version::new(1, 2, 0));

        // Major ordering
        assert!(Version::new(1, 9, 9) < Version::new(2, 0, 0));
        assert!(Version::new(2, 0, 0) < Version::new(3, 0, 0));

        // Equal versions
        assert!(Version::new(1, 2, 3) == Version::new(1, 2, 3));
        assert!(Version::new(1, 2, 3) <= Version::new(1, 2, 3));
        assert!(Version::new(1, 2, 3) >= Version::new(1, 2, 3));
    }

    #[test]
    fn test_version_sorting() {
        let mut versions = [
            Version::new(2, 0, 0),
            Version::new(1, 0, 1),
            Version::new(1, 0, 0),
            Version::new(1, 1, 0),
            Version::new(0, 1, 0),
        ];
        versions.sort();

        assert_eq!(versions[0], Version::new(0, 1, 0));
        assert_eq!(versions[1], Version::new(1, 0, 0));
        assert_eq!(versions[2], Version::new(1, 0, 1));
        assert_eq!(versions[3], Version::new(1, 1, 0));
        assert_eq!(versions[4], Version::new(2, 0, 0));
    }

    #[test]
    fn test_version_min_max() {
        let v1 = Version::new(1, 0, 0);
        let v2 = Version::new(2, 0, 0);

        assert_eq!(v1.min(v2), v1);
        assert_eq!(v1.max(v2), v2);
    }

    #[test]
    fn test_version_is_repr_c() {
        // Verify Version has predictable FFI-safe layout
        // 3 * u32 = 12 bytes, aligned to 4 bytes
        assert_eq!(std::mem::size_of::<Version>(), 12);
        assert_eq!(std::mem::align_of::<Version>(), 4);
    }
}
