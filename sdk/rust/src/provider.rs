//! Owned provider views used by native publication; Rust objects stay private.

use alloc::{boxed::Box, rc::Rc, vec::Vec};
use core::{any::Any, cell::RefCell, ffi::c_void, slice, str};
use reovim_kernel_interface::{
    Contract, Error, Message, RequestError, ResourceId, abi,
    access::Method,
    provider::{Permissions, ResourceOps, ResourceProvider, Surface},
};

pub struct Export {
    provider: Rc<RefCell<ResourceProvider>>,
    surfaces: Vec<abi::Surface>,
}
impl Export {
    pub fn new(provider: ResourceProvider) -> Rc<Self> {
        let surfaces = provider.surfaces.iter().map(surface_view).collect();
        Rc::new(Self { provider: Rc::new(RefCell::new(provider)), surfaces })
    }
    pub fn view(self: &Rc<Self>) -> abi::Provider {
        abi::Provider {
            context: Rc::as_ptr(self).cast_mut().cast(),
            table: &raw const TABLE,
            surfaces: self.surfaces.as_ptr(),
            surface_count: self.surfaces.len(),
            forwarding: u32::from(self.provider.borrow().forwarding),
            owned: u32::from(self.provider.borrow().is_owned()),
        }
    }
    pub fn recover(self: Rc<Self>) -> ResourceProvider {
        let owned = Rc::try_unwrap(self)
            .ok()
            .expect("rejection retains no native owner");
        Rc::try_unwrap(owned.provider)
            .ok()
            .expect("rejected provider is unborrowed")
            .into_inner()
    }
}
static TABLE: abi::ProviderOps = reovim_vtable::rv_vtable!(abi::ProviderOps {
    retain,
    release,
    request,
    unload,
    close,
});
unsafe extern "C" fn retain(context: *mut c_void) {
    unsafe { Rc::increment_strong_count(context.cast::<Export>()) };
}
unsafe extern "C" fn release(context: *mut c_void) {
    unsafe { Rc::decrement_strong_count(context.cast::<Export>()) };
}
unsafe extern "C" fn close(context: *mut c_void) -> abi::Failure {
    let value = unsafe { &*context.cast::<Export>() };
    failure(value.provider.borrow_mut().close())
}
unsafe extern "C" fn unload(context: *mut c_void) -> abi::Failure {
    let value = unsafe { &*context.cast::<Export>() };
    failure(value.provider.borrow_mut().unload())
}
unsafe extern "C" fn request(
    context: *mut c_void,
    input: *const abi::Request,
) -> abi::Pending {
    let provider = unsafe { &*context.cast::<Export>() }.provider.clone();
    let request = unsafe { &*input };
    let method = match request.method {
        abi::READ => Ok(Method::Read),
        abi::WRITE => Ok(Method::Write),
        _ => Err(Error::UNSUPPORTED),
    };
    let subject = ResourceId(request.subject);
    let input = unsafe { crate::decode_request(request) };
    crate::export_reply(Box::pin(async move {
        let future = provider.borrow_mut().call(
            method.map_err(RequestError::Rejected)?,
            subject,
            input.map_err(RequestError::Rejected)?,
        );
        future.await
    }))
}
fn surface_view(surface: &Surface) -> abi::Surface {
    abi::Surface {
        contract: abi::Contract {
            name: view(surface.contract.name.as_bytes()),
            version: surface.contract.version,
        },
        methods: surface.methods.bits(),
        input: surface.input.as_ptr().cast(),
        input_count: surface.input.len(),
        output: surface.output.as_ptr().cast(),
        output_count: surface.output.len(),
    }
}
struct Imported {
    raw: abi::Provider,
    _code: Rc<dyn Any>,
}
impl Drop for Imported {
    fn drop(&mut self) {
        unsafe { ((*self.raw.table).release)(self.raw.context) };
    }
}
/// Borrows a descriptor and retains it only after copying valid metadata.
pub unsafe fn import(
    raw: abi::Provider,
    code: Rc<dyn Any>,
) -> Result<ResourceProvider, Error> {
    if raw.table.is_null() {
        return Err(Error::INVALID);
    }
    let table = unsafe { &*raw.table };
    if table.version != abi::VERSION || table.size != abi::ProviderOps::ABI_SIZE
    {
        return Err(Error::UNSUPPORTED);
    }
    let surfaces = unsafe { elements(raw.surfaces, raw.surface_count)? }
        .iter()
        .map(|surface| unsafe { surface_owned(surface) })
        .collect::<Result<_, _>>()?;
    if raw.forwarding > 1 || raw.owned > 1 {
        return Err(Error::INVALID);
    }
    unsafe { (table.retain)(raw.context) };
    let imported = Rc::new(Imported { raw, _code: code });
    let mut provider = ResourceProvider::new(
        imported.clone(),
        ResourceOps {
            unload: Some(|value| {
                check(unsafe { ((*value.raw.table).unload)(value.raw.context) })
            }),
            read: |value, subject, input| {
                call(value, subject, &input, abi::READ)
            },
            write: |value, subject, input| {
                call(value, subject, &input, abi::WRITE)
            },
        },
        surfaces,
        imported.clone(),
    );
    if raw.owned != 0 {
        provider = provider.try_owned(move || cleanup(&imported));
    }
    provider.forwarding = raw.forwarding != 0;
    Ok(provider)
}
fn call(
    value: &Rc<Imported>,
    subject: ResourceId,
    input: &Message,
    method: u32,
) -> reovim_kernel_interface::access::Reply {
    let refs: Vec<_> = input.resources.iter().map(|id| id.0).collect();
    let request = abi::Request {
        method,
        subject: subject.0,
        contract: abi::Contract {
            name: view(input.contract.name.as_bytes()),
            version: input.contract.version,
        },
        data: view(&input.data),
        resources: refs.as_ptr(),
        resource_count: refs.len(),
    };
    let pending = unsafe {
        ((*value.raw.table).request)(value.raw.context, &raw const request)
    };
    unsafe { crate::import_reply(pending, value.clone()) }
}
unsafe fn surface_owned(value: &abi::Surface) -> Result<Surface, Error> {
    let name = str::from_utf8(unsafe {
        elements(value.contract.name.data, value.contract.name.len)?
    })
    .map_err(|_| Error::INVALID)?;
    Ok(Surface {
        contract: Contract::new(name, value.contract.version),
        methods: rights(value.methods)?,
        input: unsafe { elements(value.input, value.input_count)? }
            .iter()
            .map(|bits| rights(*bits))
            .collect::<Result<_, _>>()?,
        output: unsafe { elements(value.output, value.output_count)? }
            .iter()
            .map(|bits| rights(*bits))
            .collect::<Result<_, _>>()?,
    })
}
const fn rights(bits: u32) -> Result<Permissions, Error> {
    match Permissions::from_bits(bits) {
        Some(value) if !value.is_empty() => Ok(value),
        _ => Err(Error::INVALID),
    }
}
const unsafe fn elements<'a, T>(
    data: *const T,
    len: usize,
) -> Result<&'a [T], Error> {
    if len == 0 {
        return Ok(&[]);
    }
    if data.is_null() || len > 4096 {
        return Err(Error::INVALID);
    }
    Ok(unsafe { slice::from_raw_parts(data, len) })
}
const fn view(data: &[u8]) -> abi::Bytes {
    abi::Bytes { data: data.as_ptr(), len: data.len() }
}
const fn check(value: abi::Failure) -> Result<(), Error> {
    if value.code == 0 { Ok(()) } else { Err(Error::new(value.code)) }
}
fn cleanup(value: &Imported) -> Result<(), Error> {
    let failure = unsafe { ((*value.raw.table).close)(value.raw.context) };
    check(failure).map_err(|mut error| {
        error.id =
            (failure.resource != 0).then_some(ResourceId(failure.resource));
        error
    })
}
fn failure(value: Result<(), Error>) -> abi::Failure {
    value.map_or_else(
        |e| abi::Failure {
            code: e.code.0,
            resource: e.id.map_or(0, |id| id.0),
        },
        |()| abi::Failure { code: 0, resource: 0 },
    )
}
