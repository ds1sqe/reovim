//! Experimental native module and caller tables for one target platform.
//!
//! All input views are borrowed for the call. The adapter copies output data
//! before release. A transferred immutable state lives until `drop_state`.

use core::ffi::c_void;

/// Experimental ABI `SemVer`; this stays at 0.1.0 until official release.
pub const VERSION: Version =
    Version { major: 0, minor: 1, patch: 0, reserved: 0 };

#[repr(C)]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
    /// Reserved for future use; producers must set this to zero.
    pub reserved: u32,
}
pub const RESOURCE: u32 = 1;
pub const MODEL: u32 = 2;
pub const READ: u32 = 1;
pub const WRITE: u32 = 2;

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Bytes {
    pub data: *const u8,
    pub len: usize,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Contract {
    pub name: Bytes,
    pub version: u32,
}

#[repr(C)]
pub struct Request {
    pub method: u32,
    pub subject: u64,
    pub contract: Contract,
    pub data: Bytes,
    pub resources: *const u64,
    pub resource_count: usize,
}

#[repr(C)]
pub struct Reply {
    pub error: u64,
    pub data: Bytes,
    pub state: *mut c_void,
    pub state_bytes: u64,
}

pub type Call = unsafe extern "C" fn(*mut c_void, *const Request) -> Reply;
pub type Prepare =
    unsafe extern "C" fn(*mut c_void, *const c_void, *const Request) -> Reply;
pub type Observe =
    unsafe extern "C" fn(*mut c_void, *const c_void, *const Request) -> Reply;
pub type Inspect =
    unsafe extern "C" fn(*mut c_void, *const c_void, *const Request) -> Pending;

/// Deferred preparation with one-time state transfer.
///
/// Poll reports readiness without transferring state. Take transfers one Reply
/// after readiness, consumed with the Model's `release`/`drop_state` callbacks.
/// Release cancels unfinished work and drops any state not taken. The caller
/// retains the Model context, code and borrowed old state through release.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct StatePending {
    pub context: *mut c_void,
    pub poll: unsafe extern "C" fn(*mut c_void, Wake) -> StatePoll,
    pub take: unsafe extern "C" fn(*mut c_void) -> Reply,
    pub release: unsafe extern "C" fn(*mut c_void),
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct StatePoll {
    /// 0 pending, 1 ready, 2 failed. No state ownership transfers here.
    pub status: u32,
    pub error: u64,
}

reovim_vtable::rv_vtable! {
    /// A loaded module returns this immutable table via `rv_module_entry`.
    /// No Rust containers, trait objects or Futures cross it.
#[derive(Clone, Copy)]
pub struct Module {
    version: Version = VERSION;
    pub kind: u32,
    /// Borrowed immutable declaration, retained with the module's code.
    pub declaration: *const c_void,
    pub create_contract: Contract,
    /// Initialization's borrowed Resource-slot permissions, in input order.
    pub initial_input: *const u32,
    pub initial_input_count: usize,
    pub read_contract: Contract,
    pub edit_contract: Contract,
    pub create: Option<
        unsafe extern "C" fn(*const c_void, *const Connection) -> *mut c_void,
    >,
    pub destroy: Option<unsafe extern "C" fn(*mut c_void)>,
    pub call: Option<Call>,
    pub initialize: Option<Call>,
    pub prepare: Option<Prepare>,
    pub initialize_async: Option<
        unsafe extern "C" fn(*mut c_void, *const Request) -> StatePending,
    >,
    /// Async callbacks copy request views before returning. The old state stays
    /// borrowed until pending release. Async slots supersede finite ones.
    pub prepare_async: Option<
        unsafe extern "C" fn(
            *mut c_void, *const c_void, *const Request,
        ) -> StatePending,
    >,
    pub observe: Option<Observe>,
    pub inspect: Option<Inspect>,
    pub release: Option<unsafe extern "C" fn(*mut c_void, *mut Reply)>,
    pub drop_state: Option<unsafe extern "C" fn(*mut c_void, *mut c_void)>,
    pub unload: Option<unsafe extern "C" fn(*mut c_void) -> u64>,
    /// Resource exports use describe/request when exports is nonzero.
    pub exports: usize,
    pub describe: Option<
        unsafe extern "C" fn(
            *const c_void,
            usize,
            *mut ResourceExport,
        ) -> Failure,
    >,
    pub request: Option<
        unsafe extern "C" fn(*mut c_void, usize, *const Request) -> Pending,
    >,
    /// Passive catalog identity; an empty name is not selectable by contract.
    pub identity: Contract,
    /// Required catalog contracts, borrowed for the lifetime of this table.
    pub dependencies: *const Contract,
    pub dependency_count: usize,
}
}

/// Caller ABI: each connection and pending call owns one reference.
/// Clone with retain; release through its originating table.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Connection {
    pub context: *mut c_void,
    pub table: *const Access,
}

