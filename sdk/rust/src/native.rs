mod creation;
mod edit;
mod io;
mod module;
mod namespace;
mod opening;
mod packet;
mod registrar;
mod state;
pub use module::{ModuleImports, NativeModule, module_implementation};
pub use registrar::ExportedRegistrar;
pub use state::{import_state, import_state_init};

use alloc::{boxed::Box, rc::Rc, vec::Vec};
use core::{any::Any, cell::Cell, ffi::c_void, ptr};
use reovim_kernel_interface::{
    Error, Pending, Value, abi,
    declaration::{Declaration, Dispatch, Operation, ValueLimits},
    provider::{Creation, ProviderInterface, ResourceOps, ResourceProvider},
};

use crate::raw;

pub struct Instance {
    pub provider: abi::Provider,
    owned: Cell<bool>,
    stopped: Cell<bool>,
    code: Option<Rc<dyn Any>>,
}

impl Drop for Instance {
    fn drop(&mut self) {
        if !self.owned.get() {
            return;
        }
        if self.stopped.get() {
            // SAFETY: Import checked destroy, this instance owns context, stop
            // succeeded, and the last Rc is dropping. Code is still retained.
            unsafe { (self.provider.destroy.unwrap())(self.provider.context) };
        } else {
            // A violated/incomplete shutdown cannot unmap executable callbacks
            // still owned by native work. Runtime normally retains this owner.
            core::mem::forget(self.code.take());
        }
    }
}

struct Offered {
    context: *mut c_void,
    exec: Option<abi::ExecOps>,
    control: Option<abi::ControlOps>,
    factory: Option<abi::FactoryOps>,
    edit: Option<abi::EditOps>,
}

struct Interfaces {
    declarations: Box<[ProviderInterface<Imported>]>,
    offered: Vec<Offered>,
}

struct Imported {
    instance: Rc<Instance>,
    offered: Vec<Offered>,
    io: Option<abi::IoOps>,
    namespace: Option<abi::NamespaceOps>,
    open: Option<abi::OpenOps>,
    limits: ValueLimits,
}

/// Takes a declared native provider after checking declarations and tables.
///
/// Example: a loader supplies the descriptor and an Rc pinning its library;
/// successful import returns a safe provider for Runtime registration.
/// # Errors
/// Invalid records, unsupported families and allocation pressure leave provider
/// ownership with the supplying module. No provider callback executes here.
/// # Safety
/// Nonnull native pointers must be aligned/readable for their declared extents;
/// Callbacks obey the ABI without unwinding. `code` retains all callback,
/// table and context dependencies through destruction. The caller keeps
/// ownership/code on error and transfers them only on success.
pub unsafe fn import_provider(
    provider: *const abi::Provider,
    code: Rc<dyn Any>,
    limits: ValueLimits,
) -> Result<ResourceProvider, Error> {
    // SAFETY: The caller provides valid provider records and retained code;
    // prepare checks declarations and tables without transferring ownership.
    let (provider, instance) = unsafe { prepare(provider, code, limits)? };
    instance.owned.set(true);
    Ok(provider)
}

unsafe fn prepare(
    provider: *const abi::Provider,
    code: Rc<dyn Any>,
    limits: ValueLimits,
) -> Result<(ResourceProvider, Rc<Instance>), Error> {
    // SAFETY: Import's caller supplies the readable aligned ABI descriptor.
    // table checks its extent before copying fields; ownership stays borrowed.
    let provider = unsafe { raw::table(provider)? };
    validate(&provider)?;
    // SAFETY: The import contract keeps the immutable interface array valid;
    // elements checks its nullness, byte extent and maximum item count.
    let native = unsafe {
        raw::elements(provider.interfaces, provider.interface_count, 256)?
    };
    // SAFETY: All nested declaration/table storage remains valid under code's
    // retained library lease; interfaces copies metadata before returning.
    let Interfaces { declarations, offered } = unsafe { interfaces(native)? };
    let instance = Rc::new(Instance {
        provider,
        code: Some(code),
        owned: Cell::new(false),
        stopped: Cell::new(false),
    });
    // SAFETY: The import contract retains the nested I/O table under code;
    // optional checks its exact extent before exposing any fields.
    let io: Option<abi::IoOps> = unsafe { optional(provider.io)? };
    // SAFETY: The import contract retains readable packet Type spans nested
    // in this copied I/O table. operations validates and copies both Types.
    let operations = unsafe { io.map(|io| io::operations(io)).transpose()? };
    // SAFETY: The provider import contract retains this nested immutable
    // namespace table; optional checks its exact ABI extent before copying.
    let namespace = unsafe { optional(provider.namespace_ops)? };
    let namespace_ops = namespace.map(namespace::operations).transpose()?;
    // SAFETY: The import contract retains this immutable nested Open table;
    // optional checks its exact extent before copying any callbacks.
    let open: Option<abi::OpenOps> = unsafe { optional(provider.open)? };
    if open.is_some_and(|ops| ops.begin.is_none()) {
        return Err(Error::INVALID);
    }
    let has_open = open.is_some();
    let state = Imported {
        instance: instance.clone(),
        offered,
        io,
        namespace,
        open,
        limits,
    };
    let imported = ResourceProvider::new(
        state,
        ResourceOps {
            open: has_open.then_some(opening::begin),
            namespace: namespace_ops,
            io: operations,
            stop,
        },
        declarations,
        instance.clone(),
    )
    .map_err(|(error, _)| error)?;
    Ok((imported, instance))
}

