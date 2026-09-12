//! Safe immutable Model declarations with optional deferred state inspection.

use super::Identity;
use alloc::{boxed::Box, rc::Rc};
use core::{ffi::c_void, ptr};
use reovim_kernel_interface::{
    Connection, Error, Message, abi,
    buffer::{State, StateReply},
    provider::Permissions,
};

pub struct ModelDefinition {
    pub identity: Identity,
    pub dependencies: &'static [Identity],
    pub initial: Identity,
    /// Borrowed Resource slots; capture access before retaining it in State.
    pub initial_input: &'static [Permissions],
    pub references: Option<Permissions>,
    pub edit: Identity,
    pub read: Identity,
    pub initialize: fn(Message) -> Result<State, Error>,
    pub prepare: fn(&State, Message) -> Result<State, Error>,
    /// Receives the optional composition connection, never implicit authority.
    pub initialize_async: Option<fn(Option<Connection>, Message) -> StateReply>,
    pub prepare_async: Option<fn(State, Message) -> StateReply>,
    pub observe: fn(&State, Message) -> Result<Message, Error>,
    /// Deferred immutable reads; None reuses the finite observation function.
    pub inspect: Option<
        fn(State, Message) -> reovim_kernel_interface::provider::ProviderReply,
    >,
}

impl ModelDefinition {
    /// Exports preparation and owned immutable states over C.
    #[must_use]
    pub const fn table(&'static self) -> abi::Module {
        reovim_vtable::rv_vtable!(abi::Module {
            kind: abi::MODEL,
            declaration: ptr::from_ref(self).cast(),
            create_contract: self.initial.view(),
            initial_input: self.initial_input.as_ptr().cast(),
            initial_input_count: self.initial_input.len(),
            read_contract: self.read.view(),
            edit_contract: self.edit.view(),
            create: Some(create),
            destroy: Some(destroy),
            call: None,
            initialize: Some(initialize),
            prepare: Some(prepare),
            initialize_async: match self.initialize_async {
                Some(_) => Some(initialize_async),
                None => None,
            },
            prepare_async: match self.prepare_async {
                Some(_) => Some(prepare_async),
                None => None,
            },
            observe: Some(observe),
            inspect: Some(inspect),
            release: Some(release),
            drop_state: Some(drop_state),
            unload: Some(unload),
            exports: 0,
            describe: None,
            request: None,
            identity: self.identity.view(),
            dependencies: self.dependencies.as_ptr().cast(),
            dependency_count: self.dependencies.len(),
            references: match self.references {
                Some(rights) => rights.bits(),
                None => 0,
            },
        })
    }
}

struct Instance {
    definition: &'static ModelDefinition,
    kernel: Option<Connection>,
}

unsafe extern "C" fn create(
    declaration: *const c_void,
    kernel: *const abi::Connection,
) -> *mut c_void {
    let definition = unsafe { &*declaration.cast::<ModelDefinition>() };
    let kernel = if kernel.is_null() {
        None
    } else {
        let raw = unsafe { *kernel };
        unsafe { ((*raw.table).retain)(raw.context) };
        Some(unsafe { crate::import(raw, Rc::new(())) })
    };
    Box::into_raw(Box::new(Instance { definition, kernel })).cast()
}
unsafe extern "C" fn destroy(context: *mut c_void) {
    drop(unsafe { Box::from_raw(context.cast::<Instance>()) });
}
const unsafe extern "C" fn unload(_: *mut c_void) -> u64 {
    0
}

unsafe extern "C" fn initialize(
    context: *mut c_void,
    input: *const abi::Request,
) -> abi::Reply {
    let definition = unsafe { &*context.cast::<Instance>() }.definition;
    let input = unsafe { crate::decode_request(&*input) };
    candidate(input.and_then(|input| {
        validate_initial(definition, &input)?;
        (definition.initialize)(input)
    }))
}
unsafe extern "C" fn prepare(
    context: *mut c_void,
    state: *const c_void,
    input: *const abi::Request,
) -> abi::Reply {
    let definition = unsafe { &*context.cast::<Instance>() }.definition;
    let state = unsafe { &*state.cast::<State>() };
    let input = unsafe { crate::decode_request(&*input) };
    candidate(input.and_then(|input| {
        validate_value(&definition.edit, &input, definition.references)?;
        (definition.prepare)(state, input)
    }))
}
unsafe extern "C" fn observe(
    context: *mut c_void,
    state: *const c_void,
    input: *const abi::Request,
) -> abi::Reply {
    let definition = unsafe { &*context.cast::<Instance>() }.definition;
    let state = unsafe { &*state.cast::<State>() };
    let input = unsafe { crate::decode_request(&*input) };
    observation(
        input.and_then(|input| {
            validate(&definition.read, &input)?;
            (definition.observe)(state, input)
        }),
        definition,
    )
}

