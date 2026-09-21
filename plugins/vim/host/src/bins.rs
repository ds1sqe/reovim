//! What a name of Vim's `:` line does when it is run: one effect,
//! handed to the same `act` a key's effect goes through, and what Vim
//! would have said in its status row as the bytes of the run's output
//! cell. A name prints; it does not fault, and an `Error` is what the
//! tree refused.
//!
//! The declarations these run behind are the engine's
//! ([`reovim_plugin_vim::bins`]); the runs are the host's, because
//! every one of them performs an outcome.

use alloc::vec::Vec;

use reovim_kernel_interface::{Calls, Error, Rd};
use reovim_plugin_vim::bins::{
    bang, bound, each, file, lines, located, numbered, option, plain, point,
    subst,
};
use reovim_plugin_vim::{Effect, Quit, Switch, Vim, asked};

use crate::{Editor, named};

/// One call on a name of Vim's: the rd it came in on is the name, and
/// the name says which shape to decode the input as. The
/// api the Kernel checked says the same thing, an rd here reporting one
/// declaration and no other, so the rd alone is read.
///
/// # Errors
/// `Unsupported` for an rd none of the names stands at or a number
/// no shape declares, else what the run says.
pub fn dispatch(
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
            "edit" if held => Effect::Open(asked(path, bang)),
            "edit" => Effect::Reload(bang),
            "explore" => Effect::Explore(asked(path, bang)),
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
            vim.say(said);
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
