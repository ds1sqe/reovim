//! `cat`, a Driver under `/bin`: its declaration and its
//! run. The slots it is served through are the crate's.
//!
//! One run answers with an output cell holding the bytes of each
//! target in turn. A target that holds no bytes, a Container among
//! them, fails the run and leaves no cell.
//!
//! Under `f` the cell goes on: the run watches its targets, and a read
//! at the cell's end answers the whole of every target that changed
//! since the read before. `n` is how many of those answers end it; 0 is
//! no limit.

use alloc::boxed::Box;
use alloc::vec::Vec;

use reovim_kernel_interface::{Calls, Error, Event, Handle, Kind, Rd};
use reovim_sdk::cell::{self, Follow};
use reovim_sdk::ops;

use crate::Coreutils;

reovim_sdk::declare! {
    api cat ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args { f: bool, n: u64, cwd: Rd, targets: [utf8(256); 64] }
}

/// What a run takes, beside the api it belongs to.
pub use cat::Args;
pub(crate) use cat::{decl, dispatch};

impl cat::Api for Coreutils {
    fn run(&self, ctx: &dyn Calls, rd: Rd, args: Args) -> Result<Rd, Error> {
        let mut targets = Vec::new();
        for target in &args.targets {
            targets.push(cell::resolve(ctx, args.cwd, target)?);
        }
        if !args.f {
            return self.cell(ctx, rd, bytes(ctx, &targets)?);
        }
        // Watched before the first read, so nothing written between the
        // two is missed; a fill that fails closes them again.
        let handles = ops::watch_all(ctx, &targets)?;
        let begun =
            begin(ctx, &targets).inspect_err(|_| ops::shut(ctx, &handles));
        let (cursor, first) = begun?;
        let left = (args.n > 0).then_some(args.n);
        let again = Again { handles, targets, cursor, left };
        self.following(ctx, rd, first, Box::new(again))
    }
}

/// Where a following run stands once it is watching: the cursor its
/// events are counted from, which leaves the ring's backlog to whoever
/// asked for it, and the bytes of the targets as they are now.
fn begin(ctx: &dyn Calls, targets: &[Rd]) -> Result<(u64, Vec<u8>), Error> {
    let cursor = ops::since(ctx, 0)?.1;
    Ok((cursor, bytes(ctx, targets)?))
}

/// The bytes of each target in turn.
fn bytes(ctx: &dyn Calls, targets: &[Rd]) -> Result<Vec<u8>, Error> {
    let mut bytes = Vec::new();
    for target in targets {
        bytes.extend(ops::bytes(ctx, *target)?);
    }
    Ok(bytes)
}

/// `f` between reads: the handles that hold the watch, what is
/// watched, this follower's own place in the Kernel's event ring, and
/// how many answers are left before it is over.
struct Again {
    handles: Vec<Handle>,
    targets: Vec<Rd>,
    cursor: u64,
    left: Option<u64>,
}

impl Follow for Again {
    fn more(&mut self, ctx: &dyn Calls) -> Result<Option<Vec<u8>>, Error> {
        if self.left == Some(0) {
            return Ok(None);
        }
        let (events, cursor) = ops::since(ctx, self.cursor)?;
        self.cursor = cursor;
        let changed = changed(&events, &self.targets);
        let (bytes, left) = answers(ctx, &changed, self.left)?;
        self.left = left;
        Ok(Some(bytes))
    }

    fn end(&mut self, ctx: &dyn Calls) {
        ops::shut(ctx, &self.handles);
    }
}

/// Which of `targets` the events say changed, in the order they say
/// them; one target says itself once per event.
fn changed(events: &[Event], targets: &[Rd]) -> Vec<Rd> {
    let modified = |event: &&Event| {
        event.kind == Kind::Modify && targets.contains(&event.rd)
    };
    events.iter().filter(modified).map(|event| event.rd).collect()
}

/// The whole of each of `changed` in turn, and how many answers are
/// left once they are written. An answer of nothing is nothing said and
/// counts for nothing.
fn answers(
    ctx: &dyn Calls,
    changed: &[Rd],
    mut left: Option<u64>,
) -> Result<(Vec<u8>, Option<u64>), Error> {
    let mut bytes = Vec::new();
    for target in changed {
        if left == Some(0) {
            break;
        }
        let read = ops::bytes(ctx, *target)?;
        if read.is_empty() {
            continue;
        }
        left = left.map(|n| n.saturating_sub(1));
        bytes.extend(read);
    }
    Ok((bytes, left))
}
