//! Native adapter for an open set of ordinary Resource providers.
//! Modules declare their own paths, surfaces, state and operation tables.

mod declaration;
mod model;
pub use declaration::{Definition, Export, Identity, Surface};
pub use model::ModelDefinition;

use alloc::{boxed::Box, rc::Rc, vec::Vec};
use core::{cell::RefCell, ffi::c_void, ptr};
use reovim_kernel_interface::{
    Error, Message, RequestError, ResourceId, abi,
    access::{Method, Reply},
    provider::ResourceProvider,
};

struct Instance {
    providers: Vec<RefCell<ResourceProvider>>,
}

impl Definition {
    /// Supplies the current native table for this module's declared exports.
    /// The entry point returns a pointer to a static copy of this table.
    #[must_use]
    pub const fn table(&'static self) -> abi::Module {
        const EMPTY: abi::Contract =
            abi::Contract { name: declaration::view(b""), version: 1 };
        reovim_vtable::rv_vtable!(abi::Module {
            kind: abi::RESOURCE,
            declaration: ptr::from_ref(self).cast(),
            create_contract: EMPTY,
            read_contract: EMPTY,
            edit_contract: EMPTY,
            create: Some(create),
            destroy: Some(destroy),
            call: None,
            initialize: None,
            prepare: None,
            initialize_async: None,
            prepare_async: None,
            observe: None,
            inspect: None,
            release: None,
            drop_state: None,
            unload: Some(unload),
            exports: self.exports.len(),
            describe: Some(describe),
            request: Some(request),
            identity: self.identity.view(),
            dependencies: self.dependencies.as_ptr().cast(),
            dependency_count: self.dependencies.len(),
        })
    }
}

unsafe extern "C" fn describe(
    declaration: *const c_void,
    index: usize,
    output: *mut abi::ResourceExport,
) -> abi::Failure {
    let definition = unsafe { &*declaration.cast::<Definition>() };
    let Some(export) = definition.exports.get(index) else {
        return abi::Failure { code: Error::INVALID.code.0, resource: 0 };
    };
    unsafe { output.write(export.view()) };
    abi::Failure { code: 0, resource: 0 }
}

unsafe extern "C" fn create(
    declaration: *const c_void,
    kernel: *const abi::Connection,
) -> *mut c_void {
    if kernel.is_null() {
        return ptr::null_mut();
    }
    let definition = unsafe { &*declaration.cast::<Definition>() };
    let raw = unsafe { *kernel };
    unsafe { ((*raw.table).retain)(raw.context) };
    let kernel = unsafe { crate::import(raw, Rc::new(())) };
    let providers = (definition.construct)(kernel);
    if !matches(definition, &providers) {
        return ptr::null_mut();
    }
    let instance = Instance {
        providers: providers.into_iter().map(RefCell::new).collect(),
    };
    Box::into_raw(Box::new(Rc::new(instance))).cast()
}

fn matches(definition: &Definition, providers: &[ResourceProvider]) -> bool {
    definition.exports.len() == providers.len()
        && definition.exports.iter().zip(providers).all(|(export, provider)| {
            export.forwarding == provider.forwarding
                && export.surfaces().as_ref() == provider.surfaces.as_ref()
        })
}

unsafe extern "C" fn destroy(context: *mut c_void) {
    drop(unsafe { Box::from_raw(context.cast::<Rc<Instance>>()) });
}

unsafe extern "C" fn unload(context: *mut c_void) -> u64 {
    let instance = unsafe { &*context.cast::<Rc<Instance>>() };
    if Rc::strong_count(instance) != 1 {
        return Error::BUSY.code.0;
    }
    for provider in &instance.providers {
        if let Err(error) = provider.borrow_mut().unload() {
            return error.code.0;
        }
    }
    0
}

unsafe extern "C" fn request(
    context: *mut c_void,
    index: usize,
    input: *const abi::Request,
) -> abi::Pending {
    let instance = unsafe { &*context.cast::<Rc<Instance>>() }.clone();
    let request = unsafe { &*input };
    let value = unsafe { crate::decode_request(request) };
    let future = invoke(
        instance,
        index,
        request.method,
        ResourceId(request.subject),
        value,
    );
    crate::export_reply(future)
}

fn invoke(
    instance: Rc<Instance>,
    index: usize,
    method: u32,
    subject: ResourceId,
    input: Result<Message, Error>,
) -> Reply {
    Box::pin(async move {
        let input = input.map_err(RequestError::Rejected)?;
        let method = match method {
            abi::READ => Method::Read,
            abi::WRITE => Method::Write,
            _ => return Err(RequestError::Rejected(Error::UNSUPPORTED)),
        };
        let provider = instance
            .providers
            .get(index)
            .ok_or(RequestError::Rejected(Error::INVALID))?;
        let future = admit(&mut provider.borrow_mut(), method, subject, input)
            .map_err(RequestError::Rejected)?;
        future.await
    })
}

fn admit(
    provider: &mut ResourceProvider,
    method: Method,
    subject: ResourceId,
    input: Message,
) -> Result<reovim_kernel_interface::access::Reply, Error> {
    if !provider.surfaces.iter().any(|surface| {
        surface.contract == input.contract && surface.methods.allows(method)
    }) {
        return Err(Error::UNSUPPORTED);
    }
    Ok(provider.call(method, subject, input))
}
