//! FFI type re-exports and documentation.
//!
//! This module re-exports `#[repr(C)]` types from the kernel for use by external
//! modules. These types have stable ABI guarantees.
//!
//! # Memory Layout Guarantees
//!
//! All types in this module are `#[repr(C)]` with documented sizes:
//!
//! | Type | Size (bytes) | Alignment |
//! |------|--------------|-----------|
//! | `Version` | 12 | 4 |
//! | `ModuleProbe` | 1308 | 4 |
//!
//! # Stability
//!
//! Breaking changes to these layouts will bump the ABI version.

use reovim_kernel::api::v1::{ModuleProbe, Version};

/// Verify that `Version` has expected FFI-safe layout.
///
/// This is a compile-time assertion that will fail if the layout changes.
const _: () = {
    assert!(std::mem::size_of::<Version>() == 12);
    assert!(std::mem::align_of::<Version>() == 4);
};

/// Verify that `ModuleProbe` has expected FFI-safe layout.
const _: () = {
    assert!(std::mem::size_of::<ModuleProbe>() == 1308);
    assert!(std::mem::align_of::<ModuleProbe>() == 4);
};

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // Size and alignment tests
    // ========================================================================

    #[test]
    fn test_module_probe_size() {
        assert_eq!(std::mem::size_of::<ModuleProbe>(), 1308);
        assert_eq!(std::mem::align_of::<ModuleProbe>(), 4);
    }

    #[test]
    fn test_version_repr_c_layout() {
        let v = Version::new(1, 2, 3);

        // Verify field ordering: major, minor, patch (each u32)
        let ptr = (&raw const v).cast::<u32>();
        unsafe {
            assert_eq!(*ptr, 1); // major
            assert_eq!(*ptr.add(1), 2); // minor
            assert_eq!(*ptr.add(2), 3); // patch
        }
    }

    #[test]
    fn test_module_probe_id_offset() {
        let probe =
            ModuleProbe::new("test-id", "Test Name", Version::new(1, 0, 0), Version::new(0, 2, 0));

        // ID field is at offset 0, 64 bytes
        let ptr = (&raw const probe).cast::<u8>();
        let id_bytes = unsafe { std::slice::from_raw_parts(ptr, 7) };
        assert_eq!(id_bytes, b"test-id");
    }

    // ========================================================================
    // String boundary tests (ModuleProbe buffer limits)
    // ========================================================================

    #[test]
    fn test_module_probe_id_max_length() {
        // ID buffer is 64 bytes, max string is 63 chars + null
        let max_id = "a".repeat(63);
        let probe = ModuleProbe::new(&max_id, "Test", Version::new(1, 0, 0), Version::new(0, 2, 0));

        assert_eq!(probe.id_str(), max_id);
        assert_eq!(probe.id_str().len(), 63);
    }

    #[test]
    fn test_module_probe_id_truncation() {
        // ID longer than 63 chars should be truncated
        let long_id = "b".repeat(100);
        let probe =
            ModuleProbe::new(&long_id, "Test", Version::new(1, 0, 0), Version::new(0, 2, 0));

        // Should be truncated to 63 chars
        assert_eq!(probe.id_str().len(), 63);
        assert_eq!(probe.id_str(), "b".repeat(63));
    }

    #[test]
    fn test_module_probe_name_max_length() {
        // Name buffer is 128 bytes, max string is 127 chars + null
        let max_name = "c".repeat(127);
        let probe =
            ModuleProbe::new("test", &max_name, Version::new(1, 0, 0), Version::new(0, 2, 0));

        assert_eq!(probe.name_str(), max_name);
        assert_eq!(probe.name_str().len(), 127);
    }

    #[test]
    fn test_module_probe_name_truncation() {
        // Name longer than 127 chars should be truncated
        let long_name = "d".repeat(200);
        let probe =
            ModuleProbe::new("test", &long_name, Version::new(1, 0, 0), Version::new(0, 2, 0));

        // Should be truncated to 127 chars
        assert_eq!(probe.name_str().len(), 127);
        assert_eq!(probe.name_str(), "d".repeat(127));
    }

    #[test]
    fn test_module_probe_rustc_version_max_length() {
        // rustc_version buffer is 64 bytes, max string is 63 chars + null
        let max_version = "e".repeat(63);
        let probe = ModuleProbe::new("test", "Test", Version::new(1, 0, 0), Version::new(0, 2, 0))
            .with_rustc_version(&max_version);

        assert_eq!(probe.rustc_version_str(), max_version);
        assert_eq!(probe.rustc_version_str().len(), 63);
    }

    #[test]
    fn test_module_probe_rustc_version_truncation() {
        // rustc_version longer than 63 chars should be truncated
        let long_version = "f".repeat(100);
        let probe = ModuleProbe::new("test", "Test", Version::new(1, 0, 0), Version::new(0, 2, 0))
            .with_rustc_version(&long_version);

        assert_eq!(probe.rustc_version_str().len(), 63);
    }

    #[test]
    fn test_module_probe_dependency_max_length() {
        // Dependency ID buffer is 64 bytes each, max string is 63 chars + null
        let max_dep = "g".repeat(63);
        let probe = ModuleProbe::new("test", "Test", Version::new(1, 0, 0), Version::new(0, 2, 0))
            .with_required_dep(0, &max_dep);

        let deps = probe.required_deps();
        assert_eq!(deps.len(), 1);
        assert_eq!(deps[0].as_str(), max_dep);
    }

    #[test]
    fn test_module_probe_max_dependencies() {
        // Test all 8 required + 8 optional dependencies
        let mut probe =
            ModuleProbe::new("test", "Test", Version::new(1, 0, 0), Version::new(0, 2, 0));

        for i in 0..8 {
            probe = probe.with_required_dep(i, &format!("req-{i}"));
            probe = probe.with_optional_dep(i, &format!("opt-{i}"));
        }

        let req_deps = probe.required_deps();
        let opt_deps = probe.optional_deps();

        assert_eq!(req_deps.len(), 8);
        assert_eq!(opt_deps.len(), 8);
        assert_eq!(req_deps[7].as_str(), "req-7");
        assert_eq!(opt_deps[7].as_str(), "opt-7");
    }
}
