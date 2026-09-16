use core::ffi::c_void;

use super::{Args, Error, Pending, Provider, Write};

/// Immutable state ownership, e.g. History retains before releasing a node.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct StateOps {
    /// Exact table extent, e.g. `size_of::<StateOps>()`.
    pub bytes: usize,
    /// Retains immutable state; invoked outside the atomic commit cut.
    pub retain: Option<unsafe extern "C" fn(*const c_void)>,
    /// Releases one state owner with code pinned, outside commit guards.
    pub release: Option<unsafe extern "C" fn(*const c_void)>,
    /// Constructs inert readonly access. OK transfers provider; failure fills
    /// error and leaves output untouched. No external I/O or waiting.
    pub snapshot: Option<
        unsafe extern "C" fn(*const c_void, *mut Provider, *mut Error) -> u32,
    >,
}

/// One owned immutable state reference, e.g. a History node's private value.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct State {
    /// Private immutable storage; only its originating provider interprets it.
    pub data: *const c_void,
    /// Immutable lifetime/snapshot table, retained with code through release.
    pub ops: *const StateOps,
    /// Declared quota cost; e.g. a sparse model charges retained storage.
    pub cost: u64,
}

/// Request-scoped preparation capability, e.g. a splice proposes one candidate.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Edit {
    /// Selected immutable state, borrowed through preparation settlement.
    pub before: *const c_void,
    /// Request staging owner, not general access to Core installation.
    pub context: *mut c_void,
    /// On OK retains one candidate; caller still releases its original owner.
    /// Requires WRITE and rejects a second proposal for this request.
    pub propose: Option<unsafe extern "C" fn(*mut c_void, *const State) -> u32>,
}

/// State preparation, e.g. edits never mutate Buffer state through ioctl.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct EditOps {
    /// Exact table extent, e.g. `size_of::<EditOps>()`.
    pub bytes: usize,
    /// Declared exec (command zero) or ioctl preparation over selected state.
    pub prepare: Option<
        unsafe extern "C" fn(
            *mut c_void,
            u32,
            *const Edit,
            *const Args,
            *mut Pending,
            *mut Error,
        ) -> u32,
    >,
    /// Optional byte-write preparation using this same immutable state owner.
    pub write: Option<
        unsafe extern "C" fn(
            *mut c_void,
            *const Edit,
            *const Write,
            *mut Pending,
            *mut Error,
        ) -> u32,
    >,
}

/// Staged state subject, e.g. Buffer installation captures all three records.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct StateInit {
    /// Initial immutable state; staging retains an owner only on success.
    pub initial: State,
    /// Offered interfaces and owned implementation context.
    pub implementation: Provider,
    /// Optional standard byte edits; null for non-byte state.
    pub byte_edits: *const EditOps,
}

/// Source refresh/publication callbacks, e.g. a save retains its original work
/// across UNKNOWN and reconciles without applying a destination twice.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct BackingOps {
    /// Exact table extent, e.g. `size_of::<BackingOps>()`.
    pub bytes: usize,
    /// Optional refresh stages a candidate using captured sources and policy.
    pub refresh: Option<
        unsafe extern "C" fn(
            *mut c_void,
            *const Edit,
            *const Args,
            *mut Pending,
            *mut Error,
        ) -> u32,
    >,
    /// Optional conditional publication over captured immutable state.
    pub publish: Option<
        unsafe extern "C" fn(
            *mut c_void,
            *const c_void,
            *const Args,
            *mut Pending,
            *mut Error,
        ) -> u32,
    >,
}

/// Staged Buffer; e.g. sources and initial state become visible together.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct BufferInit {
    /// Initial state plus implementation and optional byte edits.
    pub state: StateInit,
    /// Canonical source attachment entries; copied/retained on staging.
    pub attachments: Args,
    /// Optional callbacks using implementation.context; absent is Unsupported.
    pub backing: *const BackingOps,
}

/// Factory staging capability, e.g. Parse stages one immutable tree Resource.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Creation {
    /// Stages a new namespace object and its copied key as one primary target.
    pub keyed: Option<
        unsafe extern "C" fn(*mut c_void, super::Bytes, *const Provider) -> u32,
    >,
    /// Caller-owned creation batch; no publication before successful admission.
    pub context: *mut c_void,
    /// Stages the sole ordinary primary; takes context ownership only on OK.
    pub resource:
        Option<unsafe extern "C" fn(*mut c_void, *const Provider) -> u32>,
    /// Stages the sole Buffer primary, including atomic History initialization.
    pub buffer:
        Option<unsafe extern "C" fn(*mut c_void, *const BufferInit) -> u32>,
    /// Stages retained revisioned state without Buffer History, e.g. Session.
    pub retained:
        Option<unsafe extern "C" fn(*mut c_void, *const StateInit) -> u32>,
    /// On OK returns provisional access for a declared auxiliary output slot.
    pub auxiliary: Option<
        unsafe extern "C" fn(*mut c_void, *const Provider, *mut u64) -> u32,
    >,
}

/// Explicit factory entry, e.g. watch acquisition is a creating ioctl.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct FactoryOps {
    /// Exact table extent, e.g. `size_of::<FactoryOps>()`.
    pub bytes: usize,
    /// Stages one primary. Command is zero for exec, nonzero for ioctl.
    /// OK transfers pending; errors leave output and input ownership unchanged.
    pub create: Option<
        unsafe extern "C" fn(
            *mut c_void,
            u32,
            *const Creation,
            *const Args,
            *mut Pending,
            *mut Error,
        ) -> u32,
    >,
}
