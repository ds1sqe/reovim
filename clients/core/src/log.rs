//! `/sys/log` as the tree reads it: whole entries from a sequence.

use alloc::vec::Vec;

use reovim_kernel_interface::{Handle, Rd, Rights};

use crate::{Fault, Tree};

// The entry itself is the sdk's, since a Driver under `/bin` reads the
// same bytes over its own `Calls`; what a `Tree` does with them is here.
pub use reovim_sdk::log::{Entry, LEVELS, level, parse};

/// Bytes asked of the log per `Read`.
const CHUNK: u32 = 64 * 1024;

/// Every entry from `from` on, through `h`, a handle on the log opened
/// READ. A `Read` at a sequence answers whole entries from there.
///
/// # Errors
/// As `Tree::read`.
pub fn read(
    tree: &mut Tree,
    h: Handle,
    from: u64,
) -> Result<Vec<Entry>, Fault> {
    let mut entries: Vec<Entry> = Vec::new();
    let mut next = from;
    loop {
        let got = parse(&tree.read(h, next, CHUNK)?);
        let Some(last) = got.last() else { return Ok(entries) };
        next = last.seq + 1;
        entries.extend(got);
    }
}

/// The log rd, `/sys/log`, and every entry from `from`, through a handle
/// opened and closed here.
///
/// # Errors
/// As `Tree::resolve` and [`read`].
pub fn tail(tree: &mut Tree, from: u64) -> Result<Vec<Entry>, Fault> {
    let rd = tree.resolve("/sys/log")?;
    read_rd(tree, rd, from)
}

fn read_rd(tree: &mut Tree, rd: Rd, from: u64) -> Result<Vec<Entry>, Fault> {
    let h = tree.open(rd, Rights::READ)?;
    let entries = read(tree, h, from);
    tree.close(h)?;
    entries
}
