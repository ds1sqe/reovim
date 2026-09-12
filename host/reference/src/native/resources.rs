//! Asynchronous Resource exports share one native instance and code owner.

use super::{KernelImport, Library, bytes, contract};
use alloc::{boxed::Box, format, rc::Rc, vec::Vec};
use core::{ffi::c_void, mem::MaybeUninit, ptr, slice, str};
use reovim_kernel_interface::{
    Connection, Error, Message, RequestError, ResourceId, abi,
    provider::{
        Contribution, Permissions, ProviderReply, ResourceOps,
        ResourceProvider, Surface,
    },
};

struct Instance {
    table: abi::Module,
    context: *mut c_void,
    _library: Rc<Library>,
}

impl Drop for Instance {
    fn drop(&mut self) {
        unsafe { (self.table.destroy.unwrap())(self.context) };
    }
}

struct Export {
    path: Box<str>,
    surfaces: Box<[Surface]>,
    forwarding: bool,
}

struct Endpoint {
    instance: Rc<Instance>,
    index: usize,
}

pub(super) unsafe fn load(
    table: &abi::Module,
    library: Rc<Library>,
    mount: &str,
    kernel: Option<&Connection>,
) -> Result<Vec<Contribution>, Error> {
    validate(table)?;
    // Validate and copy all views before constructing the inert context.
    let exports = unsafe { exports(table, mount)? };
    let imported = kernel.map(|kernel| {
        KernelImport(reovim_sdk::export_with_code(
            kernel.clone(),
            library.clone(),
        ))
    });
    let pointer =
        imported.as_ref().map_or(ptr::null(), |value| &raw const value.0);
    let context =
        unsafe { (table.create.unwrap())(table.declaration, pointer) };
    if context.is_null() {
        return Err(Error::PROVIDER);
    }
    let instance =
        Rc::new(Instance { table: *table, context, _library: library });
    Ok(exports
        .into_iter()
        .enumerate()
        .map(|(index, export)| contribution(&instance, index, export))
        .collect())
}

fn validate(table: &abi::Module) -> Result<(), Error> {
    if table.create.is_none()
        || table.destroy.is_none()
        || table.unload.is_none()
        || table.describe.is_none()
        || table.request.is_none()
    {
        return Err(Error::UNSUPPORTED);
    }
    if table.exports == 0 || table.exports > 256 {
        return Err(Error::INVALID);
    }
    Ok(())
}

unsafe fn exports(
    table: &abi::Module,
    mount: &str,
) -> Result<Vec<Export>, Error> {
    (0..table.exports)
        .map(|index| unsafe { describe(table, index, mount) })
        .collect()
}

unsafe fn describe(
    table: &abi::Module,
    index: usize,
    mount: &str,
) -> Result<Export, Error> {
    let mut output = MaybeUninit::uninit();
    let error = unsafe {
        (table.describe.unwrap())(table.declaration, index, output.as_mut_ptr())
    };
    if error.code != 0 {
        return Err(Error::new(error.code));
    }
    let output = unsafe { output.assume_init() };
    let suffix = str::from_utf8(unsafe { bytes(output.path)? })
        .map_err(|_| Error::INVALID)?;
    let path = if mount.is_empty() {
        suffix.into()
    } else if suffix.is_empty() {
        mount.into()
    } else {
        format!("{mount}/{suffix}").into()
    };
    let surfaces = unsafe { elements(output.surfaces, output.surface_count)? }
        .iter()
        .map(|value| unsafe { surface(value) })
        .collect::<Result<_, _>>()?;
    let forwarding = match output.forwarding {
        0 => false,
        1 => true,
        _ => return Err(Error::INVALID),
    };
    Ok(Export { path, surfaces, forwarding })
}

const unsafe fn elements<'a, T>(
    pointer: *const T,
    count: usize,
) -> Result<&'a [T], Error> {
    if count == 0 {
        return Ok(&[]);
    }
    if pointer.is_null() || count > 256 {
        return Err(Error::INVALID);
    }
    Ok(unsafe { slice::from_raw_parts(pointer, count) })
}

unsafe fn surface(value: &abi::Surface) -> Result<Surface, Error> {
    Ok(Surface {
        contract: unsafe { contract(value.contract)? },
        methods: permissions(value.methods)?,
        input: unsafe { slots(value.input, value.input_count)? },
        output: unsafe { slots(value.output, value.output_count)? },
    })
}

pub(super) unsafe fn slots(
    pointer: *const u32,
    count: usize,
) -> Result<Box<[Permissions]>, Error> {
    unsafe { elements(pointer, count)? }
        .iter()
        .map(|value| permissions(*value))
        .collect()
}

const fn permissions(value: u32) -> Result<Permissions, Error> {
    match value {
        abi::READ => Ok(Permissions::READ),
        abi::WRITE => Ok(Permissions::WRITE),
        3 => Ok(Permissions::ALL),
        _ => Err(Error::INVALID),
    }
}

fn contribution(
    instance: &Rc<Instance>,
    index: usize,
    export: Export,
) -> Contribution {
    let mut provider = ResourceProvider::new(
        Endpoint { instance: instance.clone(), index },
        ResourceOps {
            unload: Some(unload),
            read: |state, id, input| call(state, id, &input, abi::READ),
            write: |state, id, input| call(state, id, &input, abi::WRITE),
        },
        export.surfaces,
        instance.clone(),
    );
    provider.forwarding = export.forwarding;
    Contribution::Resource { path: export.path, provider }
}

fn unload(state: &mut Endpoint) -> Result<(), Error> {
    let instance = &state.instance;
    let error = unsafe { (instance.table.unload.unwrap())(instance.context) };
    if error == 0 { Ok(()) } else { Err(Error::new(error)) }
}

fn call(
    state: &Endpoint,
    subject: ResourceId,
    input: &Message,
    method: u32,
) -> ProviderReply<RequestError> {
    let references: Vec<_> = input.resources.iter().map(|id| id.0).collect();
    let request = abi::Request {
        method,
        subject: subject.0,
        contract: abi::Contract {
            name: view(input.contract.name.as_bytes()),
            version: input.contract.version,
        },
        data: view(&input.data),
        resources: references.as_ptr(),
        resource_count: references.len(),
    };
    let instance = &state.instance;
    let pending = unsafe {
        (instance.table.request.unwrap())(
            instance.context,
            state.index,
            &raw const request,
        )
    };
    unsafe { reovim_sdk::import_reply(pending, instance.clone()) }
}

const fn view(bytes: &[u8]) -> abi::Bytes {
    abi::Bytes { data: bytes.as_ptr(), len: bytes.len() }
}