fn validate(provider: &abi::Provider) -> Result<(), Error> {
    if provider.stop.is_none() || provider.destroy.is_none() {
        return Err(Error::INVALID);
    }
    Ok(())
}

unsafe fn interfaces(native: &[abi::Interface]) -> Result<Interfaces, Error> {
    let mut declarations = Vec::new();
    let mut offered = Vec::new();
    declarations
        .try_reserve_exact(native.len())
        .map_err(|_| Error::PRESSURE)?;
    offered.try_reserve_exact(native.len()).map_err(|_| Error::PRESSURE)?;
    for interface in native {
        // SAFETY: This reference belongs to the validated array; the caller
        // retains its ABI storage and table checks the exact record extent.
        let interface = unsafe { raw::table(interface)? };
        // SAFETY: The supplying provider retains initialized immutable bytes
        // for this declared extent; elements checks nullness and the bound.
        let bytes = unsafe {
            raw::elements(
                interface.declaration.ptr,
                interface.declaration.len,
                64 * 1024,
            )?
        };
        let declaration = Declaration::decode(bytes)?;
        // SAFETY: The interface's nested callable tables share the provider's
        // retained code lifetime; offer validates sizes and required slots.
        let entry = unsafe { offer(interface, &declaration)? };
        declarations.push(ProviderInterface {
            prepare: entry.edit.as_ref().map(|_| edit::prepare as _),
            declaration,
            begin: if entry.exec.is_some() || entry.control.is_some() {
                Some(begin)
            } else {
                None
            },
            create: entry.factory.as_ref().map(|_| create as _),
        });
        offered.push(entry);
    }
    Ok(Interfaces { declarations: declarations.into_boxed_slice(), offered })
}

unsafe fn offer(
    interface: abi::Interface,
    declaration: &Declaration,
) -> Result<Offered, Error> {
    // SAFETY: The native provider retains the immutable editing table;
    // optional validates its extent before exposing preparation callbacks.
    let edit: Option<abi::EditOps> = unsafe { optional(interface.edit)? };
    // SAFETY: A nonnull exec table satisfies the import storage contract;
    // optional checks its exact ABI extent before copying its callbacks.
    let exec: Option<abi::ExecOps> = unsafe { optional(interface.exec)? };
    // SAFETY: The control table has the same retained-storage guarantee and
    // receives its own extent check, independent of executable support.
    let control: Option<abi::ControlOps> =
        unsafe { optional(interface.control)? };
    // SAFETY: A nonnull factory table belongs to the same retained provider;
    // optional validates its exact extent before exposing the create slot.
    let factory: Option<abi::FactoryOps> =
        unsafe { optional(interface.factory)? };
    if exec.is_some_and(|table| table.exec.is_none())
        || control.is_some_and(|table| table.ioctl.is_none())
        || factory.is_some_and(|table| table.create.is_none())
        || edit.is_some_and(|table| table.prepare.is_none())
    {
        return Err(Error::INVALID);
    }
    for method in declaration.methods() {
        let exists = match (method.dispatch(), method.operation()) {
            (Dispatch::Factory, _) => factory.is_some(),
            (Dispatch::Call, Operation::Exec) => exec.is_some(),
            (Dispatch::Call, Operation::Ioctl(_)) => control.is_some(),
            (Dispatch::Edit, _) => edit.is_some(),
        };
        if !exists {
            return Err(Error::INVALID);
        }
    }
    Ok(Offered { context: interface.context, exec, control, factory, edit })
}

unsafe fn optional<T: Copy>(pointer: *const T) -> Result<Option<T>, Error> {
    if pointer.is_null() {
        Ok(None)
    } else {
        // SAFETY: The caller guarantees a readable aligned ABI table header
        // and valid fields for its advertised extent; table checks that size.
        unsafe { raw::table(pointer).map(Some) }
    }
}

