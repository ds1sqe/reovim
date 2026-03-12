//! ABI versioning for FFI module compatibility.
//!
//! The ABI version is separate from the API version:
//!
//! - **ABI version (1.0.0)**: Binary layout compatibility
//!   - Struct sizes and field layouts
//!   - Calling conventions
//!   - Symbol names
//!
//! - **API version (0.2.0)**: Semantic compatibility
//!   - Trait methods and behavior
//!   - Module lifecycle semantics
//!
//! # Version Compatibility
//!
//! ABI compatibility uses a subset of semver:
//!
//! - **Major version mismatch**: Always incompatible
//! - **Minor version**: Module's required minor <= kernel's provided minor
//! - **Patch version**: Ignored for compatibility
//!
//! # Example
//!
//! ```c
//! #include <reovim.h>
//!
//! bool check_kernel_compatibility(void) {
//!     ReovimVersion required = {1, 0, 0};
//!     ReovimVersion provided = reovim_abi_version();
//!     return reovim_abi_is_compatible(required, provided);
//! }
//! ```

use reovim_kernel::api::v1::Version;

/// Current ABI version.
///
/// This version is bumped when binary compatibility is broken:
/// - Struct layout changes
/// - Function signature changes
/// - Symbol name changes
///
/// # History
///
/// - 1.0.0: Initial stable ABI with `ModuleProbe` at 1308 bytes
/// - 1.1.0: Buffer, window, mode, command, and event APIs
/// - 1.2.0: Clipboard, register, and undo APIs
pub const ABI_VERSION: Version = Version::new(1, 2, 0);

/// ABI version major component.
///
/// Exported for C header inclusion.
#[unsafe(no_mangle)]
pub static ABI_VERSION_MAJOR: u32 = 1;

/// ABI version minor component.
///
/// Exported for C header inclusion.
#[unsafe(no_mangle)]
pub static ABI_VERSION_MINOR: u32 = 2;

/// ABI version patch component.
///
/// Exported for C header inclusion.
#[unsafe(no_mangle)]
pub static ABI_VERSION_PATCH: u32 = 0;

/// Get the current ABI version.
///
/// External modules can call this to determine the kernel's ABI version
/// before attempting to use any FFI functions.
#[unsafe(no_mangle)]
pub const extern "C" fn reovim_abi_version() -> Version {
    ABI_VERSION
}

/// Check if two versions are ABI-compatible.
///
/// # Compatibility Rules
///
/// - Major version must match exactly
/// - Required minor must be <= provided minor
/// - Patch version is ignored
///
/// # Parameters
///
/// - `required`: The version the module requires
/// - `provided`: The version the kernel provides
///
/// # Returns
///
/// `true` if the versions are compatible, `false` otherwise.
#[unsafe(no_mangle)]
pub const extern "C" fn reovim_abi_is_compatible(required: Version, provided: Version) -> bool {
    // Major must match
    if required.major != provided.major {
        return false;
    }

    // Required minor must be <= provided minor
    required.minor <= provided.minor
}

#[cfg(test)]
#[path = "version_tests.rs"]
mod tests;
