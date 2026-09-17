//! No host: every operation is `Unsupported` and the clock stands.

use alloc::string::String;
use alloc::vec::Vec;

use crate::term::Key;
use crate::{Entry, Error, Stat};

pub fn list(_: &str) -> Result<Vec<Entry>, Error> {
    Err(Error::Unsupported)
}

pub fn stat(_: &str) -> Result<Stat, Error> {
    Err(Error::Unsupported)
}

pub fn read(_: &str, _: u64, _: usize) -> Result<Vec<u8>, Error> {
    Err(Error::Unsupported)
}

pub fn write(_: &str, _: u64, _: &[u8]) -> Result<usize, Error> {
    Err(Error::Unsupported)
}

pub fn replace(_: &str, _: &[u8]) -> Result<(), Error> {
    Err(Error::Unsupported)
}

pub fn create(_: &str) -> Result<(), Error> {
    Err(Error::Unsupported)
}

pub fn remove(_: &str) -> Result<(), Error> {
    Err(Error::Unsupported)
}

pub const fn now() -> u64 {
    0
}

pub const fn is_tty_out() -> bool {
    false
}

pub const fn catch_interrupt() {}

pub const fn interrupted() -> bool {
    false
}

pub struct Listener;
pub struct Link;

pub fn listen(_: &str) -> Result<Listener, Error> {
    Err(Error::Unsupported)
}

pub fn accept(_: &Listener) -> Result<Link, Error> {
    Err(Error::Unsupported)
}

pub fn open(_: &str) -> Result<Link, Error> {
    Err(Error::Unsupported)
}

pub fn send(_: &Link, _: &[u8]) -> Result<(), Error> {
    Err(Error::Unsupported)
}

pub fn recv(_: &Link) -> Result<Vec<u8>, Error> {
    Err(Error::Unsupported)
}

pub const fn close(_: Link) {}

pub struct Library;

pub fn load(_: &str) -> Result<Library, Error> {
    Err(Error::Unsupported)
}

pub fn symbol(_: &Library) -> Result<*const core::ffi::c_void, Error> {
    Err(Error::Unsupported)
}

pub fn clipboard_read() -> Result<Vec<u8>, Error> {
    Err(Error::Unsupported)
}

pub fn clipboard_write(_: &[u8]) -> Result<(), Error> {
    Err(Error::Unsupported)
}

pub fn runtime() -> Result<String, Error> {
    Err(Error::Unsupported)
}

pub fn work() -> Result<String, Error> {
    Err(Error::Unsupported)
}

pub const fn is_tty() -> bool {
    false
}

pub fn enter() -> Result<(), Error> {
    Err(Error::Unsupported)
}

pub fn leave() -> Result<(), Error> {
    Err(Error::Unsupported)
}

pub const fn stopped() -> bool {
    false
}

pub fn poll(_: u64) -> Result<Option<Key>, Error> {
    Err(Error::Unsupported)
}

pub fn size() -> Result<(u16, u16), Error> {
    Err(Error::Unsupported)
}
