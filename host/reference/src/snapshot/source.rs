use alloc::{boxed::Box, rc::Rc, sync::Arc};
use core::{cell::RefCell, sync::atomic::AtomicBool};
use reovim_kernel_interface::{
    Error, Message,
    provider::{ProviderReply, ResourceOps, ResourceProvider},
};
use std::{
    fs::File, fs::OpenOptions, io, os::unix::fs::OpenOptionsExt, path::Path,
};

struct Prepared {
    file: Arc<File>,
    length: u64,
    busy: Arc<AtomicBool>,
}
pub(super) struct Source {
    file: Arc<File>,
    scratch: Arc<File>,
    busy: Arc<AtomicBool>,
    prepared: Rc<RefCell<Option<Prepared>>>,
}

/// Pins a regular file as a shared, read-only source during Host assembly.
///
/// Identity follows the opened file, including through rename or unlink; a
/// replacement at its old pathname is a different source. Preparation reads
/// the pinned file, never resolves that pathname again.
///
/// Read `resource.state.prepare` v1 with empty data before acquiring a view.
/// Preparation performs a coherent capture on the Host worker. Acquisition is
/// inert and returns `NOT_READY` until preparation succeeds. Existing views
/// keep their captured bytes through later preparation and source withdrawal.
///
/// Views expose resource.snapshot and resource.base v1 with the same bounded
/// length/window encoding. The base is the exact original content, not a
/// timestamp or a freshness token. This read-only profile offers no conditional
/// write, locator replacement, durability or publication guarantee.
/// # Errors
/// Returns native errors for inaccessible paths or non-regular input. Clone
/// or read-lease copy support is checked by asynchronous preparation.
pub fn source(source: &Path, scratch: &Path) -> io::Result<ResourceProvider> {
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(super::NOFOLLOW | super::NONBLOCK)
        .open(source)?;
    if !file.metadata()?.is_file() {
        return Err(io::Error::from(io::ErrorKind::InvalidInput));
    }
    let state = Source::new(file, Arc::new(crate::path::root(scratch)?));
    Ok(state.provider())
}

impl Source {
    pub(super) fn new(file: File, scratch: Arc<File>) -> Rc<Self> {
        Rc::new(Self {
            file: Arc::new(file),
            scratch,
            busy: Arc::new(AtomicBool::new(false)),
            prepared: Rc::new(RefCell::new(None)),
        })
    }

    pub(super) fn select(&self, file: File, length: u64) {
        install(&self.prepared, file, length);
    }

    pub(super) fn provider(self: Rc<Self>) -> ResourceProvider {
        provider(self)
    }
}

fn provider(state: Rc<Source>) -> ResourceProvider {
    let views = state.prepared.clone();
    ResourceProvider::new(
        state,
        ResourceOps {
            unload: Some(|state| {
                if state.busy.load(core::sync::atomic::Ordering::Acquire) {
                    Err(Error::BUSY)
                } else {
                    Ok(())
                }
            }),
            read: |state, _, input| prepare(state, input),
            write: |_, _, _| Box::pin(async { Err(Error::UNSUPPORTED) }),
        },
        Box::new([super::surface("resource.state.prepare")]),
        Rc::new(()),
    )
    .with_read_view(move || {
        let selected = views.borrow();
        let selected = selected.as_ref().ok_or(Error::NOT_READY)?;
        Ok(super::reader(
            selected.file.clone(),
            selected.length,
            selected.busy.clone(),
            true,
        ))
    })
}

fn prepare(state: &Source, input: Message) -> ProviderReply {
    if !input.data.is_empty() || !input.resources.is_empty() {
        return Box::pin(async { Err(Error::INVALID) });
    }
    let source = state.file.clone();
    let scratch = state.scratch.clone();
    let prepared = state.prepared.clone();
    let work = crate::worker::start(&state.busy, move || {
        super::clone(&source, &scratch)
            .map_err(|error| super::directory::native(&error))
    });
    Box::pin(async move {
        let (file, length) = work.await?;
        install(&prepared, file, length);
        Ok(Message::new(input.contract, &[]))
    })
}

fn install(prepared: &RefCell<Option<Prepared>>, file: File, length: u64) {
    *prepared.borrow_mut() = Some(Prepared {
        file: Arc::new(file),
        length,
        busy: Arc::new(AtomicBool::new(false)),
    });
}
