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

/// Every entry from `from` on, 0 being from the first the ring holds,
/// through a handle opened and closed here.
///
/// # Errors
/// As `Op::Open` and `Op::Read`, and `Invalid` when the root has no
/// `/sys/log`.
pub fn from(ctx: &dyn Calls, from: u64) -> Result<Vec<Entry>, Error> {
    let rd = cell::resolve(ctx, ROOT, LOG)?;
    let h = ctx.open(rd, Rights::READ)?;
    let read = read_all(ctx, h, from);
    let _ = ctx.submit(Op::Close { h });
    read
}

/// Every entry from `from` through `h`, one `Read` at a time.
fn read_all(
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
