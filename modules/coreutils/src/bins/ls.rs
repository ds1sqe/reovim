//! `ls`, a Driver under `/bin`: its declaration and its
//! run. The slots it is served through are the crate's.
//!
//! One run answers with an output cell holding the children of each
//! target by name, sorted: down the columns that fit in `width`, one
//! name a line when `width` is 0, and the long line under `long`. A
//! target that is no Container lists itself. No target at all is the
//! `cwd` the run was given.
//!
//! Under `w` the cell goes on: the run watches its targets, and a read
//! at the cell's end answers the listing again for every event under
//! one of them.

use alloc::boxed::Box;
use alloc::string::String;
use alloc::vec::Vec;
use alloc::{format, vec};
use core::fmt::Write as _;

use reovim_kernel_interface::{Calls, Error, Flags, Handle, Rd};
use reovim_sdk::cell::{self, Follow};
use reovim_sdk::ops;

use crate::Coreutils;
use crate::format::{self, Entry};

reovim_sdk::declare! {
    api ls ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args {
        long: bool,
        all: bool,
        w: bool,
        cwd: Rd,
        width: u64,
        color: bool,
        targets: [utf8(256); 64]
    }
}

/// What a run takes, beside the api it belongs to.
pub use ls::Args;
pub(crate) use ls::{decl, dispatch};

impl ls::Api for Coreutils {
    fn run(&self, ctx: &dyn Calls, rd: Rd, args: Args) -> Result<Rd, Error> {
        if !args.w {
            return self.cell(ctx, rd, text(ctx, &args)?.into_bytes());
        }
        let mut targets = Vec::new();
        for target in named(&args) {
            targets.push(cell::resolve(ctx, args.cwd, target)?);
        }
        // Watched before the first listing, so nothing that happens
        // between the two is missed; a listing that fails closes them
        // again.
        let handles = ops::watch_all(ctx, &targets)?;
        let begun = begin(ctx, &args).inspect_err(|_| ops::shut(ctx, &handles));
        let (cursor, first) = begun?;
        let relist = Relist { handles, targets, cursor, args };
        self.following(ctx, rd, first, Box::new(relist))
    }
}

/// Where a following run stands once it is watching: the cursor its
/// events are counted from, which leaves the ring's backlog to whoever
/// asked for it, and the listing as it is now.
fn begin(ctx: &dyn Calls, args: &Args) -> Result<(u64, Vec<u8>), Error> {
    let cursor = ops::since(ctx, 0)?.1;
    Ok((cursor, text(ctx, args)?.into_bytes()))
}

/// `w` between reads: the handles that hold the watch, what is
/// watched, this follower's own place in the Kernel's event ring, and
/// the record to list again from.
struct Relist {
    handles: Vec<Handle>,
    targets: Vec<Rd>,
    cursor: u64,
    args: Args,
}

impl Follow for Relist {
    fn more(&mut self, ctx: &dyn Calls) -> Result<Option<Vec<u8>>, Error> {
        let (events, cursor) = ops::since(ctx, self.cursor)?;
        self.cursor = cursor;
        let happened = events
            .iter()
            .filter(|event| self.targets.contains(&event.rd))
            .count();
        let mut bytes = Vec::new();
        for _ in 0..happened {
            bytes.extend(text(ctx, &self.args)?.into_bytes());
        }
        Ok(Some(bytes))
    }

    fn end(&mut self, ctx: &dyn Calls) {
        ops::shut(ctx, &self.handles);
    }
}

/// The targets a run listed. No target is the `cwd`, which an empty
/// path walks to.
fn named(args: &Args) -> Vec<&str> {
    if args.targets.is_empty() {
        vec![""]
    } else {
        args.targets.iter().map(String::as_str).collect()
    }
}

/// Every target's listing in turn, each named first where there is
/// more than one.
fn text(ctx: &dyn Calls, args: &Args) -> Result<String, Error> {
    let targets = named(args);
    let many = targets.len() > 1;
    let mut text = String::new();
    for target in targets {
        let gap = if text.is_empty() { "" } else { "\n" };
        if many {
            let _ = writeln!(text, "{gap}{target}:");
        }
        text.push_str(&listing(ctx, args, target)?);
    }
    Ok(text)
}

/// One target's listing: a Container's children, or the target itself.
fn listing(
    ctx: &dyn Calls,
    args: &Args,
    target: &str,
) -> Result<String, Error> {
    let rd = cell::resolve(ctx, args.cwd, target)?;
    let info = ops::info(ctx, rd)?;
    let listed = if info.flags.contains(Flags::CONTAINER) {
        format::entries(ctx, rd)?
    } else {
        vec![Entry { name: format::named(rd, &info), rd, info }]
    };
    if args.long {
        let mut text = String::new();
        for entry in &listed {
            text.push_str(&format::long_line(
                ctx, entry, args.all, args.color,
            )?);
        }
        return Ok(text);
    }
    let name = |entry: &Entry| {
        let name = format::painted(&entry.name, &entry.info, args.color);
        if args.all { format!("{} {name}", entry.rd.0) } else { name }
    };
    let names: Vec<String> = listed.iter().map(name).collect();
    Ok(format::laid_out(&names, args.width))
}
