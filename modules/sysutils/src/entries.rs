//! The entries `/sys/log` holds from a sequence.
//!
//! The log is a path in the tree like any other, and a `Read` at a
//! sequence answers whole entries from there, so a run asks past the
//! last one it saw until a read brings nothing.

use alloc::vec::Vec;

use reovim_kernel_interface::{Calls, Error, Handle, Op, Out, Rights};
use reovim_sdk::cell;
use reovim_sdk::log::{Entry, parse};
use reovim_sdk::ops::ready;
use reovim_sdk::walk::ROOT;

/// Bytes asked of the log per `Read`.
const CHUNK: u32 = 64 * 1024;

/// Where the log is; a Driver names no Module to reach it.
const LOG: &str = "/sys/log";

/// A handle on the log, open for READ; a run that tails it keeps one,
/// so a following read is the reads alone and no lookup.
///
/// # Errors
/// As `Op::Open`, and `Invalid` when the root has no `/sys/log`.
pub fn open(ctx: &dyn Calls) -> Result<Handle, Error> {
    let rd = cell::resolve(ctx, ROOT, LOG)?;
    ctx.open(rd, Rights::READ)
}

/// Every entry from `from` on through `h`, 0 being from the first the
/// ring holds, one `Read` at a time.
///
/// # Errors
/// As `Op::Read`.
pub fn from(
    ctx: &dyn Calls,
    h: Handle,
    from: u64,
) -> Result<Vec<Entry>, Error> {
    let mut entries: Vec<Entry> = Vec::new();
    let mut next = from;
    loop {
        let out = ready(ctx.submit(Op::Read { h, at: next, len: CHUNK }))?;
        let Out::Bytes(bytes) = out else { return Err(Error::Invalid) };
        let got = parse(&bytes);
        let Some(last) = got.last() else { return Ok(entries) };
        next = last.seq + 1;
        entries.extend(got);
    }
}
