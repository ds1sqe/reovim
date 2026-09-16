//! Native declared-operation ABI for independent providers and callers.
//!
//! `PendingOps::poll` observes readiness; `PendingOps::take` transfers one
//! outcome owner. Runtime retains code through result release and request
//! finish. No Rust collections, trait objects or futures cross these records.
//! Callbacks are serialized and must not unwind. Nonempty spans and callbacks
//! must stay valid for their documented lifetime. Checking table sizes cannot
//! make arbitrary native pointers safe.

use core::ffi::c_void;

mod io;
mod module;
mod provider;
mod request;
mod state;

pub use io::{IoOps, NamespaceOps, Open, OpenOps, Read, Write};
pub use module::{
    Module, ModuleContext, ModuleDependency, ModuleDriver, ModuleImport,
    ModuleInstance, ModuleStatus,
};
pub use provider::{Interface, Provider, Registrar};
pub use request::{
    Call, Caller, CallerOps, ControlOps, ExecOps, Expected, Pending, PendingOps,
};
pub use state::{
    BackingOps, BufferInit, Creation, Edit, EditOps, FactoryOps, State,
    StateInit, StateOps,
};

/// Experimental ABI version. Example: an entry advertises `VERSION` exactly.
pub const VERSION: Version =
    Version { major: 0, minor: 1, patch: 0, reserved: 0 };

/// Exact ABI version; reserved must be zero, e.g. `VERSION.reserved == 0`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Version {
    /// Major component. Example: `VERSION.major == 0`.
    pub major: u32,
    /// Minor component. Example: `VERSION.minor == 1`.
    pub minor: u32,
    /// Patch component. Example: `VERSION.patch == 0`.
    pub patch: u32,
    /// Must be zero; nonzero rejects before loading callable tables.
    pub reserved: u32,
}

/// Copied identity, never access. Example: zero serial denotes absent optional
/// error detail; an actual Resource always has a nonzero serial.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Identity {
    /// Kernel instance key. Example: another Kernel uses a distinct epoch.
    pub epoch: [u8; 16],
    /// Nonreused object number. Example: deletion never frees this number.
    pub serial: u64,
}

/// Borrowed bytes. Example: null plus zero length denotes an empty span.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Bytes {
    /// Readable storage, e.g. a canonical declaration's first byte.
    pub ptr: *const u8,
    /// Accessible byte extent; e.g. zero for Unit.
    pub len: usize,
}

/// Borrowed caller-scoped access tokens, e.g. slots used by Ref values.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Refs {
    /// Token storage. Example: slot zero reads the first checked element.
    pub ptr: *const u64,
    /// Token count; every slot must occur in the declared value.
    pub len: usize,
}

/// Canonical value and separate access slots; e.g. Ref encodes a slot index.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Args {
    /// Declared bytes; e.g. U32 uses four little-endian bytes.
    pub data: Bytes,
    /// Borrowed input or owned result slots, according to the call boundary.
    pub refs: Refs,
}

/// Machine failure plus optional copied identity; e.g. detail grants no access.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Error {
    /// Extensible machine code; unknown codes remain reportable unchanged.
    pub code: u32,
    /// All zero means absent; otherwise an instance-qualified diagnostic ID.
    pub detail: Identity,
}

/// Borrowed progress signal. Example: retain before storing beyond poll and
/// release after the last wake; all three callbacks are required.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Wake {
    /// Opaque signal owner, e.g. an adapter-owned scheduler waker.
    pub context: *mut c_void,
    /// Signals progress; e.g. a previously pending device is ready.
    pub wake: Option<unsafe extern "C" fn(*mut c_void)>,
    /// Retains one signal reference before asynchronous use.
    pub retain: Option<unsafe extern "C" fn(*mut c_void)>,
    /// Releases that retained reference when no more wake can occur.
    pub release: Option<unsafe extern "C" fn(*mut c_void)>,
}

/// One owned outcome generation. Example: UNKNOWN contains declared progress
/// without returned access; its request retains reconciliation and cleanup.
#[repr(C)]
pub struct Outcome {
    /// OK=0, FAILED=1, UNKNOWN=2; readiness is separate from the outcome.
    pub status: u32,
    /// Known READ=1, WRITE=2, STATUS=4 events within the declared allowance.
    pub activity: u32,
    /// Failure cause; e.g. an unknown provider code remains intact.
    pub error: Error,
    /// OK output or UNKNOWN progress; FAILED has empty bytes and references.
    pub value: Args,
    /// Owns spans and unadopted references until the matching release.
    pub owner: *mut c_void,
    /// Required even for empty values; never closes adopted caller access.
    pub release: Option<unsafe extern "C" fn(*mut c_void)>,
}
