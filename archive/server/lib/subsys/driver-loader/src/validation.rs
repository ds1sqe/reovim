//! Pre-dispatch vtable validation.
//!
//! Runs pure memory reads over the first three fields of every server
//! driver vtable (`abi_version: u32`, `api_version: Version`,
//! `size_of_self: usize`). No function pointer is dereferenced until
//! these checks pass.
//!
//! Mirrors `clients/lib/subsys/driver-loader/src/validation.rs`. The
//! per-family `check_*` functions are tier-isolated: each reads only
//! the constants exported by its own subsys-net peer, so a future
//! driver family added to this crate brings its own `check_*` and its
//! own constants set rather than parameterizing this one.

use {
    crate::error::ValidationError, reovim_kernel::api::v1::Version,
    reovim_subsys_buffer::abi::BufferVTable, reovim_subsys_net::abi::NetGrpcVTable,
};

/// Host-side expectations for a loaded net-grpc driver vtable.
#[derive(Debug, Clone, Copy)]
pub struct NetGrpcExpectations {
    pub abi_version: u32,
    pub api_version: Version,
    pub size_of_self: usize,
}

impl NetGrpcExpectations {
    /// Expectations read from the current host build.
    pub const fn from_host() -> Self {
        Self {
            abi_version: reovim_subsys_net::abi::REOVIM_NET_GRPC_DRIVER_ABI_VERSION,
            api_version: reovim_subsys_net::abi::REOVIM_NET_GRPC_DRIVER_API_VERSION,
            size_of_self: std::mem::size_of::<NetGrpcVTable>(),
        }
    }
}

/// Validate the net-grpc driver vtable header before dispatch.
///
/// # Errors
///
/// Returns [`ValidationError`] if the pointer is null, the ABI epoch
/// disagrees, the API semver major/minor is incompatible, or the
/// driver's `size_of_self` does not match the host's.
///
/// # Safety
///
/// `vtable` must either be null or point to a [`NetGrpcVTable`]
/// whose `abi_version`, `api_version`, and `size_of_self` fields are
/// fully initialized and valid to read. Typically the pointer comes
/// from a `libloading::Symbol<*const NetGrpcVTable>` resolved from a
/// freshly opened cdylib; in that case the fields are guaranteed to
/// be initialized by the cdylib's static initializer.
#[allow(clippy::missing_const_for_fn)]
pub unsafe fn check_net_grpc(
    vtable: *const NetGrpcVTable,
    expected: NetGrpcExpectations,
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

/// Host-side expectations for a loaded buffer driver vtable.
#[derive(Debug, Clone, Copy)]
pub struct BufferExpectations {
    pub abi_version: u32,
    pub api_version: Version,
    pub size_of_self: usize,
}

impl BufferExpectations {
    /// Expectations read from the current host build.
    pub const fn from_host() -> Self {
        Self {
            abi_version: reovim_subsys_buffer::abi::REOVIM_BUFFER_DRIVER_ABI_VERSION,
            api_version: reovim_subsys_buffer::abi::REOVIM_BUFFER_DRIVER_API_VERSION,
            size_of_self: std::mem::size_of::<BufferVTable>(),
        }
    }
}

/// Validate the buffer driver vtable header before dispatch.
///
/// # Errors
///
/// Returns [`ValidationError`] if the pointer is null, the ABI epoch
/// disagrees, the API semver major/minor is incompatible, or the
/// driver's `size_of_self` does not match the host's.
///
/// # Safety
///
/// `vtable` must either be null or point to a [`BufferVTable`]
/// whose `abi_version`, `api_version`, and `size_of_self` fields are
/// fully initialized and valid to read. Typically the pointer comes
/// from a `libloading::Symbol<*const BufferVTable>` resolved from a
/// freshly opened cdylib; in that case the fields are guaranteed to
/// be initialized by the cdylib's static initializer.
#[allow(clippy::missing_const_for_fn)]
pub unsafe fn check_buffer(
    vtable: *const BufferVTable,
    expected: BufferExpectations,
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
