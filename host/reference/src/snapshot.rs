//! Immutable file snapshots with bounded worker-owned reads.

use alloc::{boxed::Box, rc::Rc, sync::Arc, vec::Vec};
use core::{
    ffi::{c_int, c_ulong},
    sync::atomic::{AtomicBool, Ordering},
};
use reovim_kernel_interface::{
    Contract, Error, Message,
    provider::{Permissions, ResourceOps, ResourceProvider, Surface},
};
use std::{
    fs::{File, OpenOptions},
    io,
    os::{
        fd::AsRawFd,
        unix::fs::{FileExt, OpenOptionsExt},
    },
    path::Path,
};

const NOFOLLOW: c_int = 1 << 17;
const NONBLOCK: c_int = 1 << 11;
const FICLONE: c_ulong = 0x4004_9409;
/// Maximum bytes returned by one immutable window request.
pub const WINDOW: u64 = 32768;

mod directory;
pub use directory::Directory;
mod source;
pub use source::source;
#[cfg(target_arch = "x86_64")]
mod copying;
mod selection;

unsafe extern "C" {
    fn ioctl(fd: c_int, request: c_ulong, ...) -> c_int;
}

/// Captures a regular file coherently during Host assembly.
///
/// Scratch is an explicit writable directory. Its anonymous snapshot cannot
/// be linked by this adapter and dies with its last
/// owner. The returned provider exposes no mutation or file descriptor access.
/// # Errors
/// Cloning is preferred. Linux `x86_64` can copy under a private read lease
/// when cloning is unavailable. Lease conflict, break or unsupported protection
/// rejects; copying never relies on timestamp comparisons. This fallback
/// requires owned regular input and /proc/self/fd for private descriptor
/// access.
/// Returns native errors for unsupported protection, I/O or resource pressure.
pub fn capture(source: &Path, scratch: &Path) -> io::Result<ResourceProvider> {
    let source = OpenOptions::new()
        .read(true)
        .custom_flags(NOFOLLOW | NONBLOCK)
        .open(source)?;
    let scratch = crate::path::root(scratch)?;
    let (file, length) = clone(&source, &scratch)?;
    Ok(provider(file, length))
}

fn clone(source: &File, scratch: &File) -> io::Result<(File, u64)> {
    if !source.metadata()?.is_file() {
        return Err(io::Error::from(io::ErrorKind::InvalidInput));
    }
    let snapshot = crate::path::anonymous(scratch)?;
    if unsafe { ioctl(snapshot.as_raw_fd(), FICLONE, source.as_raw_fd()) } == -1
    {
        let error = io::Error::last_os_error();
        #[cfg(target_arch = "x86_64")]
        if matches!(error.raw_os_error(), Some(18 | 22 | 25 | 95)) {
            return copying::capture(source, snapshot);
        }
        return Err(error);
    }
    let length = snapshot.metadata()?.len();
    Ok((snapshot, length))
}

fn provider(file: File, length: u64) -> ResourceProvider {
    reader(Arc::new(file), length, Arc::new(AtomicBool::new(false)), false)
}

fn reader(
    file: Arc<File>,
    length: u64,
    busy: Arc<AtomicBool>,
    base: bool,
) -> ResourceProvider {
    let view_file = file.clone();
    let view_busy = busy.clone();
    ResourceProvider::new(
        (file, length, busy),
        ResourceOps {
            unload: Some(|(_, _, busy)| {
                if busy.load(Ordering::Acquire) {
                    Err(Error::BUSY)
                } else {
                    Ok(())
                }
            }),
            read: |(file, length, busy), _, input| {
                let (file, length) = (file.clone(), *length);
                crate::worker::start(busy, move || read(&file, length, input))
            },
            write: |_, _, _| Box::pin(async { Err(Error::DENIED) }),
        },
        surfaces(base),
        Rc::new(()),
    )
    .with_read_view(move || {
        Ok(reader(view_file.clone(), length, view_busy.clone(), base))
    })
}

fn surfaces(base: bool) -> Box<[Surface]> {
    if base {
        Box::new([surface("resource.snapshot"), surface("resource.base")])
    } else {
        Box::new([surface("resource.snapshot")])
    }
}

fn surface(name: &str) -> Surface {
    Surface {
        contract: Contract::new(name, 1),
        methods: Permissions::READ,
        input: Box::new([]),
        output: Box::new([]),
    }
}

fn read(file: &File, length: u64, input: Message) -> Result<Message, Error> {
    if input.data.is_empty() {
        return Ok(Message::new(input.contract, &length.to_le_bytes()));
    }
    let raw: &[u8; 16] =
        input.data.as_ref().try_into().map_err(|_| Error::INVALID)?;
    let start = u64::from_le_bytes(raw[..8].try_into().unwrap());
    let end = u64::from_le_bytes(raw[8..].try_into().unwrap());
    let size = end.checked_sub(start).ok_or(Error::INVALID)?;
    if end > length || size > WINDOW {
        return Err(Error::INVALID);
    }
    let size = usize::try_from(size).map_err(|_| Error::PRESSURE)?;
    let mut bytes = Vec::new();
    bytes.try_reserve_exact(size).map_err(|_| Error::PRESSURE)?;
    bytes.resize(size, 0);
    file.read_exact_at(&mut bytes, start).map_err(|_| Error::UNAVAILABLE)?;
    Ok(Message {
        contract: input.contract,
        data: bytes.into(),
        resources: Box::new([]),
    })
}