fn begin(
    state: &mut Imported,
    interface: usize,
    operation: Operation,
    input: &Value,
) -> Result<Pending, Error> {
    start(state, interface, operation, input, None)
}

fn create(
    state: &mut Imported,
    interface: usize,
    operation: Operation,
    input: &Value,
    creation: &Creation,
) -> Result<Pending, Error> {
    start(state, interface, operation, input, Some(creation))
}

fn start(
    state: &Imported,
    interface: usize,
    operation: Operation,
    input: &Value,
    creation: Option<&Creation>,
) -> Result<Pending, Error> {
    if state.instance.stopped.get() {
        return Err(Error::CLOSED);
    }
    let offered = state.offered.get(interface).ok_or(Error::UNSUPPORTED)?;
    let mut refs = Vec::new();
    refs.try_reserve_exact(input.references.len())
        .map_err(|_| Error::PRESSURE)?;
    refs.extend(input.references.iter().map(|access| access.0));
    let args = abi::Args {
        data: raw::bytes(&input.data),
        refs: abi::Refs { ptr: refs.as_ptr(), len: refs.len() },
    };
    let creation = creation.map(|creation| {
        creation::Export::new(
            creation.clone(),
            state.instance.clone(),
            state.limits,
        )
    });
    let mut pending =
        abi::Pending { context: ptr::null_mut(), ops: ptr::null() };
    let mut error = abi::Error::default();
    // SAFETY: Import validated the selected callbacks and retains their code
    // and context. args/refs and both initialized outputs live through begin;
    // the native ABI requires begin to copy borrowed input before returning.
    let result = unsafe {
        invoke(
            offered,
            operation,
            offered.context,
            creation.as_ref().map(|creation| &creation.raw),
            &args,
            &mut pending,
            &mut error,
        )?
    };
    complete_begin(
        state,
        pending,
        result,
        error,
        creation.map(|owner| owner as Rc<dyn Any>),
    )
}

fn complete_begin(
    state: &Imported,
    pending: abi::Pending,
    result: u32,
    error: abi::Error,
    owner: Option<Rc<dyn Any>>,
) -> Result<Pending, Error> {
    if result != 0 && pending.context.is_null() && pending.ops.is_null() {
        return Err(raw::error(error).unwrap_or(Error::PROVIDER));
    }
    // SAFETY: A conforming begin returns owned pending storage and callbacks
    // under this instance's lease. Malformed shapes are fenced, not invoked.
    Ok(unsafe {
        crate::pending::import(
            pending,
            state.instance.clone(),
            state.limits,
            result != 0,
            owner,
        )
    })
}

unsafe fn invoke(
    offered: &Offered,
    operation: Operation,
    context: *mut c_void,
    creation: Option<&abi::Creation>,
    input: &abi::Args,
    output: &mut abi::Pending,
    error: &mut abi::Error,
) -> Result<u32, Error> {
    if let Some(creation) = creation {
        let create = offered
            .factory
            .and_then(|ops| ops.create)
            .ok_or(Error::UNSUPPORTED)?;
        let command = match operation {
            Operation::Exec => 0,
            Operation::Ioctl(command) => command,
        };
        // SAFETY: offer checked the factory slot. The retained export owns its
        // creation view through pending finish; input/output records are live
        // for this serialized call. Native create copies borrowed input.
        return Ok(unsafe {
            create(context, command, creation, input, output, error)
        });
    }
    Ok(match operation {
        Operation::Exec => {
            // SAFETY: The caller holds the live instance and borrowed argument
            // spans. offer checked this callback; outputs are valid writable
            // records for the duration of this serialized invocation.
            unsafe {
                (offered
                    .exec
                    .and_then(|table| table.exec)
                    .ok_or(Error::UNSUPPORTED)?)(
                    context, input, output, error
                )
            }
        }
        Operation::Ioctl(command) => {
            // SAFETY: The checked control callback belongs to this retained
            // instance. The caller supplies live input/output records and the
            // selected nonzero local command for a serialized invocation.
            unsafe {
                (offered
                    .control
                    .and_then(|table| table.ioctl)
                    .ok_or(Error::UNSUPPORTED)?)(
                    context, command, input, output, error,
                )
            }
        }
    })
}

fn stop(state: &mut Imported) -> Result<(), Error> {
    if state.instance.stopped.get() {
        return Ok(());
    }
    let provider = state.instance.provider;
    // SAFETY: Import checked stop and this state retains its context/code.
    // Busy keeps that owner alive; no other call can borrow this state mutably.
    let result = unsafe { (provider.stop.unwrap())(provider.context) };
    if result != 0 {
        return Err(Error::new(result));
    }
    state.instance.stopped.set(true);
    Ok(())
}
