//! Linux loader for the synchronous native provider table.

use alloc::{boxed::Box, rc::Rc, vec::Vec};
use core::{
    ffi::{c_char, c_int, c_void},
    ptr, slice, str,
};
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, ResourceId, abi,
    provider::{
        Contribution, Permissions, ProviderReply, ResourceOps,
        ResourceProvider, Surface,
    },
};
use std::{ffi::CString, os::unix::ffi::OsStrExt, path::Path};

#[link(name = "dl")]
unsafe extern "C" {
    fn dlopen(path: *const c_char, flags: c_int) -> *mut c_void;
    fn dlsym(handle: *mut c_void, name: *const c_char) -> *mut c_void;
    fn dlclose(handle: *mut c_void) -> c_int;
}

struct Library(*mut c_void);
impl Drop for Library {
    fn drop(&mut self) {
        // All contexts and callbacks retain this owner.
        unsafe {
            dlclose(self.0);
        }
    }
}

struct Instance {
    table: abi::Module,
    context: *mut c_void,
    _library: Rc<Library>,
}
impl Drop for Instance {
    fn drop(&mut self) {
        if let Some(destroy) = self.table.destroy {
            unsafe {
                destroy(self.context);
            }
        }
    }
}

/// Loads a Resource provider through the native ABI.
///
/// # Safety
/// The artifact must obey module ABI 0.1.0 ownership and no-unwind rules.
/// Register before calling; destroy only after unload readiness succeeds.
/// Library constructors execute during loading. Native code is trusted.
/// # Errors
/// Rejects loader failures, incompatible tables and malformed contracts.
pub unsafe fn load_resource(path: &Path) -> Result<ResourceProvider, Error> {
    unsafe { resource_with_imports(path, None) }
}

/// Loads a provider with separately granted Kernel access.
/// # Safety
/// Same lifetime rules as `load_resource`. The factory constructs inert state;
/// it may inspect metadata but cannot start requests before registration.
/// # Errors
/// Reports loader, table, contract or context-construction failure.
pub unsafe fn load_resource_with(
    path: &Path,
    kernel: &Connection,
) -> Result<ResourceProvider, Error> {
    unsafe { resource_with_imports(path, Some(kernel)) }
}

unsafe fn resource_with_imports(
    path: &Path,
    kernel: Option<&Connection>,
) -> Result<ResourceProvider, Error> {
    let (table, library) = unsafe { load_table(path)? };
    unsafe { resource_table(table, library, kernel) }
}

unsafe fn resource_table(
    table: abi::Module,
    library: Rc<Library>,
    kernel: Option<&Connection>,
) -> Result<ResourceProvider, Error> {
    if table.kind != abi::RESOURCE
        || table.call.is_none()
        || table.release.is_none()
        || table.destroy.is_none()
        || table.unload.is_none()
    {
        return Err(Error::UNSUPPORTED);
    }
    let read = unsafe { contract(table.read_contract)? };
    let write = unsafe { contract(table.edit_contract)? };
    let context = unsafe { create(&table, kernel, library.clone())? };
    let instance = Instance { table, context, _library: library.clone() };
    let surfaces = if read == write {
        alloc::vec![surface(read, Permissions::ALL)]
    } else {
        alloc::vec![
            surface(read, Permissions::READ),
            surface(write, Permissions::WRITE)
        ]
    };
    Ok(ResourceProvider::new(
        instance,
        ResourceOps {
            unload: Some(unload),
            read: read_call,
            write: write_call,
        },
        surfaces.into_boxed_slice(),
        library,
    ))
}

/// Loads the contribution kind declared by one native module table.
/// ```no_run
/// # fn example() -> Result<(), reovim_kernel_interface::Error> {
/// let path = std::path::Path::new("/opt/reovim/keys.so");
/// // The selected artifact is trusted and obeys the native module contract.
/// let contribution = unsafe {
///     reovim_host_reference::native::load(path, "models/keys")?
/// };
/// // Assembly hands the contribution to its module activation owner.
/// # Ok(()) }
/// ```
/// # Safety
/// The artifact must obey module ABI 0.1.0 and the ownership, no-unwind and
/// immutability requirements of its declared Resource or Model kind.
/// Register before use; retain it until unload readiness succeeds.
/// # Errors
/// Rejects unknown kinds, incompatible tables or failed native construction.
pub unsafe fn load(path: &Path, export: &str) -> Result<Contribution, Error> {
    unsafe { load_with(path, export, None) }
}

/// Loads a contribution with explicitly granted caller access.
/// # Safety
/// Same artifact/ownership requirements as [`load`]. Native code must retain
/// the caller ABI connection if it stores imports beyond construction.
/// # Errors
/// Reports loader, ABI, contract or native construction failure.
pub unsafe fn load_with(
    path: &Path,
    export: &str,
    kernel: Option<&Connection>,
) -> Result<Contribution, Error> {
    let (table, library) = unsafe { load_table(path)? };
    match table.kind {
        abi::RESOURCE => Ok(Contribution::Resource {
            path: export.into(),
            provider: unsafe { resource_table(table, library, kernel)? },
        }),
        abi::MODEL => Ok(Contribution::Model {
            path: export.into(),
            provider: unsafe { model::from_table(table, library, kernel)? },
        }),
        _ => Err(Error::UNSUPPORTED),
    }
}

fn surface(contract: Contract, methods: Permissions) -> Surface {
    Surface { contract, methods, input: Box::new([]), output: Box::new([]) }
}

const unsafe fn bytes<'a>(value: abi::Bytes) -> Result<&'a [u8], Error> {
    if value.len == 0 {
        return Ok(&[]);
    }
    if value.data.is_null() || value.len > 1024 * 1024 {
        return Err(Error::INVALID);
    }
    Ok(unsafe { slice::from_raw_parts(value.data, value.len) })
}

