//! 6.3 §2.1 — Versions and identifiers.
//!
//! All types here have `#[repr(C)]` or `#[repr(transparent)]` layouts as
//! defined in the catalog.  Layouts are frozen (AB15).
use core::num::NonZeroU32;

/// Binary epoch version (6.2 §1, 6.3 §2.1).
///
/// The major field governs ABI compatibility: a major mismatch causes the
/// loader to reject the cdylib with [`crate::error::ErrorCode::IncompatibleAbi`].
/// Minor `cdylib > kernel` also rejects.  Patch is ignored at load.
///
/// ```rust
/// use reovim_uapi_abi::ids::AbiVersion;
///
/// let v = AbiVersion { major: 1, minor: 0, patch: 0, pad: 0 };
/// assert_eq!(v.major, 1);
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AbiVersion {
    /// ABI epoch; major mismatch → [`crate::error::ErrorCode::IncompatibleAbi`].
    pub major: u16,
    /// ABI minor; `cdylib.minor > kernel.minor` → reject.
    pub minor: u16,
    /// Patch level; ignored at load.
    pub patch: u16,
    /// Padding to align to 8 bytes.
    pub pad: u16,
}

/// Semantic-contract version within a fixed ABI epoch (6.2 §1, 6.3 §2.1).
///
/// A driver-render API can grow new operations across api-minor without
/// breaking the ABI shell.
///
/// ```rust
/// use reovim_uapi_abi::ids::Version;
///
/// let v = Version { major: 0, minor: 3, patch: 1, pad: 0 };
/// assert_eq!(v.minor, 3);
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Version {
    /// Semantic major (breaking change within the ABI epoch).
    pub major: u16,
    /// Semantic minor (additive changes).
    pub minor: u16,
    /// Semantic patch.
    pub patch: u16,
    /// Padding to align to 8 bytes.
    pub pad: u16,
}

/// Opaque identifier for a loaded cdylib instance (6.3 §2.1).
///
/// ```rust
/// use reovim_uapi_abi::ids::CdylibId;
/// use core::num::NonZeroU32;
///
/// let id = CdylibId(NonZeroU32::new(1).unwrap());
/// assert_eq!(id.0.get(), 1);
/// ```
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct CdylibId(pub NonZeroU32);

/// Opaque identifier for a registered domain (6.3 §2.1).
///
/// ```rust
/// use reovim_uapi_abi::ids::DomainId;
/// use core::num::NonZeroU32;
///
/// let id = DomainId(NonZeroU32::new(7).unwrap());
/// assert_eq!(id.0.get(), 7);
/// ```
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DomainId(pub NonZeroU32);

/// Connection-to-server identifier (6.3 §2.1).
///
/// ```rust
/// use reovim_uapi_abi::ids::ClientId;
///
/// let id = ClientId(42);
/// assert_eq!(id.0, 42);
/// ```
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ClientId(pub usize);

// SessionId(Arc<str>) does NOT appear here: it does not cross the ABI
// boundary as-is.  The boundary exchanges an opaque `*mut c_void` session
// handle (6.3 §2.1 note).  SessionId is a kernel-tier type.

/// Buffer identifier in the kernel (6.3 §2.1).
///
/// ```rust
/// use reovim_uapi_abi::ids::BufferId;
///
/// let id = BufferId(0);
/// assert_eq!(id.0, 0);
/// ```
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct BufferId(pub usize);

/// Window identifier in the kernel (6.3 §2.1).
///
/// ```rust
/// use reovim_uapi_abi::ids::WindowId;
///
/// let id = WindowId(3);
/// assert_eq!(id.0, 3);
/// ```
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct WindowId(pub usize);

/// Identifier for a resolved domain attachment (6.3 §2.1).
///
/// ```rust
/// use reovim_uapi_abi::ids::DomainAttachmentId;
///
/// let id = DomainAttachmentId(100);
/// assert_eq!(id.0, 100);
/// ```
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct DomainAttachmentId(pub u64);

/// Identifier for a pending domain attachment (6.3 §2.1).
///
/// ```rust
/// use reovim_uapi_abi::ids::PendingAttachmentId;
///
/// let id = PendingAttachmentId(200);
/// assert_eq!(id.0, 200);
/// ```
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct PendingAttachmentId(pub u64);

/// Identifier for an undo group (6.3 §2.1).
///
/// ```rust
/// use reovim_uapi_abi::ids::UndoGroupId;
///
/// let id = UndoGroupId(5);
/// assert_eq!(id.0, 5);
/// ```
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct UndoGroupId(pub u64);

/// Service registration key (6.3 §2.1).
///
/// ```rust
/// use reovim_uapi_abi::ids::ServiceKey;
/// use core::num::NonZeroU32;
///
/// let key = ServiceKey(NonZeroU32::new(1).unwrap());
/// assert_eq!(key.0.get(), 1);
/// ```
#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct ServiceKey(pub NonZeroU32);
