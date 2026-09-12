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
const TMPFILE: c_int = (1 << 22) | (1 << 16);
const EXCLUSIVE: c_int = 1 << 7;
const FICLONE: c_ulong = 0x4004_9409;
/// Maximum bytes returned by one immutable window request.
pub const WINDOW: u64 = 32768;

unsafe extern "C" {
    fn ioctl(fd: c_int, request: c_ulong, ...) -> c_int;
}

/// Captures a regular file at one atomic clone boundary during Host assembly.
///
/// Scratch is an explicit writable directory on the source filesystem. Its
/// anonymous clone cannot be linked by this adapter and dies with its last
/// owner. The returned provider exposes no mutation or file descriptor access.
/// # Errors
/// Returns native errors, including unsupported cloning or cross-device input.
/// No copying fallback can silently weaken concurrent-write consistency.
pub fn capture(source: &Path, scratch: &Path) -> io::Result<ResourceProvider> {
    let source = OpenOptions::new()
        .read(true)
        .custom_flags(NOFOLLOW | NONBLOCK)
        .open(source)?;
    if !source.metadata()?.is_file() {
        return Err(io::Error::from(io::ErrorKind::InvalidInput));
    }
    let snapshot = OpenOptions::new()
        .read(true)
        .write(true)
        .mode(0o600)
        .custom_flags(TMPFILE | EXCLUSIVE | NOFOLLOW)
        .open(scratch)?;
    if unsafe { ioctl(snapshot.as_raw_fd(), FICLONE, source.as_raw_fd()) } == -1
    {
        return Err(io::Error::last_os_error());
    }
    let length = snapshot.metadata()?.len();
    Ok(provider(snapshot, length))
}

fn provider(file: File, length: u64) -> ResourceProvider {
    ResourceProvider::new(
        (Arc::new(file), length, Arc::new(AtomicBool::new(false))),
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
        Box::new([Surface {
            contract: Contract::new("resource.snapshot", 1),
            methods: Permissions::READ,
            input: Box::new([]),
            output: Box::new([]),
        }]),
        Rc::new(()),
    )
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
