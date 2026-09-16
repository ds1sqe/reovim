use core::ffi::c_void;

use super::{Args, Error, Identity, Outcome, Wake};

/// Provider custody: poll, take, release outcome, finish. After UNKNOWN,
/// reconcile may start another generation of the same effect in between.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct PendingOps {
    /// Exact table extent, e.g. `size_of::<PendingOps>()`.
    pub bytes: usize,
    /// Returns PENDING=0 or READY=1 without transferring an outcome.
    pub poll: Option<unsafe extern "C" fn(*mut c_void, Wake) -> u32>,
    /// Fills output only on OK, once per generation; else NotReady/Closed.
    pub take: Option<unsafe extern "C" fn(*mut c_void, *mut Outcome) -> u32>,
    /// Cancellation: ACCEPTED=0 or `TOO_LATE=1`, not effect evidence.
    pub cancel: Option<unsafe extern "C" fn(*mut c_void) -> u32>,
    /// Optional; after taken UNKNOWN, refines the original work.
    pub reconcile: Option<unsafe extern "C" fn(*mut c_void) -> u32>,
    /// OK destroys context; Busy leaves it owned and progressable by Runtime.
    pub finish: Option<unsafe extern "C" fn(*mut c_void) -> u32>,
}

/// One request owner, e.g. retained after a caller drops its delivery view.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Pending {
    /// Request-private context, valid until finish returns OK.
    pub context: *mut c_void,
    /// Immutable complete table, retained with its executable code.
    pub ops: *const PendingOps,
}

/// Bound executable entry. Example: exec receives no command or API number.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ExecOps {
    /// Exact table extent, e.g. `size_of::<ExecOps>()`.
    pub bytes: usize,
    /// OK transfers pending; errors fill error and leave output untouched.
    /// Error proves no effect or retained input from this begin callback.
    pub exec: Option<
        unsafe extern "C" fn(
            *mut c_void,
            *const Args,
            *mut Pending,
            *mut Error,
        ) -> u32,
    >,
}

/// Bound control entry. Example: command 2 is local to this interface.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ControlOps {
    /// Exact table extent, e.g. `size_of::<ControlOps>()`.
    pub bytes: usize,
    /// Required; command is nonzero. Ownership follows `ExecOps`.
    pub ioctl: Option<
        unsafe extern "C" fn(
            *mut c_void,
            u32,
            *const Args,
            *mut Pending,
            *mut Error,
        ) -> u32,
    >,
}

/// Optional expected state. Example: absent means every field is zero.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct Expected {
    /// Zero absent, one present; unknown values reject before admission.
    pub present: u32,
    /// Qualified subject whose revision is checked; e.g. the edited Buffer.
    pub subject: Identity,
    /// Captured revision; e.g. stale writes reject before installation.
    pub revision: u64,
}

/// Captured submission. Example: the same reserved ID and exact fields join
/// original work; changing any field returns Conflict without another call.
#[repr(C)]
pub struct Call {
    /// Caller-reserved nonzero number, never a caller-selected identity.
    pub request: u64,
    /// Common code; e.g. Exec=10, Ioctl=11, SetAccess=21.
    pub operation: u32,
    /// Owned access token; e.g. zero only for Acquire.
    pub subject: u64,
    /// Local nonzero ioctl command; zero for other operations.
    pub command: u32,
    /// Expected immutable state for editing, absent otherwise.
    pub expected: Expected,
    /// Borrowed input, copied before return; no caller-chosen schema.
    pub input: Args,
}

/// Caller custody. Example: reserve, submit, recover a lost view using result,
/// adopt returned access once, then release the request receipt.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct CallerOps {
    /// Exact table extent, e.g. `size_of::<CallerOps>()`.
    pub bytes: usize,
    /// Reserves bounded custody; initializes request only on OK.
    pub reserve: Option<unsafe extern "C" fn(*mut c_void, *mut u64) -> u32>,
    /// Admits once; output is a delivery view, not provider custody.
    pub submit: Option<
        unsafe extern "C" fn(
            *mut c_void,
            *const Call,
            *mut Pending,
            *mut Error,
        ) -> u32,
    >,
    /// Gets a view of existing work, even after the subject is closed.
    /// Reserved returns `NotReady` and leaves pending output untouched.
    pub result: Option<
        unsafe extern "C" fn(*mut c_void, u64, *mut Pending, *mut Error) -> u32,
    >,
    /// Requests cancellation on the original admitted request.
    pub cancel: Option<unsafe extern "C" fn(*mut c_void, u64) -> u32>,
    /// Refines original UNKNOWN work; never submits its effect again.
    pub reconcile: Option<unsafe extern "C" fn(*mut c_void, u64) -> u32>,
    /// Idempotently transfers the original result's returned access once.
    pub adopt: Option<unsafe extern "C" fn(*mut c_void, u64) -> u32>,
    /// Releases custody or schedules cleanup; released IDs never resurrect.
    pub release: Option<unsafe extern "C" fn(*mut c_void, u64) -> u32>,
}

/// Borrowed authenticated context, e.g. a module's granted imports.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Caller {
    /// Caller authority/custody; not a concrete Runtime pointer contract.
    pub context: *mut c_void,
    /// Immutable callbacks owned by assembly through caller use.
    pub ops: *const CallerOps,
}
