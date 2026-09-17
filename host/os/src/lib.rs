#![no_std]
#![deny(unsafe_code)]
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

    /// Write `bytes` at offset `at` and end the file there, making the
    /// file when there is none. Returns the count written.
    ///
    /// # Errors
    /// As [`list`].
    pub fn write(path: &str, at: u64, bytes: &[u8]) -> Result<usize, Error> {
        backend::write(path, at, bytes)
    }

    /// The whole of `path` becomes `bytes`, or nothing changes; made
    /// when there is none. A symlink stays a symlink.
    ///
    /// # Errors
    /// As [`list`].
    pub fn replace(path: &str, bytes: &[u8]) -> Result<(), Error> {
        backend::replace(path, bytes)
    }

    /// A new empty file at `path`.
    ///
    /// # Errors
    /// `Exists` when there already is one; otherwise as [`list`].
    pub fn create(path: &str) -> Result<(), Error> {
        backend::create(path)
    }

    /// The file at `path` is gone.
    ///
    /// # Errors
    /// As [`list`].
    pub fn remove(path: &str) -> Result<(), Error> {
        backend::remove(path)
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

/// Links that carry whole messages between two ends.
///
/// What a name is, and how bytes are framed, checked or ordered below a
/// message, is the backend's own; the std backend names a Unix socket
/// path and frames each message with a `u32` length.
pub mod link {
    use alloc::vec::Vec;

    use super::{Error, backend};

    /// One listening end.
    pub struct Listener(backend::Listener);

    /// One connected end.
    pub struct Link(backend::Link);

    /// Listen at `name`; a stale name from an earlier run is replaced.
    ///
    /// # Errors
    /// `Denied` or `Io` from the host; `Unsupported` without a backend.
    pub fn listen(name: &str) -> Result<Listener, Error> {
        backend::listen(name).map(Listener)
    }

    /// The next link to `listener`, blocking.
    ///
    /// # Errors
    /// As [`listen`].
    pub fn accept(listener: &Listener) -> Result<Link, Error> {
        backend::accept(&listener.0).map(Link)
    }

    /// A link to whoever listens at `name`.
    ///
    /// # Errors
    /// `NotFound` when nothing listens there; otherwise as [`listen`].
    pub fn open(name: &str) -> Result<Link, Error> {
        backend::open(name).map(Link)
    }

    /// One whole message.
    ///
    /// # Errors
    /// `Io` when the other end is gone.
    pub fn send(link: &Link, bytes: &[u8]) -> Result<(), Error> {
        backend::send(&link.0, bytes)
    }

    /// The next whole message, blocking.
    ///
    /// # Errors
    /// `Io` when the other end is gone or sent something that is not a
    /// message.
    pub fn recv(link: &Link) -> Result<Vec<u8>, Error> {
        backend::recv(&link.0)
    }

    /// End the link; the other end sees `Io`.
    pub fn close(link: Link) {
        backend::close(link.0);
    }
}

/// The terminal: raw mode on the alternate screen, keys and size.
///
/// Drawing is the caller's, as bytes on stdout. Whatever way the
/// process leaves, the terminal is as it was found: [`term::leave`] runs
/// from a panic hook, and SIGTERM or SIGHUP end the key wait so the
/// caller's loop can leave.
pub mod term {
    use super::{Error, backend};

    /// One key, decoded. Control and a letter is `Ctrl` with the
    /// letter in lower case.
    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    pub enum Key {
        Char(char),
        Enter,
        Tab,
        Backspace,
        Left,
        Right,
        Up,
        Down,
        Home,
        End,
        Ctrl(char),
        Esc,
    }

    /// Whether stdin is a terminal. The empty backend says no.
    #[must_use]
    pub fn is_tty() -> bool {
        backend::is_tty()
    }

    /// Raw mode on the alternate screen: keys arrive one by one,
    /// nothing is echoed, and output lines need `\r\n`. The first call
    /// installs the panic hook and the signal handlers.
    ///
    /// # Errors
    /// `Io` from the host; `Unsupported` without a backend.
    pub fn enter() -> Result<(), Error> {
        backend::enter()
    }

    /// The terminal as it was before [`enter`], the cursor shown in
    /// the shape the terminal chooses; twice is fine.
    ///
    /// # Errors
    /// As [`enter`].
    pub fn leave() -> Result<(), Error> {
        backend::leave()
    }

    /// Whether SIGTERM or SIGHUP arrived; [`poll`] answers `None` at
    /// once from then on.
    #[must_use]
    pub fn stopped() -> bool {
        backend::stopped()
    }

    /// The next key within `ms` milliseconds, or `None`. An event that
    /// is not a key, or a signal, ends the wait early with `None`.
    ///
    /// # Errors
    /// As [`enter`].
    pub fn poll(ms: u64) -> Result<Option<Key>, Error> {
        backend::poll(ms)
    }

    /// Columns and rows.
    ///
    /// # Errors
    /// As [`enter`].
    pub fn size() -> Result<(u16, u16), Error> {
        backend::size()
    }
}

/// Loadable code: a `.rvm` file is a shared object with one exported
/// symbol, `rv_module_entry`. Loading runs its initializers; the caller
/// trusts the file.
pub mod native {
    use core::ffi::c_void;

    use super::{Error, backend};

    /// One loaded file, closed on drop.
    pub struct Library(backend::Library);

    /// Load `path`.
    ///
    /// # Errors
    /// `NotFound` when there is no such file, `Io` when the loader
    /// refuses it; `Unsupported` without a backend.
    pub fn load(path: &str) -> Result<Library, Error> {
        backend::load(path).map(Library)
    }

    /// The address of `rv_module_entry` in `library`.
    ///
    /// # Errors
    /// `Unsupported` when the file exports no such symbol.
    pub fn entry(library: &Library) -> Result<*const c_void, Error> {
        backend::symbol(&library.0)
    }
}

/// The system clipboard, through the first tool found on PATH:
/// `wl-paste`/`wl-copy`, `xclip`, `xsel`, `pbpaste`/`pbcopy`.
pub mod clipboard {
    use alloc::vec::Vec;

    use super::{Error, backend};

    /// What the clipboard holds.
    ///
    /// # Errors
    /// `Unsupported` when no tool is found or without a backend; `Io`
    /// when the tool fails.
    pub fn read() -> Result<Vec<u8>, Error> {
        backend::clipboard_read()
    }

    /// `bytes` onto the clipboard.
    ///
    /// # Errors
    /// As [`read`].
    pub fn write(bytes: &[u8]) -> Result<(), Error> {
        backend::clipboard_write(bytes)
    }
}

/// Where things go by default, so every argument has a value.
pub mod dirs {
    use alloc::string::String;

    use super::{Error, backend};

    /// Per-user runtime files such as the socket; created if missing.
    ///
    /// # Errors
    /// `Io` when it cannot be made; `Unsupported` without a backend.
    pub fn runtime() -> Result<String, Error> {
        backend::runtime()
    }

    /// The working directory.
    ///
    /// # Errors
    /// `Io` when there is none; `Unsupported` without a backend.
    pub fn work() -> Result<String, Error> {
        backend::work()
    }
}
