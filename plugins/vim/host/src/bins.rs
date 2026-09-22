//! What a name of Vim's `:` line does when it is run: one effect,
//! handed to the same `act` a key's effect goes through, and what Vim
//! would have said in its status row as the bytes of the run's output
//! cell. A name prints; it does not fault, and an `Error` is what the
//! tree refused.
//!
//! The declarations these run behind are the engine's
//! ([`reovim_plugin_vim::bins`]); the runs are the host's, because
//! every one of them performs an outcome.

use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;

use reovim_kernel_interface::{Calls, Error, Rd};
use reovim_plugin_vim::bins::{
    bang, bound, canonical, each, file, lines, located, numbered, option,
    plain, point, sought, subst,
};
use reovim_plugin_vim::{Effect, Quit, Switch, Vim, asked};

use crate::{E140, E481, Editor, named};

/// One call on a name of Vim's: the rd it came in on is a full name or
/// one of its abbreviations, [`canonical`] reading either back to the
/// one the api the Kernel checked reports, an rd here reporting one
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
    match canonical(&named(ctx, rd)?) {
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
        "search" => sought::dispatch(vim, ctx, rd, number, input),
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
        match canonical(&named(ctx, rd)?) {
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
        let effect = match canonical(&named(ctx, rd)?) {
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
    /// `:[range]w[!] [PATH]`, `:e[!] PATH` and `:exp[lore][!] [DIR]`.
    /// The path is the rest of the line, so `:w my file` is one name,
    /// and it is resolved where the editor stands.
    ///
    /// An address makes `write` another door (`wrote`, below) and is no
    /// part of `edit` or `explore` at all; it is resolved ahead of
    /// `did`, as `point`'s is, a write of its own being the host's
    /// call and the message it leaves this run's own to print.
    fn run(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        args: file::Args,
    ) -> Result<Rd, Error> {
        let file::Args { bang, address, path } = args;
        let name = named(ctx, rd)?;
        let lines = match addressed(&self.vim, &address) {
            Ok(lines) => lines,
            Err(text) => return self.said(ctx, rd, text),
        };
        if let Some(lines) = lines {
            return wrote(self, ctx, rd, canonical(&name), lines, &path, bang);
        }
        let held = !path.is_empty();
        let effect = match canonical(&name) {
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

/// A name of the `file` shape run with an address on it, as nvim
/// 0.12.5 answers one: `:{range}w PATH` writes the lines the address
/// named, and nothing else, to PATH; a partial draft over the draft's
/// own file, named or not, is `E140` without a bang and a write with
/// one; and a range on `edit` or `explore` is `E481`.
///
/// Nothing here publishes the draft or sends it an edit, so its
/// source and its modified flag stand as they were: nvim's
/// `:2,3w out` leaves an unsaved draft unsaved.
fn wrote(
    editor: &Editor,
    ctx: &dyn Calls,
    rd: Rd,
    name: &str,
    lines: (usize, usize),
    path: &str,
    bang: bool,
) -> Result<Rd, Error> {
    let text = editor.vim.borrow().text_of(lines);
    match name {
        "write" if bang && path.is_empty() => {
            editor.lines_over(ctx, bang, text)?;
        }
        "write" if bang => editor.lines_to(ctx, path, bang, text)?,
        // nvim's E140 is about a partial draft over *the draft's own
        // file*, which a name reaches as well as an empty one; another
        // file that is there already is E13, and the bang above is the
        // whole of the difference.
        "write" if path.is_empty() || editor.own(ctx, path)? => {
            editor.fails(E140);
        }
        "write" => editor.lines_to(ctx, path, bang, text)?,
        "edit" | "explore" => editor.fails(E481),
        _ => return Err(Error::Unsupported),
    }
    let said = editor.vim.borrow_mut().saying();
    editor.said(ctx, rd, said)
}

impl numbered::Api for Editor {
    fn run(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        args: numbered::Args,
    ) -> Result<Rd, Error> {
        let effect = match canonical(&named(ctx, rd)?) {
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
    ///
    /// The address is resolved ahead of `did`, which `pointed`
    /// itself must stand outside of for `ctx`'s own calls; an address
    /// that fails is said through `said`, `point` printing
    /// and never faulting.
    fn run(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        args: point::Args,
    ) -> Result<Rd, Error> {
        let point::Args { address, col, relative } = args;
        let range = match addressed(&self.vim, &address) {
            Ok(range) => range,
            Err(text) => return self.said(ctx, rd, text),
        };
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

impl sought::Api for Editor {
    /// `:search [--backward] [PATTERN]`: the `/` line by another
    /// door. The pattern is the rest of the line, verbatim, so
    /// `:search a b` looks for `a b`, and with none the prompt opens
    /// — `bind <C-s> search` makes `C-s` the `/` key and `bind <C-r>
    /// search --backward` the `?` key.
    fn run(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        args: sought::Args,
    ) -> Result<Rd, Error> {
        let sought::Args { backward, pattern } = args;
        self.did(ctx, rd, move |vim| vim.searching(&pattern, backward))
    }
}

impl subst::Api for Editor {
    /// `:[range]s/PAT/REP/[flags]`, over the lines the address named,
    /// resolved once inside `did` so a failure's message
    /// stands through `ran`'s own clear. The spec is the rest of the
    /// line, spaces and all, so `:%s/^/> /` reaches Vim's own parser
    /// whole.
    fn run(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        args: subst::Args,
    ) -> Result<Rd, Error> {
        let subst::Args { address, spec } = args;
        self.did(ctx, rd, move |vim| {
            let lines = match vim.lines_of(&address) {
                Ok(lines) => lines,
                Err(effect) => return effect,
            };
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
    /// `:[range]g[!]/PAT/CMD`: the address empty means the whole
    /// buffer, `:global`'s own default and not the cursor line
    /// [`Vim::lines_of`] would answer for one — resolved here and not
    /// inside `globally`'s own run, which is not `did`
    /// and clears no message of its own to lose one to.
    fn run(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        args: each::Args,
    ) -> Result<Rd, Error> {
        let invert = args.bang || canonical(&named(ctx, rd)?) == "vglobal";
        let range = match addressed(&self.vim, &args.address) {
            Ok(range) => range,
            Err(text) => return self.said(ctx, rd, text),
        };
        self.globally(ctx, rd, range, &args.spec, invert)
    }
}

/// `address`'s lines, `None` for an empty one — `:global`'s own
/// default, the whole buffer, and never the cursor line
/// [`Vim::lines_of`] would answer for it. `Err` is the message an
/// address that failed to resolve already said, said through
/// [`Editor::said`] and not [`Editor::did`], which would lose it to
/// `ran`'s own clear.
fn addressed(
    vim: &RefCell<Vim>,
    address: &str,
) -> Result<Option<(usize, usize)>, String> {
    if address.is_empty() {
        return Ok(None);
    }
    let mut vim = vim.borrow_mut();
    vim.lines_of(address)
        .map_or_else(|_| Err(vim.saying()), |range| Ok(Some(range)))
}

impl lines::Api for Editor {
    /// `:[range]d[elete] [register]`, the address resolved inside
    /// `did` so a failure's message stands through `ran`'s own
    /// clear.
    fn run(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        args: lines::Args,
    ) -> Result<Rd, Error> {
        self.did(ctx, rd, move |vim| {
            let lines = match vim.lines_of(&args.address) {
                Ok(lines) => lines,
                Err(effect) => return effect,
            };
            vim.delete(lines, &args.register)
        })
    }
}
