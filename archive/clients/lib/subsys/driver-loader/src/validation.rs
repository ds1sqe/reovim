//! Pre-dispatch vtable validation.
//!
//! Runs pure memory reads over the first three fields of every driver
//! vtable (`abi_version: u32`, `api_version: Version`, `size_of_self:
//! usize`). No function pointer is dereferenced until these checks pass.

use {
    crate::error::ValidationError, reovim_client_subsys_debug::abi::ClientDebugVTable,
    reovim_client_subsys_render::abi::ClientRenderVTable, reovim_kernel::api::v1::Version,
};

/// Host-side expectations for a loaded client-render driver vtable.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ClientRenderExpectations {
    pub abi_version: u32,
    pub api_version: Version,
    pub size_of_self: usize,
}

impl ClientRenderExpectations {
    /// Expectations read from the current host build.
    pub(crate) const fn from_host() -> Self {
        Self {
            abi_version: reovim_client_subsys_render::abi::REOVIM_CLIENT_RENDER_DRIVER_ABI_VERSION,
            api_version: reovim_client_subsys_render::abi::REOVIM_CLIENT_RENDER_DRIVER_API_VERSION,
            size_of_self: std::mem::size_of::<ClientRenderVTable>(),
        }
    }
}

/// Validate the client-render driver vtable header before dispatch.
///
/// # Errors
///
/// Returns [`ValidationError`] if the pointer is null, the ABI epoch
/// disagrees, the API semver major/minor is incompatible, or the
/// driver's `size_of_self` does not match the host's.
///
/// # Safety
///
/// `vtable` must either be null or point to a `ClientRenderVTable`
/// whose `abi_version`, `api_version`, and `size_of_self` fields are
/// fully initialized and valid to read. Typically the pointer comes
/// from a `libloading::Symbol<*const ClientRenderVTable>` resolved from
/// a freshly opened cdylib; in that case the fields are guaranteed to
/// be initialized by the cdylib's static initializer.
#[allow(clippy::missing_const_for_fn)]
pub(crate) unsafe fn check_client_render(
    vtable: *const ClientRenderVTable,
    expected: ClientRenderExpectations,
) -> Result<(), ValidationError> {
    if vtable.is_null() {
        return Err(ValidationError::VtablePointerNull);
    }
    // SAFETY: caller contract guarantees the pointer is non-null and
    // that the header fields are initialized.
    let vt = unsafe { &*vtable };

    if vt.abi_version != expected.abi_version {
        return Err(ValidationError::AbiVersionMismatch {
            found: vt.abi_version,
            expected: expected.abi_version,
        });
    }
    if vt.api_version.major != expected.api_version.major
        || vt.api_version.minor < expected.api_version.minor
    {
        return Err(ValidationError::ApiVersionIncompatible {
            found_major: vt.api_version.major,
            found_minor: vt.api_version.minor,
            found_patch: vt.api_version.patch,
            expected_major: expected.api_version.major,
            expected_minor: expected.api_version.minor,
        });
    }
    if vt.size_of_self != expected.size_of_self {
        return Err(ValidationError::SizeOfSelfMismatch {
            found: vt.size_of_self,
            expected: expected.size_of_self,
        });
    }
    Ok(())
}

/// Host-side expectations for a loaded client-debug driver vtable.
#[derive(Debug, Clone, Copy)]
pub(crate) struct ClientDebugExpectations {
    pub abi_version: u32,
    pub api_version: Version,
    pub size_of_self: usize,
}

impl ClientDebugExpectations {
    /// Expectations read from the current host build.
    pub(crate) const fn from_host() -> Self {
        Self {
            abi_version: reovim_client_subsys_debug::abi::REOVIM_CLIENT_DEBUG_DRIVER_ABI_VERSION,
            api_version: reovim_client_subsys_debug::abi::REOVIM_CLIENT_DEBUG_DRIVER_API_VERSION,
            size_of_self: std::mem::size_of::<ClientDebugVTable>(),
        }
    }
}

/// Validate the client-debug driver vtable header before dispatch.
///
/// # Errors
///
/// Returns [`ValidationError`] if the pointer is null, the ABI epoch
/// disagrees, the API semver major/minor is incompatible, or the
/// driver's `size_of_self` does not match the host's.
///
/// # Safety
///
/// `vtable` must either be null or point to a `ClientDebugVTable`
/// whose `abi_version`, `api_version`, and `size_of_self` fields are
/// fully initialized and valid to read.
#[allow(clippy::missing_const_for_fn)]
pub(crate) unsafe fn check_client_debug(
    vtable: *const ClientDebugVTable,
    expected: ClientDebugExpectations,
) -> Result<(), ValidationError> {
    if vtable.is_null() {
        return Err(ValidationError::VtablePointerNull);
    }
    // SAFETY: caller contract guarantees the pointer is non-null and
    // that the header fields are initialized.
    let vt = unsafe { &*vtable };

    if vt.abi_version != expected.abi_version {
        return Err(ValidationError::AbiVersionMismatch {
            found: vt.abi_version,
            expected: expected.abi_version,
        });
    }
    if vt.api_version.major != expected.api_version.major
        || vt.api_version.minor < expected.api_version.minor
    {
        return Err(ValidationError::ApiVersionIncompatible {
            found_major: vt.api_version.major,
            found_minor: vt.api_version.minor,
            found_patch: vt.api_version.patch,
            expected_major: expected.api_version.major,
            expected_minor: expected.api_version.minor,
        });
    }
    if vt.size_of_self != expected.size_of_self {
        return Err(ValidationError::SizeOfSelfMismatch {
            found: vt.size_of_self,
            expected: expected.size_of_self,
        });
    }
    Ok(())
}

#[cfg(test)]
#[path = "validation_tests.rs"]
mod tests;
