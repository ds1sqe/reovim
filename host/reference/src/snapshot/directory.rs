use alloc::{boxed::Box, format, rc::Rc, sync::Arc};
use core::sync::atomic::{AtomicBool, Ordering};
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, ResourceId,
    provider::{Permissions, ResourceOps, ResourceProvider, Surface},
};
use std::{fs::File, io, path::Path};

/// Explicit read root and same-filesystem snapshot storage grants.
pub struct Directory {
    root: Arc<File>,
    scratch: Arc<File>,
}
impl Directory {
    /// Pins both directories without following final symlinks.
    /// # Errors
    /// Returns native errors for missing, inaccessible or non-directory paths.
    pub fn open(root: &Path, scratch: &Path) -> io::Result<Self> {
        Ok(Self {
            root: Arc::new(crate::path::root(root)?),
            scratch: Arc::new(crate::path::root(scratch)?),
        })
    }

    /// Supplies capture operations under an activated module's bound namespace.
    #[must_use]
    pub fn provider(self, kernel: Connection) -> ResourceProvider {
        let state = Capture {
            directory: self,
            kernel,
            next: 0,
            busy: Arc::new(AtomicBool::new(false)),
        };
        ResourceProvider::new(
            state,
            ResourceOps {
                unload: Some(unload),
                read: |_, _, _| Box::pin(async { Err(Error::UNSUPPORTED) }),
                write: capture,
            },
            Box::new([Surface {
                contract: Contract::new("host.files.capture", 1),
                methods: Permissions::WRITE,
                input: Box::new([]),
                output: Box::new([Permissions::READ]),
            }]),
            Rc::new(()),
        )
    }
}
struct Capture {
    directory: Directory,
    kernel: Connection,
    next: u64,
    busy: Arc<AtomicBool>,
}
fn capture(
    state: &mut Capture,
    parent: ResourceId,
    input: Message,
) -> reovim_kernel_interface::provider::ProviderReply {
    let root = state.directory.root.clone();
    let scratch = state.directory.scratch.clone();
    let kernel = state.kernel.clone();
    let Some(next) = state.next.checked_add(1) else {
        return Box::pin(async { Err(Error::PRESSURE) });
    };
    state.next = next;
    let work = crate::worker::start(&state.busy, move || {
        let path =
            core::str::from_utf8(&input.data).map_err(|_| Error::INVALID)?;
        let source = crate::path::read(&root, path).map_err(|e| native(&e))?;
        super::clone(&source, &scratch).map_err(|e| native(&e))
    });
    Box::pin(async move {
        let (file, length) = work.await?;
        let provider = super::provider(file, length).owned(|| {});
        let name = format!("snapshot-{next}");
        let id = kernel
            .publish(parent, &name, provider)
            .map_err(|(error, _)| error)?;
        Ok(Message::reference(Contract::new("host.files.capture", 1), id))
    })
}
fn unload(state: &mut Capture) -> Result<(), Error> {
    if state.busy.load(Ordering::Acquire) { Err(Error::BUSY) } else { Ok(()) }
}
fn native(error: &io::Error) -> Error {
    match error.raw_os_error() {
        Some(13 | 1) => Error::DENIED,
        Some(18 | 25 | 95) => Error::UNSUPPORTED,
        Some(22 | 40 | 20) => Error::INVALID,
        Some(12 | 23 | 24 | 28) => Error::PRESSURE,
        _ if error.kind() == io::ErrorKind::InvalidInput => Error::INVALID,
        _ => Error::UNAVAILABLE,
    }
}