unsafe extern "C" fn initialize_async(
    context: *mut c_void,
    input: *const abi::Request,
) -> abi::StatePending {
    let instance = unsafe { &*context.cast::<Instance>() };
    let definition = instance.definition;
    let kernel = instance.kernel.clone();
    let input = unsafe { crate::decode_request(&*input) };
    crate::export_state_reply(Box::pin(async move {
        let input = input?;
        validate_initial(definition, &input)?;
        match definition.initialize_async {
            Some(initialize) => initialize(kernel, input).await,
            None => (definition.initialize)(input),
        }
    }))
}

unsafe extern "C" fn prepare_async(
    context: *mut c_void,
    state: *const c_void,
    input: *const abi::Request,
) -> abi::StatePending {
    let definition = unsafe { &*context.cast::<Instance>() }.definition;
    let state = unsafe { &*state.cast::<State>() }.clone();
    let input = unsafe { crate::decode_request(&*input) };
    crate::export_state_reply(Box::pin(async move {
        let input = input?;
        validate_value(&definition.edit, &input, definition.references)?;
        match definition.prepare_async {
            Some(prepare) => prepare(state, input).await,
            None => (definition.prepare)(&state, input),
        }
    }))
}

unsafe extern "C" fn inspect(
    context: *mut c_void,
    state: *const c_void,
    input: *const abi::Request,
) -> abi::Pending {
    let definition = unsafe { &*context.cast::<Instance>() }.definition;
    let state = unsafe { &*state.cast::<State>() }.clone();
    let input = unsafe { crate::decode_request(&*input) };
    crate::export_reply(Box::pin(async move {
        let input = input?;
        validate(&definition.read, &input)?;
        let output = match definition.inspect {
            Some(inspect) => inspect(state, input).await?,
            None => (definition.observe)(&state, input)?,
        };
        validate_value(&definition.read, &output, definition.references)
            .map_err(|_| Error::PROVIDER)?;
        Ok(output)
    }))
}
fn candidate(result: Result<State, Error>) -> abi::Reply {
    match result {
        Ok(state) => abi::Reply {
            error: 0,
            data: empty(),
            state_bytes: state.cost,
            state: Box::into_raw(Box::new(state)).cast(),
            resources: ptr::null(),
            resource_count: 0,
        },
        Err(error) => failure(error),
    }
}
fn observation(
    result: Result<Message, Error>,
    definition: &ModelDefinition,
) -> abi::Reply {
    let message = match result {
        Ok(message) => message,
        Err(error) => return failure(error),
    };
    if validate_value(&definition.read, &message, definition.references)
        .is_err()
    {
        return failure(Error::PROVIDER);
    }
    let len = message.data.len();
    let pointer = Box::into_raw(message.data).cast::<u8>();
    abi::Reply {
        error: 0,
        data: abi::Bytes { data: pointer, len },
        state: ptr::null_mut(),
        state_bytes: 0,
        resource_count: message.resources.len(),
        resources: Box::into_raw(message.resources).cast(),
    }
}
const fn failure(error: Error) -> abi::Reply {
    abi::Reply {
        error: error.code.0,
        data: empty(),
        state: ptr::null_mut(),
        state_bytes: 0,
        resources: ptr::null(),
        resource_count: 0,
    }
}
const fn empty() -> abi::Bytes {
    abi::Bytes { data: ptr::null(), len: 0 }
}
fn validate(contract: &Identity, input: &Message) -> Result<(), Error> {
    validate_value(contract, input, None)
}
fn validate_value(
    contract: &Identity,
    input: &Message,
    references: Option<Permissions>,
) -> Result<(), Error> {
    if !contract.matches(&input.contract) {
        return Err(Error::UNSUPPORTED);
    }
    if references.is_none() && !input.resources.is_empty() {
        return Err(Error::INVALID);
    }
    Ok(())
}
fn validate_initial(
    definition: &ModelDefinition,
    input: &Message,
) -> Result<(), Error> {
    if !definition.initial.matches(&input.contract) {
        return Err(Error::UNSUPPORTED);
    }
    if definition.references.is_none()
        && input.resources.len() != definition.initial_input.len()
    {
        return Err(Error::INVALID);
    }
    Ok(())
}
unsafe extern "C" fn release(_: *mut c_void, reply: *mut abi::Reply) {
    let reply = unsafe { &mut *reply };
    if !reply.resources.is_null() {
        let resources = ptr::slice_from_raw_parts_mut(
            reply.resources.cast_mut(),
            reply.resource_count,
        );
        drop(unsafe { Box::from_raw(resources) });
        reply.resources = ptr::null();
        reply.resource_count = 0;
    }
    if !reply.data.data.is_null() {
        let bytes = ptr::slice_from_raw_parts_mut(
            reply.data.data.cast_mut(),
            reply.data.len,
        );
        drop(unsafe { Box::from_raw(bytes) });
        reply.data = empty();
    }
    if !reply.state.is_null() {
        drop(unsafe { Box::from_raw(reply.state.cast::<State>()) });
        reply.state = ptr::null_mut();
    }
}
unsafe extern "C" fn drop_state(_: *mut c_void, state: *mut c_void) {
    drop(unsafe { Box::from_raw(state.cast::<State>()) });
}
