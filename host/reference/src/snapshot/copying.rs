//! Coherent copy under a private, worker-owned Linux read lease.

use std::{
    fs::{File, OpenOptions},
    io,
    os::{
        fd::AsRawFd,
        unix::fs::{FileExt, MetadataExt, OpenOptionsExt},
    },
};

// Linux fcntl.h; libc does not expose this operation or its two-integer type.
#[repr(C)]
struct Owner {
    kind: libc::c_int,
    pid: libc::pid_t,
}
const SET_OWNER_EX: libc::c_int = 15;

#[cfg(test)]
mod tests;

pub(super) fn capture(
    source: &File,
    snapshot: File,
) -> io::Result<(File, u64)> {
    // A dedicated thread can exit with lease notifications blocked. Neither
    // the caller's mask nor the process-wide SIGIO disposition is changed.
    std::thread::scope(|scope| {
        std::thread::Builder::new()
            .name("snapshot-copy".into())
            .spawn_scoped(scope, move || copy(source, snapshot))?
            .join()
            .map_err(|_| io::Error::other("snapshot copy worker panicked"))?
    })
}

fn copy(source: &File, snapshot: File) -> io::Result<(File, u64)> {
    let window = usize::try_from(super::WINDOW).unwrap();
    let mut bytes = alloc::vec::Vec::new();
    bytes
        .try_reserve_exact(window)
        .map_err(|_| io::Error::from_raw_os_error(libc::ENOMEM))?;
    bytes.resize(window, 0);
    let lease = lease(source)?;
    let length = lease.metadata()?.len();
    // A failed clone must not leave old bytes beneath skipped sparse holes.
    snapshot.set_len(0)?;
    snapshot.set_len(length)?;
    let mut offset = 0;
    while let Some((start, end)) = extent(&lease, offset, length)? {
        copy_range(&lease, &snapshot, &mut bytes, start, end)?;
        offset = end;
    }
    held(&lease)?;
    // The only descriptor for this lease closes before the worker exits.
    // The returned scratch file has independent immutable byte meaning.
    drop(lease);
    Ok((snapshot, length))
}

fn lease(source: &File) -> io::Result<File> {
    let identity = source.metadata()?;
    let file = OpenOptions::new()
        .read(true)
        .custom_flags(libc::O_NONBLOCK | libc::O_CLOEXEC)
        .open(alloc::format!("/proc/self/fd/{}", source.as_raw_fd()))?;
    let actual = file.metadata()?;
    if (actual.dev(), actual.ino()) != (identity.dev(), identity.ino()) {
        return Err(io::Error::from_raw_os_error(libc::ESTALE));
    }
    block_notification()?;
    // SAFETY: gettid has no pointer arguments; owner has the native ABI layout
    // and remains valid throughout the synchronous fcntl call.
    let owner = Owner {
        kind: 0, // F_OWNER_TID
        pid: unsafe { libc::gettid() },
    };
    if unsafe { libc::fcntl(file.as_raw_fd(), SET_OWNER_EX, &owner) } < 0 {
        return Err(io::Error::last_os_error());
    }
    control(&file, libc::F_SETLEASE, libc::F_RDLCK).map_err(|error| {
        if error.raw_os_error() == Some(libc::EINVAL) {
            io::Error::from_raw_os_error(libc::EOPNOTSUPP)
        } else {
            error
        }
    })?;
    Ok(file)
}

fn block_notification() -> io::Result<()> {
    let mut signals = core::mem::MaybeUninit::<libc::sigset_t>::uninit();
    // SAFETY: sigemptyset initializes the native signal set before it is
    // passed to sigaddset and pthread_sigmask. No handler is installed.
    unsafe {
        if libc::sigemptyset(signals.as_mut_ptr()) != 0 {
            return Err(io::Error::last_os_error());
        }
        let mut signals = signals.assume_init();
        if libc::sigaddset(&raw mut signals, libc::SIGIO) != 0 {
            return Err(io::Error::last_os_error());
        }
        let error = libc::pthread_sigmask(
            libc::SIG_BLOCK,
            &raw const signals,
            core::ptr::null_mut(),
        );
        if error != 0 {
            return Err(io::Error::from_raw_os_error(error));
        }
    }
    Ok(())
}

fn control(
    file: &File,
    command: libc::c_int,
    argument: libc::c_int,
) -> io::Result<libc::c_int> {
    // SAFETY: the descriptor is live and these commands accept an integer.
    let result = unsafe { libc::fcntl(file.as_raw_fd(), command, argument) };
    if result < 0 { Err(io::Error::last_os_error()) } else { Ok(result) }
}

fn held(file: &File) -> io::Result<()> {
    if control(file, libc::F_GETLEASE, 0)? != libc::F_RDLCK {
        return Err(io::Error::from_raw_os_error(libc::ESTALE));
    }
    Ok(())
}

fn copy_range(
    source: &File,
    target: &File,
    bytes: &mut [u8],
    mut start: u64,
    end: u64,
) -> io::Result<()> {
    while start < end {
        held(source)?;
        let size =
            usize::try_from((end - start).min(bytes.len() as u64)).unwrap();
        source.read_exact_at(&mut bytes[..size], start)?;
        target.write_all_at(&bytes[..size], start)?;
        start += size as u64;
    }
    Ok(())
}

fn extent(
    file: &File,
    offset: u64,
    length: u64,
) -> io::Result<Option<(u64, u64)>> {
    if offset == length {
        return Ok(None);
    }
    held(file)?;
    let start = match seek(file, offset, libc::SEEK_DATA) {
        Err(error) if error.raw_os_error() == Some(libc::ENXIO) => {
            return Ok(None);
        }
        Err(error) if error.raw_os_error() == Some(libc::EINVAL) => {
            return Ok(Some((offset, length)));
        }
        other => other?,
    };
    let end = match seek(file, start, libc::SEEK_HOLE) {
        Err(error) if error.raw_os_error() == Some(libc::EINVAL) => length,
        other => other?.min(length),
    };
    if start < offset || end <= start {
        return Err(io::Error::from(io::ErrorKind::InvalidData));
    }
    Ok(Some((start, end)))
}

fn seek(file: &File, offset: u64, whence: libc::c_int) -> io::Result<u64> {
    let offset = libc::off_t::try_from(offset)
        .map_err(|_| io::ErrorKind::InvalidInput)?;
    // SAFETY: the descriptor is live; offset and whence are native integers.
    let result = unsafe { libc::lseek(file.as_raw_fd(), offset, whence) };
    u64::try_from(result).map_err(|_| io::Error::last_os_error())
}
