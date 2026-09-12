use alloc::{boxed::Box, rc::Rc, vec::Vec};
use core::{
    any::Any,
    future::Future,
    mem::MaybeUninit,
    pin::Pin,
    slice, str,
    task::{Context, Poll},
};
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, RequestError, ResourceId, abi,
    access::{Access, Method, Reply},
    namespace::ResourceEntry,
};

mod state;
mod wake;
pub use state::import_state_reply;

/// Adopts one owned ABI connection reference. `code` pins its executable owner.
///
/// # Safety
/// The connection must implement ABI 0.1.0 with valid tables, thread-safe wake
/// callbacks and the declared ownership rules. All access stays on this thread.
#[must_use]
pub unsafe fn import(raw: abi::Connection, code: Rc<dyn Any>) -> Connection {
    Connection::new(Rc::new(Imported(Rc::new(Remote { raw, code }))))
}

struct Remote {
    raw: abi::Connection,
    code: Rc<dyn Any>,
}
impl Remote {
    const fn table(&self) -> &abi::Access {
        unsafe { &*self.raw.table }
    }
}
impl Drop for Remote {
    fn drop(&mut self) {
        unsafe {
            (self.table().release)(self.raw.context);
        }
    }
}
struct Imported(Rc<Remote>);
impl Access for Imported {
    fn capture(
        &self,
        subject: ResourceId,
        rights: reovim_kernel_interface::provider::Permissions,
    ) -> Result<Connection, Error> {
        let mut output = MaybeUninit::uninit();
        check(unsafe {
            (self.0.table().capture)(
                self.0.raw.context,
                subject.0,
                rights.bits(),
                output.as_mut_ptr(),
            )
        })?;
        Ok(unsafe { import(output.assume_init(), self.0.code.clone()) })
    }
    fn withdraw(&self, resource: ResourceId) -> Result<(), Error> {
        check(unsafe {
            (self.0.table().withdraw)(self.0.raw.context, resource.0)
        })
    }

    fn publish(
        &self,
        parent: ResourceId,
        name: &str,
        provider: reovim_kernel_interface::provider::ResourceProvider,
    ) -> Result<
        ResourceId,
        (Error, reovim_kernel_interface::provider::ResourceProvider),
    > {
        let provider = crate::provider::Export::new(provider);
        let raw = provider.view();
        let mut id = 0;
        let result = check(unsafe {
            (self.0.table().publish)(
                self.0.raw.context,
                parent.0,
                view(name.as_bytes()),
                &raw const raw,
                &raw mut id,
            )
        });
        match result {
            Ok(()) => Ok(ResourceId(id)),
            Err(error) => Err((error, provider.recover())),
        }
    }
    fn root(&self) -> Result<ResourceId, Error> {
        let mut id = 0;
        check(unsafe {
            (self.0.table().root)(self.0.raw.context, &raw mut id)
        })?;
        Ok(ResourceId(id))
    }
    fn resolve_at(
        &self,
        parent: ResourceId,
        path: &str,
    ) -> Result<ResourceId, Error> {
        let mut id = 0;
        check(unsafe {
            (self.0.table().resolve)(
                self.0.raw.context,
                parent.0,
                view(path.as_bytes()),
                &raw mut id,
            )
        })?;
        Ok(ResourceId(id))
    }
    fn children(
        &self,
        parent: ResourceId,
    ) -> Result<Vec<ResourceEntry>, Error> {
        let mut listing = MaybeUninit::uninit();
        check(unsafe {
            (self.0.table().children)(
                self.0.raw.context,
                parent.0,
                listing.as_mut_ptr(),
            )
        })?;
        let listing = Listing(unsafe { listing.assume_init() });
        unsafe { elements(listing.0.entries, listing.0.count)? }
            .iter()
            .map(|entry| {
                Ok(ResourceEntry {
                    name: unsafe { text(entry.name)? }.into(),
                    resource: ResourceId(entry.resource),
                })
            })
            .collect()
    }
    fn request(
        &self,
        method: Method,
        subject: ResourceId,
        input: &Message,
    ) -> Reply {
        let references: Vec<u64> =
            input.resources.iter().map(|id| id.0).collect();
        let request = abi::Request {
            method: if method == Method::Read { abi::READ } else { abi::WRITE },
            subject: subject.0,
            contract: abi::Contract {
                name: view(input.contract.name.as_bytes()),
                version: input.contract.version,
            },
            data: view(&input.data),
            resources: references.as_ptr(),
            resource_count: references.len(),
        };
        let pending = unsafe {
            (self.0.table().request)(self.0.raw.context, &raw const request)
        };
        unsafe { import_reply(pending, self.0.clone()) }
    }
    fn close(&self, id: ResourceId) -> Result<(), Error> {
        check(unsafe { (self.0.table().close)(self.0.raw.context, id.0) })
    }
    fn reconnect(&self) -> Result<Connection, Error> {
        let mut connection = MaybeUninit::uninit();
        check(unsafe {
            (self.0.table().reconnect)(
                self.0.raw.context,
                connection.as_mut_ptr(),
            )
        })?;
        Ok(unsafe { import(connection.assume_init(), Rc::clone(&self.0.code)) })
    }
}

