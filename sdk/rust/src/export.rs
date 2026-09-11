use alloc::{boxed::Box, rc::Rc, vec::Vec};
use core::{
    ffi::c_void,
    ptr, slice, str,
    task::{Context, Poll},
};
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, RequestError, ResourceId, abi,
    access::Reply,
};

mod wake;

/// Transfers one owned connection reference to the ABI caller.
/// Release through the returned table. Calls are single-threaded.
#[must_use]
pub fn export(connection: Connection) -> abi::Connection {
    abi::Connection {
        context: Rc::into_raw(Rc::new(connection)).cast_mut().cast(),
        table: &raw const TABLE,
    }
}

const _: () = assert!(size_of::<abi::Access>() <= u32::MAX as usize);

// The ABI extent is checked above at compile time.
#[allow(clippy::cast_possible_truncation)]
static TABLE: abi::Access = abi::Access {
    version: abi::VERSION,
    size: size_of::<abi::Access>() as u32,
    retain,
    release,
    root,
    resolve,
    children,
    request,
    close,
    reconnect,
};

unsafe extern "C" fn retain(context: *mut c_void) {
    unsafe {
        Rc::increment_strong_count(context.cast::<Connection>());
    }
}
unsafe extern "C" fn release(context: *mut c_void) {
    unsafe {
        Rc::decrement_strong_count(context.cast::<Connection>());
    }
}

fn failure(error: Error) -> abi::Failure {
    abi::Failure { code: error.code.0, resource: error.id.map_or(0, |id| id.0) }
}
const SUCCESS: abi::Failure = abi::Failure { code: 0, resource: 0 };

fn rejected(error: &RequestError) -> abi::Failure {
    match error {
        RequestError::Rejected(error) | RequestError::Failed(error) => {
            failure(*error)
        }
        RequestError::Unknown { cause, .. } => failure(*cause),
    }
}

unsafe fn return_id(
    result: Result<ResourceId, RequestError>,
    output: *mut u64,
) -> abi::Failure {
    match result {
        Ok(id) => {
            unsafe {
                output.write(id.0);
            }
            SUCCESS
        }
        Err(error) => rejected(&error),
    }
}

unsafe extern "C" fn root(
    context: *mut c_void,
    output: *mut u64,
) -> abi::Failure {
    let connection = unsafe { &*context.cast::<Connection>() };
    unsafe { return_id(connection.root().into_inner(), output) }
}

const unsafe fn bytes<'a>(value: abi::Bytes) -> Result<&'a [u8], Error> {
    if value.len == 0 {
        return Ok(&[]);
    }
    if value.data.is_null() || value.len > isize::MAX as usize {
        return Err(Error::INVALID);
    }
    Ok(unsafe { slice::from_raw_parts(value.data, value.len) })
}

unsafe extern "C" fn resolve(
    context: *mut c_void,
    parent: u64,
    path: abi::Bytes,
    output: *mut u64,
) -> abi::Failure {
    let path = unsafe { bytes(path) }
        .and_then(|value| str::from_utf8(value).map_err(|_| Error::INVALID));
    let path = match path {
        Ok(path) => path,
        Err(error) => return failure(error),
    };
    let connection = unsafe { &*context.cast::<Connection>() };
    unsafe {
        return_id(
            connection.resolve_at(ResourceId(parent), path).into_inner(),
            output,
        )
    }
}

struct Listing {
    entries: Vec<abi::Entry>,
    names: Vec<Box<str>>,
}
unsafe extern "C" fn release_listing(context: *mut c_void) {
    drop(unsafe { Box::from_raw(context.cast::<Listing>()) });
}
unsafe extern "C" fn children(
    context: *mut c_void,
    parent: u64,
    output: *mut abi::Listing,
) -> abi::Failure {
    let connection = unsafe { &*context.cast::<Connection>() };
    let entries = match connection.children(ResourceId(parent)).into_inner() {
        Ok(entries) => entries,
        Err(error) => return rejected(&error),
    };
    let mut listing =
        Box::new(Listing { entries: Vec::new(), names: Vec::new() });
    for entry in entries {
        listing.entries.push(abi::Entry {
            name: view(entry.name.as_bytes()),
            resource: entry.resource.0,
        });
        listing.names.push(entry.name);
    }
    let result = abi::Listing {
        entries: listing.entries.as_ptr(),
        count: listing.entries.len(),
        owner: Box::into_raw(listing).cast(),
        release: release_listing,
    };
    unsafe {
        output.write(result);
    }
    SUCCESS
}

