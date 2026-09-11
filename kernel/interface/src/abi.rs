//! Native module v2 and caller v1 tables for one target platform.
//!
//! All input views are borrowed for the call. The adapter copies output data
//! before release. A transferred immutable state lives until `drop_state`.

use core::ffi::c_void;

pub const VERSION: u32 = 1;
pub const MODULE_VERSION: u32 = 2;
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
pub type Observe = unsafe extern "C" fn(*mut c_void, *const c_void) -> Reply;

/// A loaded module returns a pointer to this immutable table via
/// `reovim_module_v2`. No Rust containers, trait objects or Futures cross it.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Module {
    pub version: u32,
    pub size: u32,
    pub kind: u32,
    pub create_contract: Contract,
    pub read_contract: Contract,
    pub edit_contract: Contract,
    pub create: Option<unsafe extern "C" fn(*const Connection) -> *mut c_void>,
    pub destroy: Option<unsafe extern "C" fn(*mut c_void)>,
    pub call: Option<Call>,
    pub initialize: Option<Call>,
    pub prepare: Option<Prepare>,
    pub observe: Option<Observe>,
    pub release: Option<unsafe extern "C" fn(*mut c_void, *mut Reply)>,
    pub drop_state: Option<unsafe extern "C" fn(*mut c_void, *mut c_void)>,
    pub unload: Option<unsafe extern "C" fn(*mut c_void) -> u64>,
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

/// Single-threaded access; wake references alone may cross threads.
///
/// No callback may unwind. Request copies input before returning. Poll only
/// observes completion; the implementation owner drives execution separately.
#[repr(C)]
pub struct Access {
    pub version: u32,
    pub size: u32,
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
}
