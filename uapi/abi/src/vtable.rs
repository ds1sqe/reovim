//! 6.3 §3, 6.2 §2 — Common vtable header and manifest kind enum.
//!
//! Every vtable exported by a cdylib begins with [`VtableHeader`] in this
//! exact field order.  The loader reads only the header before calling any
//! function pointer (6.2 §2.1 load rules).
use crate::ids::{AbiVersion, Version};

/// Common vtable header — present at the start of every vtable (6.3 §3,
/// 6.2 §2).
///
/// AB3 (reshaped): new vtable slots append after existing ones.  Loaders of
/// older kernels read only `size_of_self >= offset + size_of(slot)` bytes.
///
/// # Load rules (6.2 §2.1)
///
/// 1. Read `sizeof(VtableHeader)` bytes.
/// 2. `cdylib.abi.major != kernel.abi.major` → reject
///    [`ErrorCode::IncompatibleAbi`][crate::error::ErrorCode::IncompatibleAbi].
/// 3. `cdylib.abi.minor > kernel.abi.minor` → reject `IncompatibleAbi`.
/// 4. `cdylib.kind != expected` → reject
///    [`ErrorCode::IllegalKind`][crate::error::ErrorCode::IllegalKind].
/// 5. `size_of_self < kernel-known minimum` → reject
///    [`ErrorCode::ShortVtable`][crate::error::ErrorCode::ShortVtable].
/// 6. Read appended slots only when `size_of_self` covers them.
///
/// # Examples
///
/// ```rust
/// use reovim_uapi_abi::vtable::{VtableHeader, ManifestKind};
/// use reovim_uapi_abi::ids::{AbiVersion, Version};
/// use core::mem;
///
/// let hdr = VtableHeader {
///     abi: AbiVersion { major: 1, minor: 0, patch: 0, pad: 0 },
///     api: Version    { major: 0, minor: 1, patch: 0, pad: 0 },
///     size_of_self: mem::size_of::<VtableHeader>(),
///     kind: ManifestKind::ModuleServer,
///     flags: 0,
/// };
/// assert_eq!(hdr.kind, ManifestKind::ModuleServer);
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct VtableHeader {
    /// Binary epoch (6.2 §1).
    pub abi: AbiVersion,
    /// Semantic contract version within this ABI epoch (6.2 §1).
    pub api: Version,
    /// Size of this vtable instance in bytes; used by the loader to guard
    /// slot reads (AB3).
    pub size_of_self: usize,
    /// Manifest kind; must match the manifest's `kind` field (6.2 §7).
    pub kind: ManifestKind,
    /// Reserved unless explicitly assigned; set to zero (6.2 open item 1).
    pub flags: u32,
}

/// Discriminates the type of vtable a cdylib exports (6.3 §3, 6.2 §7).
///
/// `Unknown = 0` is reserved-invalid; the loader rejects it with
/// [`ErrorCode::IllegalKind`][crate::error::ErrorCode::IllegalKind].
/// This set is final for ABI major 1.
///
/// Sub-kinds (e.g. driver `render`/`input`) use manifest `[[vtable]]` kind
/// strings and vtable symbol names, not new enum variants.
///
/// # Examples
///
/// ```rust
/// use reovim_uapi_abi::vtable::ManifestKind;
///
/// assert_eq!(ManifestKind::Unknown as u8, 0);
/// assert_eq!(ManifestKind::ModuleServer as u8, 1);
/// assert_eq!(ManifestKind::StreamScheme as u8, 8);
/// ```
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ManifestKind {
    /// Reserved-invalid; rejected at load.
    Unknown = 0,
    /// Server-side policy module.
    ModuleServer = 1,
    /// Server-side driver (input, render, display, …).
    DriverServer = 2,
    /// Client-side policy module.
    ModuleClient = 3,
    /// Client-side driver.
    DriverClient = 4,
    /// Client-side capability implementation.
    CapabilityClient = 5,
    /// Server-side domain implementation.
    DomainServer = 6,
    /// Server-side provider.
    ProviderServer = 7,
    /// Stream substrate scheme (4.4).
    StreamScheme = 8,
}
