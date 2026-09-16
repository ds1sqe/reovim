use alloc::rc::Rc;
use core::{cell::Cell, ptr, task::Context};
use reovim_kernel_interface::{
    Error, abi,
    driver::{DriverOps, Lifecycle, Progress},
    module::{ModuleContext, ModuleImplementation, ModuleScope},
};

use super::{ModuleImports, NativeModule};
use crate::{ExportedCaller, ExportedRegistrar, pending::wake::Export, raw};

struct Instance {
    raw: Cell<Option<abi::ModuleInstance>>,
    driver: Cell<Option<abi::ModuleDriver>>,
    stopped: Cell<bool>,
    environment: Option<Environment>,
}

struct Environment {
    _code: NativeModule,
    caller: ExportedCaller,
    registrar: ExportedRegistrar,
}

impl Drop for Instance {
    fn drop(&mut self) {
        let Some(instance) = self.raw.get() else { return };
        if !self.stopped.get() {
            // Runtime retains unfinished modules; this protects code if its
            // unsafe producer nevertheless breaks the cleanup contract.
            core::mem::forget(self.environment.take());
            return;
        }
        // SAFETY: Admission checked destroy, closing succeeded, and this final
        // Rc follows every provider/result lease. Imported contexts and code
        // are still fields of self and remain alive throughout destruction.
        unsafe { instance.destroy.unwrap()(instance.context) };
    }
}

pub(super) struct State {
    instance: Rc<Instance>,
}

pub(super) fn implementation() -> ModuleImplementation<State, ModuleImports> {
    ModuleImplementation {
        construct,
        driver: DriverOps { status, advance, close },
    }
}

#[allow(
    clippy::result_large_err,
    reason = "inert rejection returns context without allocating"
)]
fn construct(
    context: ModuleContext<ModuleImports>,
) -> Result<State, (ModuleContext<ModuleImports>, Error)> {
    if !valid_identity(context.instance)
        || !valid_identity(context.namespace)
        || !valid_scope(context.scope)
    {
        return Err((context, Error::INVALID));
    }
    let scope_bit = match context.scope {
        ModuleScope::Global => 1,
        ModuleScope::Session(_) => 2,
    };
    if context.imports.module.declaration().scopes() & scope_bit == 0 {
        return Err((context, Error::UNSUPPORTED));
    }
    let caller = match ExportedCaller::new(
        context.imports.caller.clone(),
        context.imports.limits,
        context.imports.views,
    ) {
        Ok(caller) => caller,
        Err(error) => return Err((context, error)),
    };
    let instance =
        Rc::new_cyclic(|weak: &alloc::rc::Weak<Instance>| Instance {
            raw: Cell::new(None),
            driver: Cell::new(None),
            stopped: Cell::new(false),
            environment: Some(Environment {
                _code: context.imports.module.clone(),
                caller,
                registrar: ExportedRegistrar::borrowed(
                    context.exports.clone(),
                    weak.clone(),
                    context.imports.limits,
                ),
            }),
        });
    let (scope, session) = match context.scope {
        ModuleScope::Global => (0, abi::Identity::default()),
        ModuleScope::Session(id) => (1, identity(id)),
    };
    let native = abi::ModuleContext {
        bytes: size_of::<abi::ModuleContext>(),
        instance: identity(context.instance),
        scope,
        session,
        namespace_resource: identity(context.namespace),
        imports: instance.environment.as_ref().unwrap().caller.raw(),
        exports: instance.environment.as_ref().unwrap().registrar.raw(),
    };
    let mut output = abi::ModuleInstance {
        bytes: 0,
        context: ptr::null_mut(),
        driver: ptr::null(),
        destroy: None,
    };
    let mut error = abi::Error::default();
    let callback = context.imports.module.definition.raw.construct.unwrap();
    // SAFETY: The admitted artifact retains its constructor. Context and output
    // are aligned live records; copied imports/exports borrow Instance fields,
    // which remain retained through native destruction and dependent owners.
    let result =
        unsafe { callback(&raw const native, &raw mut output, &raw mut error) };
    if result != 0 && empty(output) {
        return Err((context, raw::error(error).unwrap_or(Error::PROVIDER)));
    }
    if result != 0 {
        core::mem::forget(instance);
        return Err((context, Error::PROVIDER));
    }
    // SAFETY: Successful construction promises a complete readable instance
    // and driver table. Validation copies the table under the artifact lease.
    let driver = unsafe { validate(&output) };
    if driver.is_err() {
        // Unexpected ownership cannot be safely guessed away. Preserve native
        // context, imported callbacks and code when its table is malformed.
        core::mem::forget(instance);
        return Err((context, Error::PROVIDER));
    }
    instance.raw.set(Some(output));
    instance.driver.set(driver.ok());
    Ok(State { instance })
}