#[repr(C)]
#[derive(Clone, Copy)]
pub struct Failure {
    pub code: u64,
    pub resource: u64, // Zero means no associated Resource.
}

#[repr(C)]
pub struct Entry {
    pub name: Bytes,
    pub resource: u64,
}

/// All views remain valid until the matching release callback.
#[repr(C)]
pub struct Listing {
    pub entries: *const Entry,
    pub count: usize,
    pub owner: *mut c_void,
    pub release: unsafe extern "C" fn(*mut c_void),
}

/// Thread-safe wake reference. Poll borrows it; retention requires retain.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Wake {
    pub context: *const c_void,
    pub retain: unsafe extern "C" fn(*const c_void),
    pub release: unsafe extern "C" fn(*const c_void),
    pub wake: unsafe extern "C" fn(*const c_void),
}

/// Poll returns 0 pending, 1 success, 2 rejected, 3 failed, 4 unknown.
///
/// Terminal views remain valid until the call is released. Unknown supplies
/// the original request's recovery locator; it never authorizes replay.
#[repr(C)]
pub struct Outcome {
    pub status: u32,
    pub error: Failure,
    pub contract: Contract,
    pub data: Bytes,
    pub resources: *const u64,
    pub resource_count: usize,
    pub recovery: Bytes,
}

#[repr(C)]
pub struct Pending {
    pub context: *mut c_void,
    pub poll: unsafe extern "C" fn(*mut c_void, Wake) -> Outcome,
    pub release: unsafe extern "C" fn(*mut c_void),
}

/// Exact Resource surface metadata. Slot permissions use READ/WRITE bits.
/// Views are immutable and remain valid while the module is loaded.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Surface {
    pub contract: Contract,
    pub methods: u32,
    pub input: *const u32,
    pub input_count: usize,
    pub output: *const u32,
    pub output_count: usize,
}

/// One relative export; an empty path denotes the selected mount itself.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ResourceExport {
    pub path: Bytes,
    pub surfaces: *const Surface,
    pub surface_count: usize,
    /// Zero declares fixed slots; one forwards complete Messages and grants.
    pub forwarding: u32,
}

/// One borrowed provider descriptor; retain/release own its context and views.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Provider {
    pub context: *mut c_void,
    pub table: *const ProviderOps,
    pub surfaces: *const Surface,
    pub surface_count: usize,
    pub forwarding: u32,
    /// One declares client-owned access with explicit close.
    pub owned: u32,
}

reovim_vtable::rv_vtable! {
    pub struct ProviderOps {
        version: Version = VERSION;
        pub retain: unsafe extern "C" fn(*mut c_void),
        pub release: unsafe extern "C" fn(*mut c_void),
        pub request:
            unsafe extern "C" fn(*mut c_void, *const Request) -> Pending,
        pub close: unsafe extern "C" fn(*mut c_void) -> Failure,
        pub unload: unsafe extern "C" fn(*mut c_void) -> Failure,
    }
}

reovim_vtable::rv_vtable! {
    /// Single-threaded access; wake references alone may cross threads.
    ///
    /// No callback may unwind. Request copies input before returning. Poll only
    /// observes completion; the implementation owner drives execution.
pub struct Access {
    version: Version = VERSION;
    pub retain: unsafe extern "C" fn(*mut c_void),
    pub release: unsafe extern "C" fn(*mut c_void),
    pub root: unsafe extern "C" fn(*mut c_void, *mut u64) -> Failure,
    pub resolve:
        unsafe extern "C" fn(*mut c_void, u64, Bytes, *mut u64) -> Failure,
    pub children:
        unsafe extern "C" fn(*mut c_void, u64, *mut Listing) -> Failure,
    pub request: unsafe extern "C" fn(*mut c_void, *const Request) -> Pending,
    pub close: unsafe extern "C" fn(*mut c_void, u64) -> Failure,
    pub reconnect:
        unsafe extern "C" fn(*mut c_void, *mut Connection) -> Failure,
    pub withdraw: unsafe extern "C" fn(*mut c_void, u64) -> Failure,
    pub publish: unsafe extern "C" fn(*mut c_void, u64, Bytes,
        *const Provider, *mut u64) -> Failure,
    pub capture: unsafe extern "C" fn(*mut c_void, u64, u32,
        *mut Connection) -> Failure,
}
}
