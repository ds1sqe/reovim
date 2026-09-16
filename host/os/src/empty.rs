//! No host: every operation is `Unsupported` and the clock stands.

use alloc::vec::Vec;

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

pub fn create(_: &str) -> Result<(), Error> {
    Err(Error::Unsupported)
}

pub const fn now() -> u64 {
    0
}
