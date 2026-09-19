//! `capture`, a Driver under `/bin`: its declaration and its run. The
//! slots it is served through are the crate's.
//!
//! One run answers with an output cell holding the rows a `text` cell
//! was last drawn with, the status among them, and the cursor on a
//! last line. A target that holds no `Screen` fails the run and
//! leaves no cell.
//!
//! Under `f` the cell goes on: the run watches its target, and a read
//! at the cell's end answers the whole screen again for every change
//! since the read before.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write;

use reovim_client_api::{BAR, display, width};
use reovim_kernel_interface::{Calls, Error, Event, Handle, Kind, Rd};
use reovim_sdk::cell::{self, Follow};
use reovim_sdk::{decode, ops};

use crate::Sysutils;

reovim_sdk::declare! {
    api capture ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args { f: bool, cwd: Rd, target: utf8(256) }
}

/// What a run takes, beside the api it belongs to.
pub use capture::Args;
pub(crate) use capture::{decl, dispatch};

impl capture::Api for Sysutils {
    fn run(&self, ctx: &dyn Calls, rd: Rd, args: Args) -> Result<Rd, Error> {
        let target = cell::resolve(ctx, args.cwd, &args.target)?;
        if !args.f {
            return self.cell(ctx, rd, shown(ctx, target)?);
        }
        // Watched before the first read, so nothing drawn between the
        // two is missed; a first screen that fails closes it again.
        let h = ops::watch(ctx, target)?;
        let begun = begin(ctx, target).inspect_err(|_| ops::shut(ctx, &[h]));
        let (cursor, first) = begun?;
        let again = Again { h, target, cursor };
        self.following(ctx, rd, first, Box::new(again))
    }
}

/// Where a following run stands once it is watching: the cursor its
/// events are counted from, which leaves the ring's backlog to whoever
/// asked for it, and the screen as it is now.
fn begin(ctx: &dyn Calls, target: Rd) -> Result<(u64, Vec<u8>), Error> {
    let cursor = ops::since(ctx, 0)?.1;
    Ok((cursor, shown(ctx, target)?))
}

/// The screen `target` holds, as a person reads it: one line a row,
/// its trailing spaces cut as a terminal's own capture cuts them, then
/// the status the same way, then `cursor ROW COL SHAPE` last, `rows`
/// telling how many blank rows pad `lines` out between the two.
///
/// A whole read of the cell is one frame, the Client snapshotting it
/// at the read of 0, so no line here is of another frame than its
/// neighbours; `rows` is that same frame's, not one a resize between
/// two reads could disagree with it about.
fn shown(ctx: &dyn Calls, target: Rd) -> Result<Vec<u8>, Error> {
    let screen: display::Screen = decode(&ops::bytes(ctx, target)?)?;
    let text_rows = usize::try_from(screen.rows).unwrap_or(usize::MAX);
    let mut text = String::new();
    for line in &screen.lines {
        text.push_str(line.text.trim_end_matches(' '));
        text.push('\n');
    }
    for _ in screen.lines.len()..text_rows.saturating_sub(1) {
        text.push('\n');
    }
    text.push_str(screen.status.trim_end_matches(' '));
    text.push('\n');
    let shape = if screen.shape == BAR { "bar" } else { "block" };
    let (row, col) = (screen.row, column(&screen));
    let _ = writeln!(text, "cursor {row} {col} {shape}");
    Ok(text.into_bytes())
}

/// The cell of its cursor's line where the byte `screen.col` names
/// begins: the same cells-per-char rule the drawer counted it in.
fn column(screen: &display::Screen) -> usize {
    let byte = usize::try_from(screen.col).unwrap_or(usize::MAX);
    let row = usize::try_from(screen.row).unwrap_or(usize::MAX);
    screen.lines.get(row).map_or(0, |line| {
        line.text[..byte.min(line.text.len())].chars().map(width).sum()
    })
}

/// `f` between reads: the handle that holds the watch, what is
/// watched, and this follower's own place in the Kernel's event ring.
struct Again {
    h: Handle,
    target: Rd,
    cursor: u64,
}

impl Follow for Again {
    fn more(&mut self, ctx: &dyn Calls) -> Result<Option<Vec<u8>>, Error> {
        let (events, cursor) = ops::since(ctx, self.cursor)?;
        self.cursor = cursor;
        let redrawn = |event: &Event| {
            event.kind == Kind::Modify && event.rd == self.target
        };
        if !events.iter().any(redrawn) {
            return Ok(Some(Vec::new()));
        }
        // However many frames were drawn, what is answered is the one
        // that stands now: a screen is a picture and not a stream.
        shown(ctx, self.target).map(Some)
    }

    fn end(&mut self, ctx: &dyn Calls) {
        ops::shut(ctx, &[self.h]);
    }
}
