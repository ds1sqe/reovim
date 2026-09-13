use alloc::{boxed::Box, format, rc::Rc, sync::Arc};
use core::sync::atomic::{AtomicBool, Ordering};
use reovim_kernel_interface::{
    Connection, Contract, Error, Message, ResourceId,
    provider::{Permissions, ResourceOps, ResourceProvider, Surface},
};
use std::{fs::File, io, path::Path};

/// Explicit read root and coherent snapshot storage grants.
pub struct Directory {
    pub(super) root: Arc<File>,
    pub(super) scratch: Arc<File>,
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

    /// Supplies file operations under an activated module's bound namespace.
    ///
    /// `host.files.capture` v1 returns a caller-owned anonymous snapshot.
    /// `host.files.select` v1 selects and prepares a shared file source. Both
    /// take relative UTF-8 path data and return one READ reference. Selection
    /// reuses the same pinned file within this directory provider, including
    /// hard links; pathname replacement selects a different source.
    /// Selection response data names its Resource child for explicit release.
    ///
    /// Shared subjects stay installed until module unload or an explicit
    /// `host.files.release` v1 WRITE naming the Resource child in UTF-8 data
    /// with no references. Release rejects captured or in-flight use. A
    /// caller must capture selected access before relying on its lifetime.
    /// Runtime resource limits bound installed selections.
    /// No selection is evicted automatically.
    #[must_use]
    pub fn provider(self, kernel: Connection) -> ResourceProvider {
        let state = Capture {
            directory: self,
            kernel,
            next: 0,
            busy: Arc::new(AtomicBool::new(false)),
            selection: super::selection::Selection::default(),
        };
        ResourceProvider::new(
            state,
            ResourceOps {
                unload: Some(unload),
                read: |_, _, _| Box::pin(async { Err(Error::UNSUPPORTED) }),
                write,
            },
            Box::new([
                surface("host.files.capture"),
                surface("host.files.select"),
                Surface {
                    contract: Contract::new("host.files.release", 1),
                    methods: Permissions::WRITE,
                    input: Box::new([]),
                    output: Box::new([]),
                },
            ]),
            Rc::new(()),
        )
    }
}
struct Capture {
    directory: Directory,
    kernel: Connection,
    next: u64,
    busy: Arc<AtomicBool>,
    selection: super::selection::Selection,
}
fn surface(name: &str) -> Surface {
    Surface {
        contract: Contract::new(name, 1),
        methods: Permissions::WRITE,
        input: Box::new([]),
        output: Box::new([Permissions::READ]),
    }
}

fn write(
    state: &mut Capture,
    parent: ResourceId,
    input: Message,
) -> reovim_kernel_interface::provider::ProviderReply {
    if input.contract.name.as_ref() == "host.files.release" {
        let result = state.selection.release(&state.kernel, input);
        return Box::pin(core::future::ready(result));
    }
    let Some(next) = state.next.checked_add(1) else {
        return Box::pin(async { Err(Error::PRESSURE) });
    };
    state.next = next;
    if input.contract.name.as_ref() == "host.files.select" {
        return super::selection::select(
            &state.directory,
            state.selection.clone(),
            state.kernel.clone(),
            parent,
            next,
            &state.busy,
            input,
        );
    }
    capture(state, parent, input, next)
}

fn capture(
    state: &Capture,
    parent: ResourceId,
    input: Message,
    next: u64,
) -> reovim_kernel_interface::provider::ProviderReply {
    let root = state.directory.root.clone();
    let scratch = state.directory.scratch.clone();
    let kernel = state.kernel.clone();
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
pub(super) fn native(error: &io::Error) -> Error {
    match error.raw_os_error() {
        Some(13 | 1) => Error::DENIED,
        Some(11) => Error::BUSY,
        Some(116) => Error::STALE,
        Some(18 | 25 | 95) => Error::UNSUPPORTED,
        Some(22 | 40 | 20) => Error::INVALID,
        Some(12 | 23 | 24 | 28) => Error::PRESSURE,
        _ if error.kind() == io::ErrorKind::InvalidInput => Error::INVALID,
        _ => Error::UNAVAILABLE,
    }
}
