//! Memory-management bridge.
//!
//! Upper/pure crates receive `uapi/mm` control tables from composition roots.
//! This bridge backs the allocation table with the lower platform allocator
//! slots without exposing `kabi/platform` upward.

use {
    core::{alloc::Layout, ptr::NonNull},
    reovim_kabi_platform::handle,
    reovim_uapi_mm::{AllocControl, AllocError},
};

/// Returns the up-face allocation control table backed by this bridge.
///
/// ```rust,no_run
/// use reovim_system_kernel::mm::alloc_control;
///
/// let _control = alloc_control();
/// ```
#[must_use]
pub const fn alloc_control() -> AllocControl {
    AllocControl::new(alloc, dealloc)
}

/// Installs this bridge's allocation control table into `lib/ds`.
///
/// # Errors
///
/// Returns [`reovim_lib_ds::alloc_backend::AllocBackendInstallError`] when the
/// process already installed an allocation backend.
pub fn install_lib_ds_alloc_backend()
-> Result<(), reovim_lib_ds::alloc_backend::AllocBackendInstallError> {
    reovim_lib_ds::alloc_backend::install(alloc_control())
}

fn alloc(layout: Layout) -> Result<NonNull<u8>, AllocError> {
    handle().alloc(layout).map_err(|_| AllocError)
}

fn dealloc(ptr: NonNull<u8>, layout: Layout) {
    handle().dealloc(ptr, layout);
}
