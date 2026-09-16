#![no_std]
#![forbid(unsafe_code)]
//! The machine, as plain functions. This is the only crate that knows
//! the host; everything above it stays `no_std` + alloc and calls in here.
//!
//! The backend is chosen at build time. Feature `std` binds the hosted
//! one; without it the empty backend links and answers `Unsupported`,
//! so a portable build has every symbol and no host. Another OS is
//! another backend file selected the same way.

extern crate alloc;
#[cfg(feature = "std")]
extern crate std;

use alloc::string::String;
use alloc::vec::Vec;

#[cfg_attr(feature = "std", path = "hosted.rs")]
#[cfg_attr(not(feature = "std"), path = "empty.rs")]
mod backend;

/// What the host can say went wrong. Closed; a Driver maps it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Error {
    NotFound,
    Denied,
    Exists,
    /// Any other host failure.
    Io,
    /// The backend has no such operation.
    Unsupported,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Kind {
    File,
    Dir,
}

/// Shape of one path. `mtime` is in the backend's own units; compare,
/// do not interpret.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Stat {
    pub kind: Kind,
    pub len: u64,
    pub mtime: u64,
}

/// One child of a directory.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Entry {
    pub name: String,
    pub kind: Kind,
}

/// The file system. Paths are UTF-8 and `/`-separated as the caller
/// gives them; the backend hands them on unchanged.
pub mod fs {
    use super::{Entry, Error, Stat, Vec, backend};

    /// The children of `dir`. Names that are not UTF-8 are skipped.
    ///
    /// # Errors
    /// `NotFound`, `Denied` or `Io` from the host; `Unsupported` without
    /// a backend.
    pub fn list(dir: &str) -> Result<Vec<Entry>, Error> {
        backend::list(dir)
    }

    /// Kind, length and modification time of `path`.
    ///
    /// # Errors
    /// As [`list`].
    pub fn stat(path: &str) -> Result<Stat, Error> {
        backend::stat(path)
    }

    /// Up to `len` bytes of the file at `path` from offset `at`; fewer
    /// at the end.
    ///
    /// # Errors
    /// As [`list`].
    pub fn read(path: &str, at: u64, len: usize) -> Result<Vec<u8>, Error> {
        backend::read(path, at, len)
    }

    /// Write `bytes` at offset `at` and end the file there. Returns the
    /// count written.
    ///
    /// # Errors
    /// As [`list`].
    pub fn write(path: &str, at: u64, bytes: &[u8]) -> Result<usize, Error> {
        backend::write(path, at, bytes)
    }

    /// A new empty file at `path`.
    ///
    /// # Errors
    /// `Exists` when there already is one; otherwise as [`list`].
    pub fn create(path: &str) -> Result<(), Error> {
        backend::create(path)
    }
}

/// Time, in the same units as [`Stat::mtime`].
pub mod clock {
    use super::backend;

    /// Now. The empty backend answers 0.
    #[must_use]
    pub fn now() -> u64 {
        backend::now()
    }
}
