//! 6.3 §6 — Service descriptor.
//!
//! Describes a registered service instance at the ABI boundary.
use core::ffi::c_void;

use crate::{
    ids::{AbiVersion, CdylibId, ServiceKey, Version},
    slices::StrSlice,
};

/// Flag: the service object is `Send`-safe (6.3 §6).
///
/// ```rust
/// use reovim_uapi_abi::service::{
///     SERVICE_SEND_SAFE, SERVICE_SYNC_SAFE,
///     SERVICE_HOSTAPI_REENTRANT, SERVICE_DROP_MAY_CALL_HOSTAPI,
/// };
///
/// assert_eq!(SERVICE_SEND_SAFE,            0x01);
/// assert_eq!(SERVICE_SYNC_SAFE,            0x02);
/// assert_eq!(SERVICE_HOSTAPI_REENTRANT,    0x04);
/// assert_eq!(SERVICE_DROP_MAY_CALL_HOSTAPI, 0x08);
/// ```
pub const SERVICE_SEND_SAFE: u32 = 0x01;
/// Flag: the service vtable pointer is safe to call from multiple threads.
/// See [`SERVICE_SEND_SAFE`] for the full flags example.
pub const SERVICE_SYNC_SAFE: u32 = 0x02;
/// Flag: the service may call back into `HostApi` in a re-entrant manner.
/// See [`SERVICE_SEND_SAFE`] for the full flags example.
pub const SERVICE_HOSTAPI_REENTRANT: u32 = 0x04;
/// Flag: `drop_fn` may invoke `HostApi` callbacks.
/// See [`SERVICE_SEND_SAFE`] for the full flags example.
pub const SERVICE_DROP_MAY_CALL_HOSTAPI: u32 = 0x08;

/// Describes a service registered at the ABI boundary (6.3 §6).
///
/// ```rust
/// use reovim_uapi_abi::service::ServiceDescriptor;
/// use reovim_uapi_abi::ids::{AbiVersion, CdylibId, ServiceKey, Version};
/// use reovim_uapi_abi::slices::StrSlice;
/// use core::num::NonZeroU32;
/// use core::ptr;
///
/// unsafe extern "C" fn drop_noop(_: *mut core::ffi::c_void) {}
///
/// let desc = ServiceDescriptor {
///     key:             ServiceKey(NonZeroU32::new(1).unwrap()),
///     owner_cdylib_id: CdylibId(NonZeroU32::new(1).unwrap()),
///     abi:             AbiVersion { major: 1, minor: 0, patch: 0, pad: 0 },
///     api:             Version    { major: 0, minor: 1, patch: 0, pad: 0 },
///     type_name:       StrSlice { data: ptr::null(), len: 0 },
///     vtable_ptr:      ptr::null(),
///     vtable_size:     0,
///     handle:          ptr::null_mut(),
///     flags:           0,
///     drop_fn:         drop_noop,
/// };
/// assert_eq!(desc.flags, 0);
/// ```
#[repr(C)]
pub struct ServiceDescriptor {
    /// Registration key (unique per session).
    pub key: ServiceKey,
    /// Cdylib that owns this service.
    pub owner_cdylib_id: CdylibId,
    /// ABI version the vtable was built against.
    pub abi: AbiVersion,
    /// Semantic API version.
    pub api: Version,
    /// Fully-qualified Rust type name (for diagnostics; UTF-8).
    pub type_name: StrSlice,
    /// Pointer to the vtable; the vtable's first field is a
    /// [`crate::vtable::VtableHeader`].
    pub vtable_ptr: *const c_void,
    /// Size of the vtable in bytes.
    pub vtable_size: usize,
    /// Opaque handle passed as the first argument to every slot.
    pub handle: *mut c_void,
    /// Bitfield; see `SERVICE_*` constants.
    pub flags: u32,
    /// Releases `handle`; called by the kernel on unregistration.
    pub drop_fn: unsafe extern "C" fn(*mut c_void),
}
