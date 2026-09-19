//! The eleven names of Vim's `:` line, Drivers under `/self/vim/bin`.
//!
//! A run here is the arm `ex` used to have: one effect, handed to the
//! same `act` a key's effect goes through, and what Vim would have said
//! in its status row as the bytes of the run's output cell. A name
//! prints; it does not fault, and an `Error` is what the tree refused.
//!
//! A declaration is a shape and not a name, so the eleven have five,
//! and the rd a call comes in on is which name it is — where a Module
//! whose names each have a shape of their own tells them apart by the
//! api's hash. What a name takes is its declaration and nothing beside
//! it: the range and the bang are Vim's own language and reach a run as
//! the environment its host set, matched by name and type, `from` and
//! `to` being lines from 0 as the engine counts them.

use alloc::vec::Vec;

use reovim_kernel_interface::{Calls, Decl, Error, Rd};

use crate::plugin::{Editor, named};
use crate::{Effect, Quit, Vim};

reovim_sdk::declare! {
    /// A name that reads nothing after it: `wq`, `xit`, `cquit`,
    /// `nohlsearch`.
    api plain ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args {}
}

reovim_sdk::declare! {
    /// A name that reads a bang and nothing else: `quit`, `cli`.
    api bang ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args { bang: bool }
}

reovim_sdk::declare! {
    /// A name that reads a bang and a path: `write`, `edit`,
    /// `explore`.
    api file ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args { bang: bool, path: utf8(512) }
}

reovim_sdk::declare! {
    /// `substitute`, the one name that reads the range.
    api subst ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args { from: u64, to: u64, spec: utf8(256) }
}

reovim_sdk::declare! {
    /// `set`, whose options are the rest of the line.
    api option ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args { options: utf8(128) }
}

/// The eleven, in the order a `:` line resolves one, each with the
/// declaration of the shape it takes.
///
/// `:w` is `write` because it stands before `wq`, `:s` is `substitute`
/// before `set`, `:e` is `edit` before `explore`, and `:c` is `cquit`
/// before `cli`, which takes `:cl`. That order is the order they are
/// registered in and the order `children(/self/vim/bin)` lists them,
/// which is where the abbreviation is read from; nothing lists a short
/// form by hand.
pub(crate) fn declared() -> [(&'static str, Decl); 11] {
    let (plain, bang, file) = (plain::decl(), bang::decl(), file::decl());
    [
        ("write", file.clone()),
        ("wq", plain.clone()),
        ("quit", bang.clone()),
        ("xit", plain.clone()),
        ("cquit", plain.clone()),
        ("substitute", subst::decl()),
        ("set", option::decl()),
        ("nohlsearch", plain),
        ("edit", file.clone()),
        ("explore", file),
        ("cli", bang),
    ]
}

/// One call on a name of Vim's: the rd it came in on is the name, and
/// the name says which of the five shapes to decode the input as. The
/// api the Kernel checked says the same thing, an rd here reporting one
/// declaration and no other, so the rd alone is read.
///
/// # Errors
/// `Unsupported` for an rd none of the eleven stands at or a number
/// none of the five declares, else what the run says.
pub(crate) fn dispatch(
    vim: &Editor,
    ctx: &dyn Calls,
    rd: Rd,
    number: u16,
    input: &[u8],
) -> Result<Vec<u8>, Error> {
    match named(ctx, rd)?.as_str() {
        "wq" | "xit" | "cquit" | "nohlsearch" => {
            plain::dispatch(vim, ctx, rd, number, input)
        }
        "quit" | "cli" => bang::dispatch(vim, ctx, rd, number, input),
        "write" | "edit" | "explore" => {
            file::dispatch(vim, ctx, rd, number, input)
        }
        "substitute" => subst::dispatch(vim, ctx, rd, number, input),
        "set" => option::dispatch(vim, ctx, rd, number, input),
        _ => Err(Error::Unsupported),
    }
}

impl plain::Api for Editor {
    /// `:wq` and `:x` write and leave, `:cq` leaves with 1 so the
    /// caller uses nothing, and `:noh` stops marking the last search.
    fn run(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        _: plain::Args,
    ) -> Result<Rd, Error> {
        match named(ctx, rd)?.as_str() {
            "wq" | "xit" => self.did(ctx, rd, |_| Effect::Quit(Quit::Write)),
            "cquit" => self.did(ctx, rd, |_| Effect::Quit(Quit::Abort)),
            "nohlsearch" => self.did(ctx, rd, Vim::nohlsearch),
            _ => Err(Error::Unsupported),
        }
    }
}

impl bang::Api for Editor {
    /// `:q[!]`, refused with E37 while the draft is unsaved unless the
    /// bang says otherwise, and `:cli[!]`, the shell in this region.
    fn run(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        args: bang::Args,
    ) -> Result<Rd, Error> {
        let bang = args.bang;
        let effect = match named(ctx, rd)?.as_str() {
            "quit" if bang => Effect::Quit(Quit::Force),
            "quit" => Effect::Quit(Quit::Ask),
            "cli" => Effect::Cli(bang),
            _ => return Err(Error::Unsupported),
        };
        self.did(ctx, rd, |_| effect)
    }
}

impl file::Api for Editor {
    /// `:w[!] [PATH]`, `:e[!] PATH` and `:exp[lore][!] [DIR]`. The path
    /// is the rest of the line, so `:w my file` is one name, and it is
    /// resolved where the editor stands.
    fn run(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        args: file::Args,
    ) -> Result<Rd, Error> {
        let file::Args { bang, path } = args;
        let held = !path.is_empty();
        let effect = match named(ctx, rd)?.as_str() {
            "write" if held => Effect::SaveAs(path, bang),
            "write" => Effect::Save(bang),
            "edit" if held => Effect::Open(path, bang),
            "edit" => Effect::Reload(bang),
            "explore" => Effect::Explore(held.then_some(path), bang),
            _ => return Err(Error::Unsupported),
        };
        self.did(ctx, rd, |_| effect)
    }
}

impl subst::Api for Editor {
    /// `:[range]s/PAT/REP/[flags]`, over the lines the range named. The
    /// spec is the rest of the line, spaces and all, so `:%s/^/> /`
    /// reaches Vim's own parser whole.
    fn run(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        args: subst::Args,
    ) -> Result<Rd, Error> {
        let subst::Args { from, to, spec } = args;
        let lines = (line(from), line(to));
        self.did(ctx, rd, move |vim| {
            vim.substitute(lines, spec.strip_prefix('/').unwrap_or(&spec))
        })
    }
}

impl option::Api for Editor {
    /// `:set OPTION...`, whose options are one argument and split
    /// themselves.
    fn run(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        args: option::Args,
    ) -> Result<Rd, Error> {
        self.did(ctx, rd, move |vim| vim.set(&args.options))
    }
}

/// One end of a range as the engine counts lines.
fn line(at: u64) -> usize {
    usize::try_from(at).unwrap_or(usize::MAX)
}