const fn empty(instance: abi::ModuleInstance) -> bool {
    instance.bytes == 0
        && instance.context.is_null()
        && instance.driver.is_null()
        && instance.destroy.is_none()
}

unsafe fn validate(
    instance: &abi::ModuleInstance,
) -> Result<abi::ModuleDriver, Error> {
    if instance.bytes != size_of::<abi::ModuleInstance>()
        || instance.destroy.is_none()
    {
        return Err(Error::INVALID);
    }
    // SAFETY: The constructor promises a retained readable immutable driver;
    // table verifies its extent before required callbacks are checked.
    let driver = unsafe { raw::table(instance.driver)? };
    if driver.status.is_none()
        || driver.advance.is_none()
        || driver.close.is_none()
    {
        return Err(Error::INVALID);
    }
    Ok(driver)
}

fn valid_scope(scope: ModuleScope) -> bool {
    match scope {
        ModuleScope::Global => true,
        ModuleScope::Session(id) => valid_identity(id),
    }
}

fn valid_identity(id: reovim_kernel_interface::ResourceId) -> bool {
    id.serial != 0 && id.epoch != [0; 16]
}

const fn identity(id: reovim_kernel_interface::ResourceId) -> abi::Identity {
    abi::Identity { epoch: id.epoch, serial: id.serial }
}

fn status(state: &State) -> Lifecycle {
    observe(&state.instance).unwrap_or_else(Lifecycle::Failed)
}

fn observe(owner: &Instance) -> Result<Lifecycle, Error> {
    let instance = owner.raw.get().ok_or(Error::PROVIDER)?;
    let driver = owner.driver.get().ok_or(Error::PROVIDER)?;
    let mut status =
        abi::ModuleStatus { phase: u32::MAX, failure: abi::Error::default() };
    // SAFETY: Admission validated the table; owner keeps context and code live.
    // This passive callback receives initialized writable status storage.
    let result =
        unsafe { driver.status.unwrap()(instance.context, &raw mut status) };
    if result != 0 {
        return Err(Error::new(result));
    }
    if status.phase == 5 {
        return raw::error(status.failure).map(Lifecycle::Failed);
    }
    if status.failure != abi::Error::default() {
        return Err(Error::PROVIDER);
    }
    match status.phase {
        0 => Ok(Lifecycle::Constructed),
        1 => Ok(Lifecycle::Starting),
        2 => Ok(Lifecycle::Ready),
        3 => Ok(Lifecycle::Closing),
        4 => Ok(Lifecycle::Closed),
        _ => Err(Error::PROVIDER),
    }
}

fn advance(state: &mut State, cx: &mut Context<'_>) -> Result<Progress, Error> {
    let owner = &state.instance;
    let instance = owner.raw.get().ok_or(Error::PROVIDER)?;
    let driver = owner.driver.get().ok_or(Error::PROVIDER)?;
    let wake = Export::new(cx.waker().clone());
    let mut progress = u32::MAX;
    let mut error = abi::Error::default();
    // SAFETY: owner retains validated driver/context. Wake has a live retained
    // owner; native code must retain before keeping it beyond this callback.
    // Initialized outputs remain writable until the serialized call returns.
    let result = unsafe {
        driver.advance.unwrap()(
            instance.context,
            wake.0,
            &raw mut progress,
            &raw mut error,
        )
    };
    if result != 0 {
        return Err(raw::error(error).unwrap_or(Error::PROVIDER));
    }
    let closed = observe(owner)? == Lifecycle::Closed;
    let progress = match progress {
        0 => Progress::More,
        1 => Progress::Idle,
        2 if closed => Progress::Stopped,
        _ => return Err(Error::PROVIDER),
    };
    owner.stopped.set(closed);
    Ok(progress)
}

fn close(state: &mut State) -> Result<(), Error> {
    let owner = &state.instance;
    if owner.stopped.get() {
        return Ok(());
    }
    let instance = owner.raw.get().ok_or(Error::PROVIDER)?;
    let driver = owner.driver.get().ok_or(Error::PROVIDER)?;
    let mut error = abi::Error::default();
    // SAFETY: owner retains context/code and validated close callback. Error is
    // initialized writable storage; success alone does not destroy the owner.
    let result =
        unsafe { driver.close.unwrap()(instance.context, &raw mut error) };
    if result != 0 {
        return Err(raw::error(error).unwrap_or(Error::PROVIDER));
    }
    if observe(owner)? == Lifecycle::Closed {
        owner.stopped.set(true);
    }
    Ok(())
}
