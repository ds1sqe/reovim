//! Linux descriptor-relative creation; every traversed directory is pinned.

use core::ffi::{c_char, c_int};
use std::{
    ffi::CString,
    fs::{File, OpenOptions},
    io,
    os::{
        fd::{AsRawFd, FromRawFd},
        unix::fs::OpenOptionsExt,
    },
    path::Path,
};

// Linux UAPI flags, shared by the declared x86_64 hosted target.
const DIRECTORY: c_int = 1 << 16;
const NOFOLLOW: c_int = 1 << 17;
const CLOEXEC: c_int = 1 << 19;
const CREATE: c_int = 1 << 6;
const EXCLUSIVE: c_int = 1 << 7;
unsafe extern "C" {
    fn openat(
        directory: c_int,
        path: *const c_char,
        flags: c_int,
        ...
    ) -> c_int;
    fn mkdirat(directory: c_int, path: *const c_char, mode: u32) -> c_int;
}

pub fn root(path: &Path) -> io::Result<File> {
    OpenOptions::new().read(true).custom_flags(DIRECTORY | NOFOLLOW).open(path)
}

pub fn read(root: &File, path: &str) -> io::Result<File> {
    let mut parts = path.split('/').peekable();
    for part in parts.clone() {
        component(part)?;
    }
    let mut parent = root.try_clone()?;
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            let name = component(part)?;
            let fd = unsafe {
                openat(
                    parent.as_raw_fd(),
                    name.as_ptr(),
                    NOFOLLOW | CLOEXEC | (1 << 11),
                )
            };
            return owned(fd);
        }
        parent = child(&parent, part)?;
    }
    Err(io::Error::from(io::ErrorKind::InvalidInput))
}

pub fn anonymous(root: &File) -> io::Result<File> {
    let fd = unsafe {
        openat(
            root.as_raw_fd(),
            c".".as_ptr(),
            2 | (1 << 22) | DIRECTORY | EXCLUSIVE | CLOEXEC,
            0o600_u32,
        )
    };
    owned(fd)
}
pub fn component(name: &str) -> io::Result<CString> {
    if name.is_empty() || name.contains('/') || matches!(name, "." | "..") {
        return Err(io::Error::from(io::ErrorKind::InvalidInput));
    }
    CString::new(name).map_err(|_| io::Error::from(io::ErrorKind::InvalidInput))
}
pub fn directory(parent: &File, name: &str) -> io::Result<()> {
    make_directory(parent, name)?;
    parent.sync_all()
}
pub fn make_directory(parent: &File, name: &str) -> io::Result<()> {
    let name = component(name)?;
    if unsafe { mkdirat(parent.as_raw_fd(), name.as_ptr(), 0o700) } == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}
pub fn child(parent: &File, name: &str) -> io::Result<File> {
    let name = component(name)?;
    let fd = unsafe {
        openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            DIRECTORY | NOFOLLOW | CLOEXEC,
        )
    };
    owned(fd)
}
pub fn file(root: &File, path: &str) -> io::Result<File> {
    let mut parts = path.split('/').peekable();
    // Validate every component before creating intermediate directories.
    for part in parts.clone() {
        component(part)?;
    }
    let mut parent = root.try_clone()?;
    while let Some(part) = parts.next() {
        if parts.peek().is_none() {
            return create(&parent, part);
        }
        parent = intermediate(&parent, part)?;
    }
    Err(io::Error::from(io::ErrorKind::InvalidInput))
}
fn intermediate(parent: &File, name: &str) -> io::Result<File> {
    match directory(parent, name) {
        Ok(()) => (),
        Err(error) if error.kind() == io::ErrorKind::AlreadyExists => (),
        Err(error) => return Err(error),
    }
    child(parent, name)
}
fn create(parent: &File, name: &str) -> io::Result<File> {
    let name = component(name)?;
    let fd = unsafe {
        openat(
            parent.as_raw_fd(),
            name.as_ptr(),
            1 | CREATE | EXCLUSIVE | NOFOLLOW | CLOEXEC,
            0o600_u32,
        )
    };
    let file = owned(fd)?;
    parent.sync_all()?;
    Ok(file)
}
fn owned(fd: c_int) -> io::Result<File> {
    if fd == -1 {
        return Err(io::Error::last_os_error());
    }
    Ok(unsafe { File::from_raw_fd(fd) })
}
