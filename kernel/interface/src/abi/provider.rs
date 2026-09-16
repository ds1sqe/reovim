use core::ffi::c_void;

use super::{
    Bytes, ControlOps, EditOps, ExecOps, FactoryOps, IoOps, NamespaceOps,
    OpenOps,
};

/// Offered interface. Example: Factory exec requires factory, ordinary exec
/// requires exec. Unoffered table families use null pointers.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Interface {
    /// Exact record extent, e.g. `size_of::<Interface>()`.
    pub bytes: usize,
    /// Context bound to this interface's operations; retained by the provider.
    pub context: *mut c_void,
    /// Complete canonical declaration, including the nonzero API key.
    pub declaration: Bytes,
    /// Ordinary executable table; no command number is passed.
    pub exec: *const ExecOps,
    /// Ordinary control table; commands are local and nonzero.
    pub control: *const ControlOps,
    /// Immutable state preparation table for Edit methods only.
    pub edit: *const EditOps,
    /// Creation staging table for Factory methods only.
    pub factory: *const FactoryOps,
}

/// Owned implementation. Example: successful stage transfers context; failed
/// stage leaves it with the module. Executable code outlives destroy.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Provider {
    /// Exact record extent, e.g. `size_of::<Provider>()`.
    pub bytes: usize,
    /// Private implementation shared by its offered callbacks.
    pub context: *mut c_void,
    /// Immutable declarations/tables, copied or retained on stage.
    pub interfaces: *const Interface,
    /// Interface count; e.g. zero for a pure I/O provider.
    pub interface_count: usize,
    /// Optional per-open constructor; no initial operation rights.
    pub open: *const OpenOps,
    /// Optional independent transfer directions and packet types.
    pub io: *const IoOps,
    /// Optional container operations; namespace identity grants no access.
    pub namespace_ops: *const NamespaceOps,
    /// Required quiescence callback; Busy retains context and code.
    pub stop: Option<unsafe extern "C" fn(*mut c_void) -> u32>,
    /// Required destructor, called once after obligations drain.
    pub destroy: Option<unsafe extern "C" fn(*mut c_void)>,
}

/// Scoped staging, e.g. module exports become visible only at Ready.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Registrar {
    /// Exact table extent, e.g. `size_of::<Registrar>()`.
    pub bytes: usize,
    /// Bound mount, grants and rollback owner supplied by Runtime.
    pub context: *mut c_void,
    /// Takes ownership only on OK. Relative paths cannot escape the mount;
    /// failure leaves provider with its caller.
    pub stage: Option<
        unsafe extern "C" fn(*mut c_void, Bytes, *const Provider) -> u32,
    >,
}
