use core::ffi::c_void;

use super::{Args, Bytes, Creation, Error, Pending, Provider};

/// Per-open staging; e.g. a fresh open creates a cursor, capture shares it.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Open {
    /// Access-specific implementation owner supplied by Runtime.
    pub context: *mut c_void,
    /// Takes implementation only on OK; subject identity remains unchanged.
    pub stage:
        Option<unsafe extern "C" fn(*mut c_void, *const Provider) -> u32>,
}

/// Optional open callback, e.g. no data operation rights are passed or granted.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct OpenOps {
    /// Exact table extent, e.g. `size_of::<OpenOps>()`.
    pub bytes: usize,
    /// Stages the opened implementation and transfers pending only on OK.
    pub begin: Option<
        unsafe extern "C" fn(
            *mut c_void,
            *const Open,
            *mut Pending,
            *mut Error,
        ) -> u32,
    >,
}

/// Read selection; e.g. `read_at` sets positional=1 with an explicit offset.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Read {
    /// Zero sequential or one positional; unknown values reject.
    pub positional: u32,
    /// Byte offset; must be zero for sequential reads.
    pub offset: u64,
    /// Maximum bytes, bounded/aligned before provider invocation.
    pub maximum: u64,
}

/// Borrowed byte write, e.g. a pending adapter copies before returning.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Write {
    /// Zero sequential or one positional; unknown values reject.
    pub positional: u32,
    /// Byte offset; must be zero for sequential writes.
    pub offset: u64,
    /// Borrowed bytes; partial acceptance is an explicit transfer count.
    pub data: Bytes,
}

/// Independent directions; e.g. readonly positional access need not write.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct IoOps {
    /// Exact table extent, e.g. `size_of::<IoOps>()`.
    pub bytes: usize,
    /// Sequential=1, positional=2; zero requires absent read callback.
    pub read_modes: u32,
    /// Independent sequential/positional bits; zero requires absent write.
    pub write_modes: u32,
    /// Offset/transfer alignment, at least one byte.
    pub alignment: u64,
    /// Nonzero per-call byte limit; e.g. a driver DMA window.
    pub transfer_limit: u64,
    /// Packet Type, empty exactly when receive is absent.
    pub receive_type: Bytes,
    /// Packet Type, empty exactly when send is absent.
    pub send_type: Bytes,
    /// Returns bytes and EOF; only present for nonzero `read_modes`.
    pub read: Option<
        unsafe extern "C" fn(
            *mut c_void,
            *const Read,
            *mut Pending,
            *mut Error,
        ) -> u32,
    >,
    /// Returns accepted U64 bytes; only present for nonzero `write_modes`.
    pub write: Option<
        unsafe extern "C" fn(
            *mut c_void,
            *const Write,
            *mut Pending,
            *mut Error,
        ) -> u32,
    >,
    /// Optional declared packet receive preserving item boundaries.
    pub receive: Option<
        unsafe extern "C" fn(
            *mut c_void,
            *const Args,
            *mut Pending,
            *mut Error,
        ) -> u32,
    >,
    /// Optional declared packet send; receipt means local acceptance.
    pub send: Option<
        unsafe extern "C" fn(
            *mut c_void,
            *const Args,
            *mut Pending,
            *mut Error,
        ) -> u32,
    >,
}

/// Provider namespace, e.g. stable instance/object keys intern to one identity.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct NamespaceOps {
    /// Exact table extent, e.g. `size_of::<NamespaceOps>()`.
    pub bytes: usize,
    /// Required observation; canonical U32 is 1 or 3, with zero activity.
    /// Example: empty directory yields CONTAINER|EMPTY (3), not a byte extent.
    pub flags: Option<
        unsafe extern "C" fn(*mut c_void, *mut Pending, *mut Error) -> u32,
    >,
    /// Looks up a logical name and returns the provider's opaque object key.
    pub lookup: Option<
        unsafe extern "C" fn(
            *mut c_void,
            Bytes,
            *mut Pending,
            *mut Error,
        ) -> u32,
    >,
    /// Lists a bounded name/key page using cursor and byte limit.
    pub list: Option<
        unsafe extern "C" fn(
            *mut c_void,
            u64,
            u64,
            *mut Pending,
            *mut Error,
        ) -> u32,
    >,
    /// Stages a subject while revalidating its selected object key.
    pub materialize: Option<
        unsafe extern "C" fn(
            *mut c_void,
            Bytes,
            *const Creation,
            *mut Pending,
            *mut Error,
        ) -> u32,
    >,
    /// Creates through the parent's default factory under creation custody.
    pub create: Option<
        unsafe extern "C" fn(
            *mut c_void,
            Bytes,
            *const Creation,
            *mut Pending,
            *mut Error,
        ) -> u32,
    >,
    /// Removes a name only after revalidating the captured expected key.
    pub remove: Option<
        unsafe extern "C" fn(
            *mut c_void,
            Bytes,
            Bytes,
            *mut Pending,
            *mut Error,
        ) -> u32,
    >,
    /// Links a retained source key in the same mounted namespace.
    /// Example: an alias returns Unit; foreign providers are not passed here.
    pub link: Option<
        unsafe extern "C" fn(
            *mut c_void,
            Bytes,
            Bytes,
            *mut Pending,
            *mut Error,
        ) -> u32,
    >,
    /// Atomic move: canonical old name/key, Root-or-Key destination, new name.
    /// Example: the destination key remains retained through pending finish.
    pub rename: Option<
        unsafe extern "C" fn(
            *mut c_void,
            *const Args,
            *mut Pending,
            *mut Error,
        ) -> u32,
    >,
}