unsafe extern "C" fn close(context: *mut c_void, id: u64) -> abi::Failure {
    let connection = unsafe { &*context.cast::<Connection>() };
    connection
        .close(ResourceId(id))
        .into_inner()
        .map_or_else(|error| rejected(&error), |()| SUCCESS)
}
unsafe extern "C" fn reconnect(
    context: *mut c_void,
    output: *mut abi::Connection,
) -> abi::Failure {
    let connection = unsafe { &*context.cast::<Connection>() };
    match connection.reconnect() {
        Ok(connection) => {
            unsafe {
                output.write(export(connection));
            }
            SUCCESS
        }
        Err(error) => failure(error),
    }
}

struct Pending {
    future: Reply,
    result: Option<Result<Message, RequestError>>,
    references: Vec<u64>,
}
unsafe fn message(request: &abi::Request) -> Result<Message, Error> {
    let name = str::from_utf8(unsafe { bytes(request.contract.name)? })
        .map_err(|_| Error::INVALID)?;
    let data = unsafe { bytes(request.data)? };
    if request.resource_count > isize::MAX as usize / size_of::<u64>()
        || (request.resource_count != 0 && request.resources.is_null())
    {
        return Err(Error::INVALID);
    }
    let ids = if request.resource_count == 0 {
        &[]
    } else {
        unsafe {
            slice::from_raw_parts(request.resources, request.resource_count)
        }
    };
    Ok(Message {
        contract: Contract::new(name, request.contract.version),
        data: data.into(),
        resources: ids.iter().copied().map(ResourceId).collect(),
    })
}
unsafe extern "C" fn request(
    context: *mut c_void,
    input: *const abi::Request,
) -> abi::Pending {
    let connection = unsafe { &*context.cast::<Connection>() };
    let input = unsafe { &*input };
    let future = match unsafe { message(input) } {
        Ok(message) if input.method == abi::READ => {
            connection.read(ResourceId(input.subject), &message)
        }
        Ok(message) if input.method == abi::WRITE => {
            connection.write(ResourceId(input.subject), &message)
        }
        _ => Box::pin(core::future::ready(Err(RequestError::Rejected(
            Error::INVALID,
        )))),
    };
    abi::Pending {
        context: Box::into_raw(Box::new(Pending {
            future,
            result: None,
            references: Vec::new(),
        }))
        .cast(),
        poll,
        release: release_pending,
    }
}
unsafe extern "C" fn release_pending(context: *mut c_void) {
    drop(unsafe { Box::from_raw(context.cast::<Pending>()) });
}
unsafe extern "C" fn poll(
    context: *mut c_void,
    wake: abi::Wake,
) -> abi::Outcome {
    let pending = unsafe { &mut *context.cast::<Pending>() };
    if pending.result.is_none() {
        let waker = unsafe { wake::import(wake) };
        if let Poll::Ready(result) =
            pending.future.as_mut().poll(&mut Context::from_waker(&waker))
        {
            pending.references = result.as_ref().map_or_else(
                |_| Vec::new(),
                |message| message.resources.iter().map(|id| id.0).collect(),
            );
            pending.result = Some(result);
        }
    }
    outcome(pending)
}
const fn view(bytes: &[u8]) -> abi::Bytes {
    abi::Bytes { data: bytes.as_ptr(), len: bytes.len() }
}
fn outcome(pending: &Pending) -> abi::Outcome {
    let mut output = abi::Outcome {
        status: 0,
        error: SUCCESS,
        contract: abi::Contract { name: view(&[]), version: 0 },
        data: view(&[]),
        resources: ptr::null(),
        resource_count: 0,
        recovery: view(&[]),
    };
    match &pending.result {
        None => {}
        Some(Ok(message)) => {
            output.status = 1;
            output.contract = abi::Contract {
                name: view(message.contract.name.as_bytes()),
                version: message.contract.version,
            };
            output.data = view(&message.data);
            output.resources = pending.references.as_ptr();
            output.resource_count = pending.references.len();
        }
        Some(Err(error)) => set_error(&mut output, error),
    }
    output
}
fn set_error(output: &mut abi::Outcome, error: &RequestError) {
    let (status, error) = match error {
        RequestError::Rejected(error) => (2, *error),
        RequestError::Failed(error) => (3, *error),
        RequestError::Unknown { cause, recovery } => {
            output.recovery = view(recovery.as_bytes());
            (4, *cause)
        }
    };
    output.status = status;
    output.error = failure(error);
}
