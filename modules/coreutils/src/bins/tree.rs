//! `tree`, a Driver under `/bin`: its declaration and its
//! run. The slots it is served through are the crate's.
//!
//! One run answers with an output cell holding the target and what is
//! under it, Containers first and then cells, each by name, ending
//! with the count of what was drawn. `levels` stops the walk that many
//! levels down and 0 does not stop it; an empty target is the `cwd`
//! the run was given, whose path the first line then says.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::fmt::Write as _;

use reovim_kernel_interface::{Calls, Error, Flags, Rd};
use reovim_sdk::cell;
use reovim_sdk::walk::ROOT;

use crate::Coreutils;
use crate::format::{self, Entry};
use crate::ops;

reovim_sdk::declare! {
    api tree ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args { levels: u64, cwd: Rd, color: bool, target: utf8(256) }
}

/// What a run takes, beside the api it belongs to.
pub use tree::Args;
pub(crate) use tree::{decl, dispatch};

/// Names of a path past which the first line says the rd instead, as
/// the depth of a walk is nobody's to trust.
const DEEP: usize = 64;

impl tree::Api for Coreutils {
    fn run(&self, ctx: &dyn Calls, rd: Rd, args: Args) -> Result<Rd, Error> {
        let text = text(ctx, &args)?;
        self.cell(ctx, rd, text.into_bytes())
    }
}

/// The target on its own line, what is under it, and the counts.
fn text(ctx: &dyn Calls, args: &Args) -> Result<String, Error> {
    let rd = cell::resolve(ctx, args.cwd, &args.target)?;
    let info = ops::info(ctx, rd)?;
    let head = if args.target.is_empty() {
        where_is(ctx, rd)?
    } else {
        args.target.clone()
    };
    let mut text = format!("{head}\n");
    let mut count = (0, 0);
    if info.flags.contains(Flags::CONTAINER) {
        let kids = ordered(format::entries(ctx, rd)?);
        let depth = match usize::try_from(args.levels).unwrap_or(usize::MAX) {
            0 => usize::MAX,
            levels => levels,
        };
        let at = Deeper { prefix: String::new(), depth, color: args.color };
        draw(ctx, &kids, &at, &mut text, &mut count)?;
    }
    let _ = write!(text, "\n{} containers, {} cells\n", count.0, count.1);
    Ok(text)
}

/// The path of `rd`, as the first line says where a run with no target
/// stood. One that is not named the whole way up, or stands deeper
/// than [`DEEP`], has no path to show and is `rd:<n>`.
fn where_is(ctx: &dyn Calls, rd: Rd) -> Result<String, Error> {
    let mut names: Vec<String> = Vec::new();
    let mut at = rd;
    while at != ROOT {
        let Some(name) = ops::info(ctx, at)?.name else {
            return Ok(format!("rd:{}", rd.0));
        };
        if names.len() == DEEP {
            return Ok(format!("rd:{}", rd.0));
        }
        names.push(name);
        // `..` is the walk's own move to the parent.
        at = cell::resolve(ctx, at, "..")?;
    }
    names.reverse();
    Ok(format!("/{}", names.join("/")))
}

/// Containers first and then cells, each by name, as a drawing wants
/// them.
fn ordered(mut kids: Vec<Entry>) -> Vec<Entry> {
    let cell = |entry: &Entry| !entry.info.flags.contains(Flags::CONTAINER);
    kids.sort_by(|a, b| {
        cell(a).cmp(&cell(b)).then_with(|| a.name.cmp(&b.name))
    });
    kids
}

/// Where [`draw`] is: what each line starts with, how many levels are
/// left, and whether the names wear their color.
struct Deeper {
    prefix: String,
    depth: usize,
    color: bool,
}

/// `kids` under `at.prefix`, each Container a level deeper.
fn draw(
    ctx: &dyn Calls,
    kids: &[Entry],
    at: &Deeper,
    out: &mut String,
    count: &mut (usize, usize),
) -> Result<(), Error> {
    let prefix = &at.prefix;
    for (i, kid) in kids.iter().enumerate() {
        let last = i + 1 == kids.len();
        let stem = if last { "└── " } else { "├── " };
        let below = if last { "    " } else { "│   " };
        let container = kid.info.flags.contains(Flags::CONTAINER);
        if container {
            count.0 += 1;
        } else {
            count.1 += 1;
        }
        let name = format::painted(&kid.name, &kid.info, at.color);
        let _ = writeln!(out, "{prefix}{stem}{name}");
        if container && at.depth > 1 {
            let deeper = Deeper {
                prefix: format!("{prefix}{below}"),
                depth: at.depth - 1,
                color: at.color,
            };
            let kids = ordered(format::entries(ctx, kid.rd)?);
            draw(ctx, &kids, &deeper, out, count)?;
        }
    }
    Ok(())
}
