use core::ffi::c_void;

use super::{Bytes, Caller, Error, Identity, Registrar, Version, Wake};

/// Borrowed construction context; e.g. Global has scope=0 and zero session.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ModuleContext {
    /// Exact record extent, e.g. `size_of::<ModuleContext>()`.
    pub bytes: usize,
    /// Exact instance Resource, never a registry alias.
    pub instance: Identity,
    /// Global=0, Session=1; unknown tags reject.
    pub scope: u32,
    /// Zero for Global; otherwise the granted Session identity.
    pub session: Identity,
    /// Actual namespace Resource whose mount bounds exports.
    pub namespace_resource: Identity,
    /// Granted caller interface; borrowed through instance destruction.
    pub imports: Caller,
    /// Scoped exports; copied tables borrow the same retained owner.
    pub exports: Registrar,
}

/// Passive lifecycle, e.g. phase=2 reports Ready without publishing exports.
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct ModuleStatus {
    /// Constructed=0, Starting=1, Ready=2, Closing=3, Closed=4, Failed=5.
    pub phase: u32,
    /// Nonzero exactly for Failed; other phases require all-zero error data.
    pub failure: Error,
}

/// Serialized module execution, e.g. advance starts provisional providers.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ModuleDriver {
    /// Exact table extent; every callback is required.
    pub bytes: usize,
    /// Passive observation; fills status only on OK.
    pub status:
        Option<unsafe extern "C" fn(*mut c_void, *mut ModuleStatus) -> u32>,
    /// One bounded turn; progress is More=0, Idle=1, Stopped=2 on OK.
    pub advance: Option<
        unsafe extern "C" fn(*mut c_void, Wake, *mut u32, *mut Error) -> u32,
    >,
    /// Fences work; Busy keeps state and requires further advance calls.
    pub close: Option<unsafe extern "C" fn(*mut c_void, *mut Error) -> u32>,
}

/// Owned native instance; e.g. destroy runs after Closed and last provider use.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ModuleInstance {
    /// Exact record extent.
    pub bytes: usize,
    /// Private data shared by the module's callbacks and dependent providers.
    pub context: *mut c_void,
    /// Immutable execution table retained with code.
    pub driver: *const ModuleDriver,
    /// Required destructor; imported callbacks remain alive through this call.
    pub destroy: Option<unsafe extern "C" fn(*mut c_void)>,
}

/// Passive requested export and complete API. All spans are artifact-owned.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ModuleImport {
    /// Local ASCII name, at most 255 bytes; unique across the package.
    pub name: Bytes,
    /// Relative dependency export path, at most 4096 bytes.
    pub export_path: Bytes,
    /// Complete canonical API declaration, at most 65536 bytes.
    pub declaration: Bytes,
    /// Requested operation and factory-result masks, each within 0..=7.
    pub access: u32,
    pub create_access: u32,
    /// Requested loan retention, exactly 0 or 1.
    pub retain_loans: u32,
}

/// Passive exact dependency; scope is Same=0 or Global=1, optional is 0 or 1.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct ModuleDependency {
    pub module: [u8; 16],
    pub revision: u32,
    pub scope: u32,
    pub optional: u32,
    /// Requested exports; at most 256 across all package dependencies.
    pub imports: *const ModuleImport,
    pub import_count: usize,
}

/// Descriptor returned by `rv_module_entry`; inspection constructs no instance.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Module {
    /// Exact descriptor extent.
    pub bytes: usize,
    /// Exact supported ABI, e.g. `VERSION`.
    pub version: Version,
    /// Nonzero opaque package key, independent of any offered API.
    pub id: [u8; 16],
    /// Nonempty ASCII registry name, at most 255 bytes.
    pub name: Bytes,
    /// Exact nonzero package declaration revision.
    pub revision: u32,
    /// Supported state scopes: Global=1, Session=2.
    pub scopes: u32,
    /// Passive exact dependency selections, at most 256.
    pub dependencies: *const ModuleDependency,
    pub dependency_count: usize,
    /// Required relative export paths, copied at artifact admission.
    pub required: *const Bytes,
    /// Number of required paths; at most 256.
    pub required_count: usize,
    /// Maximum exports; at most 256 and at least `required_count`.
    pub export_limit: usize,
    /// Inert construction. OK transfers one instance; error leaves output zero
    /// and has no effects or retained construction context.
    pub construct: Option<
        unsafe extern "C" fn(
            *const ModuleContext,
            *mut ModuleInstance,
            *mut Error,
        ) -> u32,
    >,
}
