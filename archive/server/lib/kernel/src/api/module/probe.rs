//! FFI-safe module probe for metadata discovery.

use {super::ModuleId, crate::api::version::Version};

/// FFI-safe module probe for metadata discovery.
///
/// Linux equivalent: `struct modinfo` + `vermagic` string
///
/// This struct uses fixed-size arrays instead of pointers to avoid
/// lifetime issues when the module is unloaded. All strings are
/// null-terminated within their fixed buffers.
///
/// # FFI Safety
///
/// - `#[repr(C)]` ensures predictable memory layout across dynamic library boundaries
/// - Fixed-size arrays avoid pointer invalidation when module is unloaded
/// - All fields are Copy, no heap allocation required
/// - Can be returned by value across FFI boundary safely
///
/// # Buffer Sizes
///
/// - `id`: 64 bytes (63 chars + nul) - module identifier
/// - `name`: 128 bytes (127 chars + nul) - display name
///
/// Strings exceeding these limits are truncated (not an error).
///
/// # Example
///
/// ```
/// use reovim_kernel::api::v1::{ModuleProbe, Version};
///
/// let probe = ModuleProbe::new(
///     "lang-rust",
///     "Rust Language Support",
///     Version::new(1, 0, 0),
///     Version::new(1, 0, 0),
/// );
///
/// assert_eq!(probe.id_str(), "lang-rust");
/// assert_eq!(probe.name_str(), "Rust Language Support");
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ModuleProbe {
    /// Module ID (null-terminated, max 63 chars + nul)
    pub id: [u8; 64],
    /// Module name (null-terminated, max 127 chars + nul)
    pub name: [u8; 128],
    /// Module version
    pub version: Version,
    /// Required kernel API version
    pub api_version: Version,
    /// Rustc version used to compile the module (for ABI compatibility checks)
    pub rustc_version: [u8; 64],
    /// Number of required dependencies (max 8)
    pub required_deps_count: u8,
    /// Required dependency IDs (null-terminated strings)
    pub required_deps: [[u8; 64]; 8],
    /// Number of optional dependencies (max 8)
    pub optional_deps_count: u8,
    /// Optional dependency IDs (null-terminated strings)
    pub optional_deps: [[u8; 64]; 8],
}

impl ModuleProbe {
    /// Create a new probe with the given metadata.
    ///
    /// Strings are truncated if they exceed buffer size.
    /// This is a const fn for use in static initialization.
    ///
    /// # Example
    ///
    /// ```
    /// use reovim_kernel::api::v1::{ModuleProbe, Version};
    ///
    /// // Can be used in const context
    /// const PROBE: ModuleProbe = ModuleProbe::new(
    ///     "my-module",
    ///     "My Module",
    ///     Version::new(1, 0, 0),
    ///     Version::new(1, 0, 0),
    /// );
    /// ```
    #[must_use]
    pub const fn new(id: &str, name: &str, version: Version, api_version: Version) -> Self {
        let mut probe = Self {
            id: [0; 64],
            name: [0; 128],
            version,
            api_version,
            rustc_version: [0; 64],
            required_deps_count: 0,
            required_deps: [[0; 64]; 8],
            optional_deps_count: 0,
            optional_deps: [[0; 64]; 8],
        };

        // Copy id (const fn compatible - no iterator)
        let id_bytes = id.as_bytes();
        let id_len = if id_bytes.len() < 63 {
            id_bytes.len()
        } else {
            63
        };
        let mut i = 0;
        while i < id_len {
            probe.id[i] = id_bytes[i];
            i += 1;
        }

        // Copy name
        let name_bytes = name.as_bytes();
        let name_len = if name_bytes.len() < 127 {
            name_bytes.len()
        } else {
            127
        };
        i = 0;
        while i < name_len {
            probe.name[i] = name_bytes[i];
            i += 1;
        }

        probe
    }

    /// Get module ID as string slice.
    ///
    /// Returns the null-terminated string content from the fixed buffer.
    #[must_use]
    pub fn id_str(&self) -> &str {
        let len = self
            .id
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(self.id.len());
        // Safety: we only write valid UTF-8 in new()
        std::str::from_utf8(&self.id[..len]).unwrap_or("")
    }

