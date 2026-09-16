#![no_std]
#![forbid(unsafe_code)]
//! The clipboard Driver: the system clipboard as one rd,
//! `/module/clipboard/text`, read and written through the host's
//! tool. It holds nothing: each read asks the tool, each write hands
//! it the bytes whole.

extern crate alloc;

use alloc::boxed::Box;
use alloc::rc::Rc;
use alloc::vec::Vec;

use reovim_host::clipboard;
use reovim_kernel_interface::{
    Booted, Calls, Driver, DriverId, Error, Flags, Module, Rd, Reply, Rights,
};

/// What reads the clipboard.
pub type Read = Box<dyn Fn() -> Result<Vec<u8>, reovim_host::Error>>;
/// What writes it.
pub type Write = Box<dyn Fn(&[u8]) -> Result<(), reovim_host::Error>>;

/// The Driver and its Module in one; pass it twice at `load`.
pub struct Clipboard {
    read: Read,
    write: Write,
}

/// The clipboard Module over the host's tools, ready for `load`;
/// `root` is unused.
#[must_use]
pub fn boot(_root: &str) -> Booted {
    let read: Read = Box::new(clipboard::read);
    let write: Write = Box::new(clipboard::write);
    let module = Rc::new(Clipboard::new(read, write));
    Booted { module: module.clone(), drivers: alloc::vec![module] }
}

/// Host failures as the Driver's: no tool is `Unsupported`, a tool
/// that fails is `Busy`.
const fn map(error: reovim_host::Error) -> Error {
    match error {
        reovim_host::Error::NotFound | reovim_host::Error::Exists => {
            Error::Invalid
        }
        reovim_host::Error::Denied => Error::Denied,
        reovim_host::Error::Io => Error::Busy,
        reovim_host::Error::Unsupported => Error::Unsupported,
    }
}

/// `len` bytes of `bytes` from `at`; none past the end.
fn slice(bytes: &[u8], at: u64, len: usize) -> Vec<u8> {
    let start = usize::try_from(at).unwrap_or(usize::MAX).min(bytes.len());
    let end = start.saturating_add(len).min(bytes.len());
    bytes[start..end].to_vec()
}

impl Clipboard {
    /// A clipboard behind `read` and `write`: the host's, or a pair
    /// of a test's own.
    #[must_use]
    pub const fn new(read: Read, write: Write) -> Self {
        Self { read, write }
    }
}

impl Module for Clipboard {
    fn start(&self, ctx: &dyn Calls, home: Rd) -> Result<(), Error> {
        let rd = ctx.register(DriverId(0), home, Some("text"))?;
        ctx.report(rd, Flags::READ.union(Flags::WRITE), &[])
    }
}

impl Driver for Clipboard {
    fn open(&self, _: &dyn Calls, rd: Rd, _: Rights) -> Reply<u64> {
        Reply::ok(rd.0)
    }

    fn read(
        &self,
        _: &dyn Calls,
        _: u64,
        at: u64,
        len: usize,
    ) -> Reply<Vec<u8>> {
        (self.read)().map_err(map).map(|bytes| slice(&bytes, at, len)).into()
    }

    /// The bytes whole, at 0 only.
    fn write(
        &self,
        _: &dyn Calls,
        _: u64,
        at: u64,
        buf: &[u8],
    ) -> Reply<usize> {
        if at != 0 {
            return Err(Error::Invalid).into();
        }
        (self.write)(buf).map_err(map).map(|()| buf.len()).into()
    }
}
