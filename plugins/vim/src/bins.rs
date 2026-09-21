//! The names of Vim's `:` line, Drivers under `/self/vim/bin`.
//!
//! A run here is the arm `ex` used to have: one effect, handed to the
//! same `act` a key's effect goes through, and what Vim would have said
//! in its status row as the bytes of the run's output cell. A name
//! prints; it does not fault, and an `Error` is what the tree refused.
//!
//! A declaration is a shape and not a name, shared by matching names,
//! and the rd a call comes in on is which name it is — where a Module
//! whose names each have a shape of their own tells them apart by the
//! api's hash. What a name takes is its declaration and nothing beside
//! it: the range and the bang are Vim's own language and reach a run as
//! the environment its host set, matched by name and type, `from` and
//! `to` being lines from 0 as the engine counts them.

use alloc::vec::Vec;

use reovim_kernel_interface::{Calls, Decl, Error, Rd, Ty};

use crate::plugin::{Editor, named};
use crate::{Effect, Quit, Switch, Vim};

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
    /// `substitute`, its pattern and replacement over a range.
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

reovim_sdk::declare! {
    /// A global selection and the command to run on every marked line.
    api each ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args {
        bang: bool, ranged: bool, from: u64, to: u64, spec: utf8(512)
    }
}

reovim_sdk::declare! {
    /// Whole lines deleted into an optional register.
    api lines ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args { from: u64, to: u64, register: utf8(8) }
}

reovim_sdk::declare! {
    /// Select or close a draft by its name under the Client's buffers.
    api numbered ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args { bang: bool, name: utf8(64) }
}

reovim_sdk::declare! {
    /// `bind`: the keys and the `:` line they run.
    api bound ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args { keys: utf8(64), line: utf8(512) }
}

reovim_sdk::declare! {
    /// `point`: where the cursor or the selection is, and how to
    /// spell it. The range reaches it as the environment its host
    /// set, as `global`'s does.
    api point ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args {
        ranged: bool, from: u64, to: u64, col: bool, relative: bool
    }
}

reovim_sdk::declare! {
    /// `goto`: where in the text to land, in the grammar `point`
    /// writes it. One argument and nothing else; another domain's
    /// locator is said back, never read.
    api located ver 1 {
        0 run(args: Args) -> Rd: READ;
    }
    record Args { locator: utf8(64) }
}

