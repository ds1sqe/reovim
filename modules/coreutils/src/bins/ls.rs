//! `ls`, a Driver under `/bin`: its declaration and its
//! run. The slots it is served through are the crate's.
//!
//! One run answers with an output cell holding the children of each
//! target by name, sorted: down the columns that fit in `width`, one
//! name a line when `width` is 0, and the long line under `long`. A
//! target that is no Container lists itself. No target at all is the
//! `cwd` the run was given.

use alloc::string::String;
use alloc::vec::Vec;
use alloc::{format, vec};
use core::fmt::Write as _;

use reovim_kernel_interface::{Calls, Error, Flags, Rd};
use reovim_sdk::cell;

use crate::Coreutils;
use crate::format::{self, Entry};
use crate::ops;

reovim_sdk::declare! {
    api ls ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args {
        long: bool,
        all: bool,
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
        let text = text(ctx, &args)?;
        self.cell(ctx, rd, text.into_bytes())
    }
}

/// Every target's listing in turn, each named first where there is
/// more than one. No target is the `cwd`, which an empty path walks to.
fn text(ctx: &dyn Calls, args: &Args) -> Result<String, Error> {
    let empty = vec![String::new()];
    let targets = if args.targets.is_empty() { &empty } else { &args.targets };
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
