//! 6.3 §4, 6.4 — Config slice ABI.
//!
//! The exact wire format for the merged + validated config that the kernel
//! hands to each participant (CFG6).  One shape for every participant kind.
//!
//! **Lifetime:** `ConfigSlice` and all bytes it references are valid only
//! during the participant's init call.  The participant must copy anything
//! it retains (6.4 §11).
use crate::slices::{ByteSlice, StrSlice};

/// Top-level config slice handed to a participant at init time (6.4 §2).
///
/// `entries` is in schema-declared order.  String/path/enum/canonical-toml
/// bytes are valid for the lifetime of the participant's init call.  The
/// kernel owns; the participant must not free or mutate (CFG6).
///
/// # Examples
///
/// ```rust
/// use reovim_uapi_abi::config::{ConfigSlice, ConfigKvp};
/// use reovim_uapi_abi::slices::StrSlice;
/// use core::ptr;
///
/// let slice = ConfigSlice {
///     abi_version: 1,
///     namespace: StrSlice { data: ptr::null(), len: 0 },
///     schema_version: 0,
///     entries: ptr::null::<ConfigKvp>(),
///     len: 0,
///     total_bytes: 0,
///     reserved: 0,
/// };
/// assert_eq!(slice.abi_version, 1);
/// ```
#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct ConfigSlice {
    /// Mirrors `REOVIM_CONFIG_ABI_VERSION`.
    pub abi_version: u32,
    /// Participant namespace string (UTF-8, kernel-validated).
    pub namespace: StrSlice,
    /// Mirrors the participant's `REOVIM_CONFIG_SCHEMA_VERSION`.
    pub schema_version: u32,
    /// Pointer to the KVP array; `len` entries long.
    pub entries: *const ConfigKvp,
    /// Number of entries in `entries`.
    pub len: usize,
    /// Total bytes of all string/path/toml values; for CFG6 cap accounting.
    pub total_bytes: usize,
    /// Reserved; must be zero.
    pub reserved: u32,
}

/// One config key/value pair (6.4 §3).
///
/// `dotted_key` uses `[a-z0-9-]+` segments separated by `.`.
/// Order in `entries` is schema-declared field order.
///
/// # Examples
///
/// ```rust
/// use reovim_uapi_abi::config::{ConfigKvp, ConfigValueKind, ConfigSource, ConfigValue};
/// use reovim_uapi_abi::slices::StrSlice;
/// use core::ptr;
///
/// let kvp = ConfigKvp {
///     dotted_key: StrSlice { data: ptr::null(), len: 0 },
///     kind: ConfigValueKind::Bool,
///     value: ConfigValue { bool_value: 1 },
///     source: ConfigSource::Default,
///     flags: 0,
/// };
/// assert_eq!(kvp.kind, ConfigValueKind::Bool);
/// ```
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ConfigKvp {
    /// Dotted-key path, e.g. `"ui.replay-max-wait-ms"`.
    pub dotted_key: StrSlice,
    /// Discriminates which union member of `value` is active.
    pub kind: ConfigValueKind,
    /// The value; interpret per `kind`.
    pub value: ConfigValue,
    /// Which config layer supplied the final value.
    pub source: ConfigSource,
    /// Flags bitfield (6.4 §6).
    pub flags: u32,
}

/// Discriminates the active member of [`ConfigValue`] (6.4 §4).
///
/// `Unknown = 0` is reserved-invalid.
///
/// # Examples
///
/// ```rust
/// use reovim_uapi_abi::config::ConfigValueKind;
///
/// assert_eq!(ConfigValueKind::Unknown as u8, 0);
/// assert_eq!(ConfigValueKind::CanonicalToml as u8, 10);
/// ```
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfigValueKind {
    /// Reserved-invalid.
    Unknown = 0,
    /// Boolean; `value.bool_value` is 0 (false) or 1 (true).
    Bool = 1,
    /// Unsigned 32-bit integer; `value.u32_value`.
    U32 = 2,
    /// Signed 32-bit integer; `value.i32_value`.
    I32 = 3,
    /// Unsigned 64-bit integer; `value.u64_value`.
    U64 = 4,
    /// Signed 64-bit integer; `value.i64_value`.
    I64 = 5,
    /// IEEE 754 double; `value.f64_value`.
    F64 = 6,
    /// UTF-8 string; `value.bytes` (kernel-validated).
    String = 7,
    /// OS-encoded path bytes; `value.bytes` (opaque).
    Path = 8,
    /// String-form enum variant; discriminant in `flags` low byte.
    Enum = 9,
    /// List/table compound as canonical TOML bytes; `value.bytes`.
    CanonicalToml = 10,
}

/// Config layer that supplied a field's final value (6.4 §7).
///
/// # Examples
///
/// ```rust
/// use reovim_uapi_abi::config::ConfigSource;
///
/// assert_eq!(ConfigSource::Default as u8, 0);
/// assert_eq!(ConfigSource::Force as u8, 6);
/// ```
#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ConfigSource {
    /// Hardcoded schema default.
    Default = 0,
    /// System-wide config.
    System = 1,
    /// User config.
    User = 2,
    /// Project config.
    Project = 3,
    /// Environment variable.
    Env = 4,
    /// Command-line argument.
    Cli = 5,
    /// Force-set override.
    Force = 6,
}

/// Union of all possible config values (6.4 §5).
///
/// The active member is determined by the sibling `kind` field on
/// [`ConfigKvp`].  Reading any member other than the one indicated by
/// `kind` is undefined behaviour.
///
/// # Examples
///
/// ```rust
/// use reovim_uapi_abi::config::ConfigValue;
///
/// let v = ConfigValue { u32_value: 42 };
/// // SAFETY: we just set u32_value.
/// assert_eq!(unsafe { v.u32_value }, 42);
/// ```
// `Debug` is not derived for `ConfigValue` — unions cannot derive it since
// the active member is unknown at the Debug call site.  The caller knows
// which arm is active from the sibling `kind` field on `ConfigKvp`.
#[repr(C)]
#[derive(Clone, Copy)]
pub union ConfigValue {
    /// Active when `kind == Bool`; 0 = false, 1 = true.
    pub bool_value: u8,
    /// Active when `kind == U32`.
    pub u32_value: u32,
    /// Active when `kind == I32`.
    pub i32_value: i32,
    /// Active when `kind == U64`.
    pub u64_value: u64,
    /// Active when `kind == I64`.
    pub i64_value: i64,
    /// Active when `kind == F64`.
    pub f64_value: f64,
    /// Active when `kind == String | Path | Enum | CanonicalToml`.
    pub bytes: ByteSlice,
}