unsafe fn contract(value: abi::Contract) -> Result<Contract, Error> {
    let name = str::from_utf8(unsafe { bytes(value.name)? })
        .map_err(|_| Error::INVALID)?;
    if name.is_empty() || name.len() > 256 {
        return Err(Error::INVALID);
    }
    Ok(Contract::new(name, value.version))
}

fn read_call(
    state: &mut Instance,
    id: ResourceId,
    input: Message,
) -> ProviderReply {
    call(state, id, input, abi::READ)
}
fn write_call(
    state: &mut Instance,
    id: ResourceId,
    input: Message,
) -> ProviderReply {
    call(state, id, input, abi::WRITE)
}

fn call(
    state: &mut Instance,
    id: ResourceId,
    input: Message,
    method: u32,
) -> ProviderReply {
    let request = abi::Request {
        method,
        subject: id.0,
        contract: abi::Contract {
            name: abi::Bytes {
                data: input.contract.name.as_ptr(),
                len: input.contract.name.len(),
            },
            version: input.contract.version,
        },
        data: abi::Bytes { data: input.data.as_ptr(), len: input.data.len() },
        resources: ptr::null(),
        resource_count: 0,
    };
    // Invoked only by Runtime; all input views live through this call.
    let mut reply = unsafe {
        (state.table.call.unwrap())(state.context, &raw const request)
    };
    let result = if reply.error == 0 {
        unsafe { bytes(reply.data) }.map(|data| Message {
            contract: input.contract,
            data: data.into(),
            resources: Box::new([]),
        })
    } else {
        Err(Error::new(reply.error))
    };
    unsafe {
        (state.table.release.unwrap())(state.context, &raw mut reply);
    }
    Box::pin(async move { result })
}

mod model;
pub use model::load_model;

unsafe fn load_table(path: &Path) -> Result<(abi::Module, Rc<Library>), Error> {
    let library = unsafe { open(path)? };
    unsafe { module_table(library) }
}

unsafe fn open(path: &Path) -> Result<Rc<Library>, Error> {
    let path = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| Error::INVALID)?;
    let handle = unsafe { dlopen(path.as_ptr(), 2) }; // RTLD_NOW, local scope.
    if handle.is_null() {
        return Err(Error::UNAVAILABLE);
    }
    Ok(Rc::new(Library(handle)))
}

unsafe fn module_table(
    library: Rc<Library>,
) -> Result<(abi::Module, Rc<Library>), Error> {
    let symbol = unsafe { dlsym(library.0, c"rv_module_entry".as_ptr()) };
    if symbol.is_null() {
        return Err(Error::UNSUPPORTED);
    }
    let entry: unsafe extern "C" fn() -> *const abi::Module =
        unsafe { core::mem::transmute(symbol) };
    let pointer = unsafe { entry() };
    if pointer.is_null() {
        return Err(Error::INVALID);
    }
    // Read the fixed prefix before accessing the versioned table extent.
    let prefix = pointer.cast::<abi::Version>();
    let version = unsafe { prefix.read() };
    let size = unsafe { prefix.add(1).cast::<u32>().read() };
    if version != abi::VERSION || (size as usize) < size_of::<abi::Module>() {
        return Err(Error::UNSUPPORTED);
    }
    let table = unsafe { pointer.read() };
    Ok((table, library))
}

/// Loads the full contribution group through the current native table.
/// # Safety
/// The selected artifact must obey the matching native ABI's ownership,
/// single-threaded execution and no-unwind contracts. Loading executes code.
/// # Errors
/// Rejects incompatible metadata or construction before publication.
pub unsafe fn load_contributions(
    path: &Path,
    mount: &str,
    kernel: Option<&Connection>,
) -> Result<Vec<Contribution>, Error> {
    let (table, library) = unsafe { load_table(path)? };
    if table.kind == abi::RESOURCE && table.exports != 0 {
        return unsafe { resources::load(table, library, mount, kernel) };
    }
    let contribution = match table.kind {
        abi::RESOURCE => Contribution::Resource {
            path: mount.into(),
            provider: unsafe { resource_table(table, library, kernel)? },
        },
        abi::MODEL => Contribution::Model {
            path: mount.into(),
            provider: unsafe { model::from_table(table, library, kernel)? },
        },
        _ => return Err(Error::UNSUPPORTED),
    };
    Ok(alloc::vec![contribution])
}

mod resources;

struct KernelImport(abi::Connection);
impl Drop for KernelImport {
    fn drop(&mut self) {
        unsafe {
            ((*self.0.table).release)(self.0.context);
        }
    }
}
unsafe fn create(
    table: &abi::Module,
    kernel: Option<&Connection>,
    library: Rc<Library>,
) -> Result<*mut c_void, Error> {
    let imported = kernel.map(|kernel| {
        KernelImport(reovim_sdk::export_with_code(kernel.clone(), library))
    });
    let pointer =
        imported.as_ref().map_or(ptr::null(), |value| &raw const value.0);
    let context = unsafe {
        table.create.ok_or(Error::INVALID)?(table.declaration, pointer)
    };
    if context.is_null() {
        return Err(Error::PROVIDER);
    }
    Ok(context)
}
fn unload(instance: &mut Instance) -> Result<(), Error> {
    quiescent(instance)
}
fn quiescent(instance: &Instance) -> Result<(), Error> {
    let code = unsafe { (instance.table.unload.unwrap())(instance.context) };
    if code == 0 { Ok(()) } else { Err(Error::new(code)) }
}