struct Listing(abi::Listing);
impl Drop for Listing {
    fn drop(&mut self) {
        unsafe {
            (self.0.release)(self.0.owner);
        }
    }
}
struct Waiting {
    pending: Option<abi::Pending>,
    _owner: Rc<dyn Any>,
}

/// Adopts one native pending reference and retains its code/context owner.
/// # Safety
/// The handle must obey Pending ownership, wake and no-unwind rules. Its
/// callbacks and borrowed outcome views remain valid until release.
pub unsafe fn import_reply(pending: abi::Pending, owner: Rc<dyn Any>) -> Reply {
    Box::pin(Waiting { pending: Some(pending), _owner: owner })
}
impl Waiting {
    fn release(&mut self) {
        if let Some(pending) = self.pending.take() {
            unsafe {
                (pending.release)(pending.context);
            }
        }
    }
}
impl Drop for Waiting {
    fn drop(&mut self) {
        self.release();
    }
}
impl Future for Waiting {
    type Output = Result<Message, RequestError>;
    fn poll(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Self::Output> {
        let Some(pending) = &self.pending else {
            return Poll::Ready(Err(RequestError::Rejected(Error::CLOSED)));
        };
        let wake = wake::Export::new(context.waker().clone());
        let outcome = unsafe { (pending.poll)(pending.context, wake.0) };
        if outcome.status == 0 {
            return Poll::Pending;
        }
        let result = unsafe { result(&outcome) };
        self.release(); // Copy terminal views before releasing their origin.
        Poll::Ready(result)
    }
}
fn check(error: abi::Failure) -> Result<(), Error> {
    if error.code == 0 { Ok(()) } else { Err(failure(error)) }
}
fn failure(error: abi::Failure) -> Error {
    let mut result = Error::new(error.code);
    result.id = (error.resource != 0).then_some(ResourceId(error.resource));
    result
}
const fn view(value: &[u8]) -> abi::Bytes {
    abi::Bytes { data: value.as_ptr(), len: value.len() }
}
const unsafe fn elements<'a, T>(
    pointer: *const T,
    count: usize,
) -> Result<&'a [T], Error> {
    if count == 0 {
        return Ok(&[]);
    }
    if pointer.is_null() || count > isize::MAX as usize / size_of::<T>() {
        return Err(Error::PROVIDER);
    }
    Ok(unsafe { slice::from_raw_parts(pointer, count) })
}
unsafe fn text<'a>(value: abi::Bytes) -> Result<&'a str, Error> {
    str::from_utf8(unsafe { elements(value.data, value.len)? })
        .map_err(|_| Error::PROVIDER)
}
unsafe fn result(outcome: &abi::Outcome) -> Result<Message, RequestError> {
    let decode_error = RequestError::Failed;
    match outcome.status {
        1 => Ok(Message {
            contract: Contract::new(
                unsafe { text(outcome.contract.name) }.map_err(decode_error)?,
                outcome.contract.version,
            ),
            data: unsafe { elements(outcome.data.data, outcome.data.len) }
                .map_err(decode_error)?
                .into(),
            resources: unsafe {
                elements(outcome.resources, outcome.resource_count)
            }
            .map_err(decode_error)?
            .iter()
            .copied()
            .map(ResourceId)
            .collect(),
        }),
        2 => Err(RequestError::Rejected(failure(outcome.error))),
        3 => Err(RequestError::Failed(failure(outcome.error))),
        4 => Err(RequestError::Unknown {
            cause: failure(outcome.error),
            recovery: unsafe { text(outcome.recovery) }
                .map_err(decode_error)?
                .into(),
        }),
        _ => Err(RequestError::Failed(Error::PROVIDER)),
    }
}
