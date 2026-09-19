#![no_std]
#![forbid(unsafe_code)]
//! sysutils: the Module that provides the Drivers under `/bin` that
//! face the Kernel itself.
//!
//! coreutils has the tree and its bytes; these eight have what only
//! the Kernel holds: `log` and `dmesg` read `/sys/log`, `call` runs a
//! method of whatever api an rd offers, `watch` says what happens to an
//! rd, `date` and `uptime` spell two of the facts under `/sys/info`,
//! and `capture` and `keys` are a person's hands on another Client's
//! screen and keyboard. The shape is coreutils' own and is the sdk's
//! `drivers!`.
//!
//! Each api is a file of `bins`. Beside them the three things more
//! than one of them reads: `entries`, the log from a sequence,
//! `names`, what the numbers of the Kernel's own lines stand for, and
//! here the fact the two readers both read.

extern crate alloc;

use reovim_kernel_interface::{Calls, Error};
use reovim_sdk::cell;
use reovim_sdk::ops;
use reovim_sdk::walk::ROOT;

pub mod bins;
mod entries;
mod names;

use bins::{call, capture, date, dmesg, keys, log, uptime, watch};

reovim_sdk::drivers!(
    Sysutils: log, dmesg, call, watch, date, uptime, capture, keys
);

/// The number the fact at `path` holds, which is the whole of what a
/// reader under `/bin` is a wording of. The path is absolute and the
/// walk starts at the root, so the same reader loaded at a Client reads
/// the server's own; a cell that is no number is `Invalid`, which no
/// reader guesses a value for.
fn fact(ctx: &dyn Calls, path: &str) -> Result<u64, Error> {
    let rd = cell::resolve(ctx, ROOT, path)?;
    let bytes = ops::bytes(ctx, rd)?;
    let text = core::str::from_utf8(&bytes).map_err(|_| Error::Invalid)?;
    text.parse().map_err(|_| Error::Invalid)
}