/// The names, in the order a `:` line resolves one, each with the
/// declaration of the shape it takes.
///
/// `:w` is `write` because it stands before `wq`, `:s` is `substitute`
/// before `set`, `:e` is `edit` before `explore`, and `:c` is `cquit`
/// before `cli`, which takes `:cl`. That order is the order they are
/// registered in and the order `children(/self/vim/bin)` lists them,
/// which is where the abbreviation is read from; nothing lists a short
/// form by hand.
pub(crate) fn declared() -> [(&'static str, Decl); 25] {
    let (plain, bang, file) = (plain::decl(), bang::decl(), file::decl());
    [
        ("write", file.clone()),
        ("wq", plain.clone()),
        ("quit", bang.clone()),
        ("xit", plain.clone()),
        ("cquit", plain.clone()),
        ("substitute", subst::decl()),
        ("set", option::decl()),
        ("nohlsearch", plain.clone()),
        ("edit", file.clone()),
        ("explore", file),
        ("cli", bang.clone()),
        ("global", each::decl()),
        ("vglobal", each::decl()),
        ("delete", lines::decl()),
        ("buffer", numbered::decl()),
        ("buffers", plain.clone()),
        ("bnext", plain.clone()),
        ("bprevious", plain.clone()),
        ("bdelete", numbered::decl()),
        ("qall", bang),
        ("wall", plain.clone()),
        ("wqall", plain),
        // Last, after `wqall`: `:b` is `buffer` still and `:bi` is
        // this one, the listing being the whole of the ex rule.
        ("bind", bound::decl()),
        // And after `bind`: `:p` is this one, no other name of Vim's
        // beginning with it, and `:po` is this one too.
        ("point", point::decl()),
        // And after `point`: `:g` is `global` still and `:go` is this
        // one, no other name beginning with those two.
        ("goto", located::decl()),
    ]
}

/// How many arguments of a `:` line the run of `name` fills: the
/// `utf8` fields of the record it takes. Every positional of Vim's
/// own names is one of those and nothing else of theirs is — the
/// range and the bang reach a run as the environment its host set.
/// Zero for a name that is none of Vim's.
pub(crate) fn positionals(name: &str) -> usize {
    let Some((_, decl)) = declared().into_iter().find(|(at, _)| *at == name)
    else {
        return 0;
    };
    let Some(Ty::Record(fields)) =
        decl.methods.first().map(|method| &method.input)
    else {
        return 0;
    };
    fields.iter().filter(|field| matches!(field.ty, Ty::Utf8(_))).count()
}

/// One call on a name of Vim's: the rd it came in on is the name, and
/// the name says which shape to decode the input as. The
/// api the Kernel checked says the same thing, an rd here reporting one
/// declaration and no other, so the rd alone is read.
///
/// # Errors
/// `Unsupported` for an rd none of the names stands at or a number
/// no shape declares, else what the run says.
pub(crate) fn dispatch(
    vim: &Editor,
    ctx: &dyn Calls,
    rd: Rd,
    number: u16,
    input: &[u8],
) -> Result<Vec<u8>, Error> {
    match named(ctx, rd)?.as_str() {
        "wq" | "xit" | "cquit" | "nohlsearch" | "buffers" | "bnext"
        | "bprevious" | "wall" | "wqall" => {
            plain::dispatch(vim, ctx, rd, number, input)
        }
        "quit" | "cli" | "qall" => bang::dispatch(vim, ctx, rd, number, input),
        "write" | "edit" | "explore" => {
            file::dispatch(vim, ctx, rd, number, input)
        }
        "substitute" => subst::dispatch(vim, ctx, rd, number, input),
        "set" => option::dispatch(vim, ctx, rd, number, input),
        "global" | "vglobal" => each::dispatch(vim, ctx, rd, number, input),
        "delete" => lines::dispatch(vim, ctx, rd, number, input),
        "buffer" | "bdelete" => numbered::dispatch(vim, ctx, rd, number, input),
        "bind" => bound::dispatch(vim, ctx, rd, number, input),
        "point" => point::dispatch(vim, ctx, rd, number, input),
        "goto" => located::dispatch(vim, ctx, rd, number, input),
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
            "buffers" => self.did(ctx, rd, |_| Effect::List),
            "bnext" => self.did(ctx, rd, |_| Effect::Switch(Switch::Next)),
            "bprevious" => {
                self.did(ctx, rd, |_| Effect::Switch(Switch::Previous))
            }
            "wall" => self.did(ctx, rd, |_| Effect::SaveAll),
            "wqall" => self.did(ctx, rd, |_| Effect::Quit(Quit::WriteAll)),
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
            "qall" => Effect::Quit(Quit::All(bang)),
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

impl numbered::Api for Editor {
    fn run(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        args: numbered::Args,
    ) -> Result<Rd, Error> {
        let effect = match named(ctx, rd)?.as_str() {
            "buffer" => Effect::Switch(Switch::Named(args.name)),
            "bdelete" => Effect::Close(args.name, args.bang),
            _ => return Err(Error::Unsupported),
        };
        self.did(ctx, rd, |_| effect)
    }
}

impl bound::Api for Editor {
    /// `:bind [KEYS] [LINE]`: KEYS run LINE, and KEYS with no LINE
    /// takes the binding away. There is no `unbind`.
    fn run(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        args: bound::Args,
    ) -> Result<Rd, Error> {
        let bound::Args { keys, line } = args;
        self.did(ctx, rd, move |vim| vim.bind(&keys, &line))
    }
}

impl point::Api for Editor {
    /// `:[range]point [-c] [-r]`: one line, the point, printed and
    /// nothing else. A range is a selection's, as `binding` and the
    /// `:` prompt both set it from the marks `<` and `>`.
    fn run(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        args: point::Args,
    ) -> Result<Rd, Error> {
        let point::Args { ranged, from, to, col, relative } = args;
        let range = ranged.then(|| (line(from), line(to)));
        let said = self.pointed(ctx, col, relative, range);
        self.did(ctx, rd, move |vim| {
            vim.message = said;
            Effect::Nothing
        })
    }
}

impl located::Api for Editor {
    /// `:goto LOCATOR`: the cursor where the locator names, or the
    /// locator said back when this view cannot read it. No range and
    /// no bang: a place is a place.
    fn run(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        args: located::Args,
    ) -> Result<Rd, Error> {
        self.did(ctx, rd, move |vim| vim.goto(&args.locator))
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

impl each::Api for Editor {
    fn run(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        args: each::Args,
    ) -> Result<Rd, Error> {
        let invert = args.bang || named(ctx, rd)? == "vglobal";
        let range = args.ranged.then_some((line(args.from), line(args.to)));
        self.globally(ctx, rd, range, &args.spec, invert)
    }
}

impl lines::Api for Editor {
    fn run(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        args: lines::Args,
    ) -> Result<Rd, Error> {
        self.did(ctx, rd, |vim| {
            vim.delete((line(args.from), line(args.to)), &args.register)
        })
    }
}

/// One end of a range as the engine counts lines.
fn line(at: u64) -> usize {
    usize::try_from(at).unwrap_or(usize::MAX)
}
