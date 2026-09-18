//! `grep`, a Driver under `/bin`: its declaration and its
//! run. The slots it is served through are the crate's.
//!
//! One run answers with an output cell holding every line of every
//! target the pattern is found in. With more than one target each line
//! carries that target's name and a colon, which is the only thing that
//! makes several of them readable. A target that holds no bytes, a
//! Container among them, fails the run and leaves no cell.
//!
//! The dialect is the matcher's, which is Vim's and not POSIX: `a\+`
//! and not `a+`, `\(a\|b\)x` and not `(a|b)x`, and `\c` is how one asks
//! for case-blindness. A pattern the subset has not is `Invalid`,
//! answered before any target is read. Nothing of that is in the
//! declaration, which carries names and types and no prose.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write as _;

use reovim_kernel_interface::{Calls, Error, Rd};
use reovim_pattern::Pattern;
use reovim_sdk::{cell, ops};

use crate::Coreutils;
use crate::format;

reovim_sdk::declare! {
    api grep ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args { cwd: Rd, pattern: utf8(1024), targets: [utf8(256); 64] }
}

/// What a run takes, beside the api it belongs to.
pub use grep::Args;
pub(crate) use grep::{decl, dispatch};

impl grep::Api for Coreutils {
    fn run(&self, ctx: &dyn Calls, rd: Rd, args: Args) -> Result<Rd, Error> {
        let pattern = Pattern::parse(&args.pattern, false, false)
            .ok_or(Error::Invalid)?;
        let mut targets = Vec::new();
        for target in &args.targets {
            targets.push(cell::resolve(ctx, args.cwd, target)?);
        }
        let named = targets.len() > 1;
        let mut out = String::new();
        for target in targets {
            out.push_str(&found(ctx, &pattern, target, named)?);
        }
        self.cell(ctx, rd, out.into_bytes())
    }
}

/// Every line of `target` the pattern is found in, under the target's
/// own name where the run has more than one. A target that is not text
/// is no place to look for a line.
fn found(
    ctx: &dyn Calls,
    pattern: &Pattern,
    target: Rd,
    named: bool,
) -> Result<String, Error> {
    let prefix = if named { prefix(ctx, target)? } else { String::new() };
    let bytes = ops::bytes(ctx, target)?;
    let text = String::from_utf8(bytes).map_err(|_| Error::Invalid)?;
    let mut out = String::new();
    for line in text.split_terminator('\n') {
        if pattern.find(line, 0).is_some() {
            let _ = writeln!(out, "{prefix}{line}");
        }
    }
    Ok(out)
}

/// The name `target` shows under and a colon, which every line of it
/// stands after.
fn prefix(ctx: &dyn Calls, target: Rd) -> Result<String, Error> {
    let info = ops::info(ctx, target)?;
    Ok(format!("{}:", format::named(target, &info)))
}
