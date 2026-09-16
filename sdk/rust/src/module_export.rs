use alloc::{boxed::Box, rc::Rc};
use core::ffi::c_void;
use reovim_kernel_interface::{
    Error, abi,
    caller::Caller,
    declaration::ValueLimits,
    module::{Module, ModuleImplementation},
};

use crate::{ImportedCaller, ImportedRegistrar, raw};
mod context;
mod driver;
mod entry;

struct Instance {
    module: Module,
    imports: ImportedCaller,
    exports: Option<ImportedRegistrar>,
    closed: bool,
}

/// Constructs a native instance for a Rust module's entry callback.
///
/// Example: an artifact's `rv_module_entry` descriptor points at a constructor
/// that passes its `ModuleImplementation` and granted native context here.
/// # Errors
/// Invalid admission or inert constructor rejection returns a native error.
/// Success writes output and transfers ownership of driver cleanup.
/// # Safety
/// Context and output/error pointers must be aligned and readable/writable for
/// their records. Imported callbacks remain valid through instance destruction.
/// Construction obeys `ModuleImplementation`'s inert rejection contract:
/// no native effects, outstanding delivery views or retained native context on
/// rejection. The host serializes calls, retains this artifact through all
/// dependent owners, and destroys it once after observing Closed.
pub unsafe fn export_module_instance<State: 'static>(
    context: *const abi::ModuleContext,
    output: *mut abi::ModuleInstance,
    error: *mut abi::Error,
    implementation: ModuleImplementation<State, Caller>,
    limits: ValueLimits,
    views: usize,
) -> u32 {
    if output.is_null() || error.is_null() {
        return Error::INVALID.code.0;
    }
    // SAFETY: Construction's context promise covers the checked native record
    // and its borrowed Caller/Registrar tables until instance destruction.
    let result = unsafe { construct(context, implementation, limits, views) };
    match result {
        Ok(instance) => {
            let native = abi::ModuleInstance {
                bytes: size_of::<abi::ModuleInstance>(),
                context: Box::into_raw(Box::new(instance)).cast(),
                driver: &raw const driver::OPS,
                destroy: Some(destroy),
            };
            // SAFETY: The caller supplies writable output storage. Success
            // transfers one Box owner; native destroy consumes it after Closed.
            unsafe { output.write(native) };
            0
        }
        Err(cause) => {
            // SAFETY: The caller supplies writable error storage; no instance
            // ownership or provisional output is transferred on rejection.
            unsafe { driver::failure(error, cause) }
        }
    }
}

unsafe fn construct<State: 'static>(
    pointer: *const abi::ModuleContext,
    implementation: ModuleImplementation<State, Caller>,
    limits: ValueLimits,
    views: usize,
) -> Result<Instance, Error> {
    // SAFETY: The caller retains a readable native record; table checks the
    // leading extent before copying its identities and borrowed capabilities.
    let native: abi::ModuleContext = unsafe { raw::table(pointer)? };
    let (instance, namespace, scope) = context::identities(&native)?;
    // SAFETY: The native context's borrowed capabilities remain live through
    // destruction. This adapter explicitly drains imports before reporting
    // Closed; the host retains executable code throughout that ownership.
    let imports = unsafe {
        ImportedCaller::new(native.imports, Rc::new(()), limits, views)?
    };
    // SAFETY: The same construction promise retains the scoped staging table;
    // its guard fences safe clones before this instance can be destroyed.
    let exports =
        unsafe { ImportedRegistrar::new(native.exports, Rc::new(()), limits)? };
    let context = reovim_kernel_interface::module::ModuleContext {
        instance,
        namespace,
        scope,
        imports: imports.caller(),
        exports: exports.registrar(),
    };
    let module = Module::construct(implementation, context, Rc::new(()))?;
    Ok(Instance { module, imports, exports: Some(exports), closed: false })
}

unsafe extern "C" fn destroy(pointer: *mut c_void) {
    if pointer.is_null() {
        return;
    }
    // SAFETY: Successful export transferred this unique instance. Native
    // destruction follows Closed and the last dependent owner on the executor.
    let instance = unsafe { Box::from_raw(pointer.cast::<Instance>()) };
    if !instance.closed {
        core::mem::forget(instance);
    }
}