    /// Get module name as string slice.
    ///
    /// Returns the null-terminated string content from the fixed buffer.
    #[must_use]
    pub fn name_str(&self) -> &str {
        let len = self
            .name
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(self.name.len());
        std::str::from_utf8(&self.name[..len]).unwrap_or("")
    }

    /// Get rustc version as string slice.
    ///
    /// Returns empty string if not set.
    #[must_use]
    pub fn rustc_version_str(&self) -> &str {
        let len = self
            .rustc_version
            .iter()
            .position(|&b| b == 0)
            .unwrap_or(self.rustc_version.len());
        std::str::from_utf8(&self.rustc_version[..len]).unwrap_or("")
    }

    /// Get required dependencies as `ModuleId` list.
    ///
    /// Returns up to 8 dependencies stored in the probe.
    #[must_use]
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn required_deps(&self) -> Vec<ModuleId> {
        let count = (self.required_deps_count as usize).min(8);
        (0..count)
            .filter_map(|i| {
                let len = self.required_deps[i]
                    .iter()
                    .position(|&b| b == 0)
                    .unwrap_or(64);
                if len == 0 {
                    None
                } else {
                    std::str::from_utf8(&self.required_deps[i][..len])
                        .ok()
                        .map(|s| ModuleId::from_string(s.to_string()))
                }
            })
            .collect()
    }

    /// Get optional dependencies as `ModuleId` list.
    ///
    /// Returns up to 8 dependencies stored in the probe.
    #[must_use]
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub fn optional_deps(&self) -> Vec<ModuleId> {
        let count = (self.optional_deps_count as usize).min(8);
        (0..count)
            .filter_map(|i| {
                let len = self.optional_deps[i]
                    .iter()
                    .position(|&b| b == 0)
                    .unwrap_or(64);
                if len == 0 {
                    None
                } else {
                    std::str::from_utf8(&self.optional_deps[i][..len])
                        .ok()
                        .map(|s| ModuleId::from_string(s.to_string()))
                }
            })
            .collect()
    }

    /// Set rustc version (builder pattern for const contexts).
    ///
    /// # Example
    ///
    /// ```
    /// use reovim_kernel::api::v1::{ModuleProbe, Version};
    ///
    /// let probe = ModuleProbe::new("test", "Test", Version::new(1, 0, 0), Version::new(0, 2, 0))
    ///     .with_rustc_version("1.92.0");
    /// assert_eq!(probe.rustc_version_str(), "1.92.0");
    /// ```
    #[must_use]
    pub const fn with_rustc_version(mut self, version: &str) -> Self {
        let bytes = version.as_bytes();
        let len = if bytes.len() < 63 { bytes.len() } else { 63 };
        let mut i = 0;
        while i < len {
            self.rustc_version[i] = bytes[i];
            i += 1;
        }
        self
    }

    /// Add a required dependency at the specified index (builder pattern).
    ///
    /// Index must be 0-7. Silently ignored if index >= 8.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)] // Safe: index < 8 is checked
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub const fn with_required_dep(mut self, index: usize, dep: &str) -> Self {
        if index >= 8 {
            return self;
        }
        let bytes = dep.as_bytes();
        let len = if bytes.len() < 63 { bytes.len() } else { 63 };
        let mut i = 0;
        while i < len {
            self.required_deps[index][i] = bytes[i];
            i += 1;
        }
        // Update count if this extends it (safe cast: index < 8)
        if index as u8 >= self.required_deps_count {
            self.required_deps_count = (index + 1) as u8;
        }
        self
    }

    /// Add an optional dependency at the specified index (builder pattern).
    ///
    /// Index must be 0-7. Silently ignored if index >= 8.
    #[must_use]
    #[allow(clippy::cast_possible_truncation)] // Safe: index < 8 is checked
    #[cfg_attr(coverage_nightly, coverage(off))]
    pub const fn with_optional_dep(mut self, index: usize, dep: &str) -> Self {
        if index >= 8 {
            return self;
        }
        let bytes = dep.as_bytes();
        let len = if bytes.len() < 63 { bytes.len() } else { 63 };
        let mut i = 0;
        while i < len {
            self.optional_deps[index][i] = bytes[i];
            i += 1;
        }
        // Safe cast: index < 8
        if index as u8 >= self.optional_deps_count {
            self.optional_deps_count = (index + 1) as u8;
        }
        self
    }
}
