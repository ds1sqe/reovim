//! Vim as a Module a Client loads: what the keys asked for, done.
//!
//! The engine below is pure: a key becomes an `Effect`, which has
//! already happened to Vim's own copy of the text; here it becomes
//! calls. An edit is a `replace`, or an `amend` when it joins the
//! insert under way; undo and redo move the Buffer's head and the text
//! is read back; a save publishes it to its source; a put reads the
//! clipboard rd and a `"+` yank writes it; a quit writes
//! `/self/dev/exit`, and what all of it looks like is the View that
//! ends the frame.
//!
//! The cells served are `view` and Vim's own state. A whole read of
//! `view` is one frame: the keys Vim has not taken, each run; what
//! changed on the Buffer since the last frame; the rows the region has
//! now; then the View. The rest — the mode, the keys of a command that
//! has not fired, the registers — are made at the read out of what the
//! engine holds, so nothing is stored twice and nothing goes stale. No
//! borrow of the engine outlives an op, so a Driver of Vim's own that
//! a run re-enters finds the state free.

use alloc::collections::BTreeMap;
use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::{OnceCell, Ref, RefCell};

use reovim_client_api::{Frames, NONE, Region, count, part};
use reovim_client_sh::{self as sh, Ran, Shell};
use reovim_kernel_interface::{
    Booted, Calls, Decl, Driver, DriverId, Error, Event, Flags, Handle, Hash,
    Kind, Module, Rd, Reply, Rights,
};
use reovim_sdk::dynamic::Value;
use reovim_sdk::walk::ROOT;
use reovim_sdk::{cell, decode, encode, ops};
use reovim_text_api::{history, open as text_open, publish, text};

use crate::command::{Typed, options, typed};
use crate::{Edit, Effect, Ex, Quit, Vim, bins};

/// What `:q` says to unsaved changes.
const E37: &str = "E37: No write since last change (add ! to override)";
/// What a write says on a Buffer over nothing.
const E32: &str = "E32: No file name";
/// What a write says when the file it would make is there already.
const E13: &str = "E13: File exists (add ! to override)";
/// What a write says when the host would not make the file.
const E212: &str = "E212: Can't open file for writing";
/// What a `:` line says of a first token neither Vim's own list nor
/// PATH has.
const E492: &str = "E492: Not an editor command:";
/// What `:explore` says to a name that is not on disk: Vim's word for
/// a directory it cannot find, the name as the person typed it.
const E344: &str = "E344: Can't find directory";
/// What `:edit` says to one: the same, for a file.
const E345: &str = "E345: Can't find file";
/// What undo and redo say at the ends of the history.
const OLDEST: &str = "Already at oldest change";
const NEWEST: &str = "Already at newest change";

/// The one Driver of this Module: every rd it registers is served by
/// it, its own cells and the output of every run alike.
const DRIVER: DriverId = DriverId(0);

/// Vim at a Client: the engine behind one `RefCell`, and the cells
/// it serves.
#[derive(Default)]
pub struct Editor {
    vim: RefCell<Vim>,
    /// The cells of `/self/dev`, found once at `start`: they never
    /// change, so no borrow of the engine is held over an op.
    region: OnceCell<Region>,
    /// The rds under Vim's home, as `start` was answered with them.
    cells: OnceCell<Cells>,
    view: Frames,
    /// What the runs of Vim's own names printed, until each is read
    /// and closed.
    outputs: cell::Cells,
}

/// What a read of one of Vim's own rds is a read of: the rds `start`
/// registered under Vim's home.
struct Cells {
    view: Rd,
    mode: Rd,
    pending: Rd,
    /// One rd per register that can exist, by its name.
    registers: BTreeMap<Rd, char>,
    /// `/self/vim/bin`, whose listing is the order a `:` line resolves
    /// a name in.
    bin: Rd,
}

/// Vim, ready for a Client to load.
#[must_use]
pub fn boot() -> Booted {
    Editor::new().booted()
}

impl Editor {
    /// One, which a table gives a Client and a test reads back.
    #[must_use]
    pub fn new() -> Rc<Self> {
        Rc::new(Self::default())
    }

    /// Its Module and its one Driver, for a table to give.
    #[must_use]
    pub fn booted(self: &Rc<Self>) -> Booted {
        Booted { module: self.clone(), drivers: alloc::vec![self.clone()] }
    }

    /// What the engine holds now, for a test to judge what the keys
    /// did: no borrow of it outlives an op, so this one is free.
    ///
    /// # Panics
    /// While a frame of its own holds it, which no caller of this can
    /// be inside.
    #[must_use]
    pub fn engine(&self) -> Ref<'_, Vim> {
        self.vim.borrow()
    }
}

impl Module for Editor {
    /// A Buffer of its own over what the region was opened on, read
    /// and written through a handle it keeps, the clipboard found if
    /// there is one, and the text shown from the line it was told to
    /// begin at.
    fn start(&self, ctx: &dyn Calls, home: Rd) -> Result<(), Error> {
        let cells = registered(ctx, home)?;
        let region = Region::of(ctx)?;
        let ask = region.ask(ctx)?;
        let dir = region.cwd(ctx)?;
        let source = (ask.target != NONE).then_some(ask.target);
        let buffer = buffer_over(ctx, source)?;
        let rights = Rights::READ.union(Rights::WRITE);
        let h = ctx.open(buffer, rights)?;
        // The fs Module mirrors the directory the editor runs over, so
        // every name of the person's is under it and a leading `/` is
        // its root. A tree with no mirror leaves the editor at the
        // tree's own root, which is a place to stand.
        let root = cell::resolve(ctx, ROOT, "/module/fs/root").unwrap_or(ROOT);
        let cwd = cell::resolve(ctx, root, &dir).unwrap_or(root);
        let mut shell = Shell::at(cwd, ops::where_is(ctx, cwd)?);
        ranged(&mut shell, 0, 0, false);
        let clip = cell::resolve(ctx, ROOT, "/self/dev/clipboard").ok();
        {
            let mut vim = self.vim.borrow_mut();
            vim.buffer = Some(buffer);
            vim.h = Some(h);
            vim.source = source;
            vim.name = (!ask.name.is_empty()).then(|| ask.name.clone());
            vim.dir = dir;
            vim.root = Some(root);
            vim.cwd = Some(cwd);
            vim.shell = shell;
            vim.clip = clip;
        }
        let _ = self.cells.set(cells);
        let _ = self.region.set(region);
        self.sized(ctx)?;
        let bytes = ops::whole(ctx, h)?;
        self.vim.borrow_mut().opened(&bytes, ask.line);
        self.announce(ctx)?;
        // Where the stream stands once the Buffer is open and read: a
        // cursor of 0 against a ring that never reached it is one
        // Overflow, and what opening it left is already in the text.
        let (_, cursor) = ops::since(ctx, 0)?;
        self.vim.borrow_mut().cursor_at = cursor;
        Ok(())
    }
}

impl Driver for Editor {
    fn open(&self, _: &dyn Calls, rd: Rd, _: Rights) -> Reply<u64> {
        Reply::ok(rd.0)
    }

    /// A whole read of `view` is one frame: the work at its head, its
    /// bytes to their end. Every other cell is a reading of the state,
    /// made here and kept nowhere, or the output of a run.
    fn read(
        &self,
        ctx: &dyn Calls,
        h: u64,
        at: u64,
        len: usize,
    ) -> Reply<Vec<u8>> {
        let Some(cells) = self.cells.get() else {
            return Reply::err(Error::Closed);
        };
        let rd = Rd(h);
        if rd == cells.view {
            return self.view.read(at, len, || self.framed(ctx));
        }
        let output = || self.outputs.read(ctx, rd, at, len).into();
        self.held(cells, rd)
            .map_or_else(output, |held| Reply::ok(part(&held, at, len)))
    }

    /// One of Vim's own names run: the rd called on is which name.
    fn call(
        &self,
        ctx: &dyn Calls,
        h: u64,
        _: Hash,
        number: u16,
        input: &[u8],
    ) -> Reply<Vec<u8>> {
        bins::dispatch(self, ctx, Rd(h), number, input).into()
    }

    fn close(&self, ctx: &dyn Calls, h: u64) -> Reply<()> {
        self.outputs.close(ctx, Rd(h)).into()
    }
}

impl Editor {
    /// What a cell of Vim's state reads, made out of the engine at the
    /// read: the borrow is this statement's, and Vim's own frame has
    /// returned by the time another Plugin asks. `None` for an rd that
    /// is no cell of the state's.
    fn held(&self, cells: &Cells, rd: Rd) -> Option<Vec<u8>> {
        let vim = self.vim.borrow();
        let read = if rd == cells.mode {
            format!("{}\n", vim.mode().name())
        } else if rd == cells.pending {
            vim.showcmd()
        } else {
            vim.stored(*cells.registers.get(&rd)?)
        };
        Some(read.into_bytes())
    }

    /// The rds `start` registered; `Closed` before it has.
    fn cells(&self) -> Result<&Cells, Error> {
        self.cells.get().ok_or(Error::Closed)
    }

    /// The cells of `/self/dev` this Plugin holds; `Closed` before
    /// `start` has found them.
    fn region(&self) -> Result<&Region, Error> {
        self.region.get().ok_or(Error::Closed)
    }

    /// One frame: every key it has not taken, run; what moved on the
    /// Buffer since the last one; the rows the region has now; the
    /// View.
    fn framed(&self, ctx: &dyn Calls) -> Result<Vec<u8>, Error> {
        let keys = self.region()?.keys(ctx)?;
        for key in keys {
            let effect = self.vim.borrow_mut().key(key);
            self.act(ctx, effect)?;
            self.write_clipped(ctx)?;
        }
        self.changed(ctx)?;
        let rows = self.sized(ctx)?;
        Ok(encode(&self.vim.borrow_mut().view(rows)))
    }

    /// A Modify on the Buffer, or a ring that overflowed: the text is
    /// read again, which is how another Client's write arrives.
    fn changed(&self, ctx: &dyn Calls) -> Result<(), Error> {
        let cursor = self.vim.borrow().cursor_at;
        let (events, next) = ops::since(ctx, cursor)?;
        self.vim.borrow_mut().cursor_at = next;
        let moved = |e: &Event| matches!(e.kind, Kind::Modify | Kind::Overflow);
        if !events.iter().any(moved) {
            return Ok(());
        }
        let bytes = ops::whole(ctx, self.handle()?)?;
        self.vim.borrow_mut().loaded(&bytes);
        Ok(())
    }

    /// The rows the region has now, as `windows/0/size` says.
    fn sized(&self, ctx: &dyn Calls) -> Result<usize, Error> {
        let rows = count(self.region()?.size(ctx)?.rows);
        self.vim.borrow_mut().rows = rows;
        Ok(rows)
    }

    /// The Buffer `start` opened; `Closed` before it has.
    fn handle(&self) -> Result<Handle, Error> {
        self.vim.borrow().h.ok_or(Error::Closed)
    }

    /// `message` until the next key.
    fn says(&self, message: &str) {
        self.vim.borrow_mut().message = String::from(message);
    }

    /// What `effect` asks of the Client.
    fn act(&self, ctx: &dyn Calls, effect: Effect) -> Result<(), Error> {
        match effect {
            Effect::Nothing | Effect::Moved => Ok(()),
            Effect::Edit(edit) => self.send(ctx, edit),
            Effect::Edits(edits) => self.send_each(ctx, edits),
            Effect::Undo => self.walk(ctx, true),
            Effect::Redo => self.walk(ctx, false),
            Effect::Save(bang) => self.save(ctx, bang).map(|_| ()),
            Effect::SaveAs(name, bang) => {
                self.save_as(ctx, &name, bang).map(|_| ())
            }
            Effect::Clipboard => self.pasting(ctx),
            Effect::Quit(how) => self.quit(ctx, how),
            Effect::Explore(name, bang) => {
                self.explore(ctx, name.as_deref(), bang)
            }
            Effect::Open(name, bang) => self.edited(ctx, &name, bang),
            Effect::Line(ex) => self.ran(ctx, &ex),
            Effect::Cli(bang) => self.cli(ctx, bang),
            Effect::Complete(line) => self.completing(ctx, &line),
        }
    }

    /// One of Vim's own names run: the effect it asks for, with the
    /// bookkeeping a key's own effect gets, then done, and what Vim
    /// would have said in its status row as the bytes of the run's
    /// output cell.
    ///
    /// The borrow of the engine is that block's and is gone before
    /// anything is submitted, which is what lets a run of one of these
    /// come back into this Plugin while the key that typed the line is
    /// still on the stack.
    ///
    /// # Errors
    /// Whatever the effect's own calls fail with, and as
    /// `cell::Cells::open`.
    pub(crate) fn did(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        made: impl FnOnce(&mut Vim) -> Effect,
    ) -> Result<Rd, Error> {
        let effect = {
            let mut vim = self.vim.borrow_mut();
            vim.message.clear();
            let before = vim.position();
            let effect = made(&mut vim);
            vim.changing(before, &effect);
            vim.settle(&effect);
            effect
        };
        self.act(ctx, effect)?;
        let said = core::mem::take(&mut self.vim.borrow_mut().message);
        self.outputs.open(ctx, DRIVER, rd, said.into_bytes())
    }

    /// The name a first token stands for: the first under Vim's own
    /// `bin`, in the order the tree lists them, that it begins. `None`
    /// for an empty token and for one no name begins, which is the
    /// line the shell is handed.
    ///
    /// This is Vim's ex rule, and the listing is the whole of it: no
    /// table stands beside the declarations. A name under PATH has
    /// none of it, the shell resolving by exact name.
    fn expanded(
        &self,
        ctx: &dyn Calls,
        token: &str,
    ) -> Result<Option<String>, Error> {
        if token.is_empty() {
            return Ok(None);
        }
        let under = ops::children(ctx, self.cells()?.bin)?;
        let held = |rd: Rd| named(ctx, rd).ok();
        let mut listed = under.into_iter().filter_map(held);
        Ok(listed.find(|name| name.starts_with(token)))
    }

    /// `edit` as a new revision, or into the head one when it joins
    /// the insert under way.
    fn send(&self, mut ctx: &dyn Calls, edit: Edit) -> Result<(), Error> {
        let Edit { at, len, text, join } = edit;
        let edit = text::Edit { at: at as u64, len: len as u64, text };
        let mut call = text::Call(&mut ctx, self.handle()?);
        if join { call.amend(edit) } else { call.replace(edit) }
    }

    /// Those edits, in order, as a repeat replayed them.
    fn send_each(
        &self,
        ctx: &dyn Calls,
        edits: Vec<Edit>,
    ) -> Result<(), Error> {
        for edit in edits {
            self.send(ctx, edit)?;
        }
        Ok(())
    }

    /// Head one revision back or on, the text read again after it; the
    /// end of the history is a message.
    fn walk(&self, mut ctx: &dyn Calls, back: bool) -> Result<(), Error> {
        let h = self.handle()?;
        let mut call = history::Call(&mut ctx, h);
        let (moved, end) =
            if back { (call.undo(), OLDEST) } else { (call.redo(), NEWEST) };
        match moved {
            Ok(()) => {}
            Err(Error::Invalid) => {
                self.says(end);
                return Ok(());
            }
            Err(error) => return Err(error),
        }
        let bytes = ops::whole(ctx, h)?;
        self.vim.borrow_mut().loaded(&bytes);
        Ok(())
    }

    /// A Buffer over nothing that was opened for a file says so on
    /// the first frame, as nvim does; the directory it is to go in is
    /// not there either when it says `[New DIRECTORY]`.
    fn announce(&self, ctx: &dyn Calls) -> Result<(), Error> {
        let (name, dir) = {
            let vim = self.vim.borrow();
            (vim.name.clone(), vim.dir.clone())
        };
        let Some(name) = name else { return Ok(()) };
        // The ask's name is under the mirror's own root, wherever the
        // editor stands, so it is read from there.
        let at = rooted(name.rsplit_once('/').map_or("", |(at, _)| at));
        let word =
            if self.directory(ctx, &at)? { "[New]" } else { "[New DIRECTORY]" };
        let said = as_given(&dir, &name);
        self.vim.borrow_mut().message = format!("\"{said}\" {word}");
        Ok(())
    }

    /// The head published to its source; a Buffer over nothing writes
    /// the file it was opened for, and has no file name without one.
    /// False when it said why it did not write.
    fn save(&self, mut ctx: &dyn Calls, bang: bool) -> Result<bool, Error> {
        let (source, name) = {
            let vim = self.vim.borrow();
            (vim.source, vim.name.clone())
        };
        if source.is_some() {
            self.fixing(ctx)?;
            publish::Call(&mut ctx, self.handle()?).save()?;
            return Ok(true);
        }
        let Some(name) = name else {
            self.says(E32);
            return Ok(false);
        };
        self.written_to(ctx, &rooted(&name), bang)
    }

    /// `:w NAME`: the head written to that file, `NAME` resolved
    /// where the editor stands. Its own name is a save,
    /// and a name that is not on disk is made first. A Buffer over
    /// nothing goes on as that file's, its history starting there.
    fn save_as(
        &self,
        ctx: &dyn Calls,
        name: &str,
        bang: bool,
    ) -> Result<bool, Error> {
        let Some(target) = self.at(ctx, name)? else {
            return self.written_to(ctx, name, bang);
        };
        if Some(target) == self.vim.borrow().source {
            return self.save(ctx, bang);
        }
        self.published_to(ctx, target)
    }

    /// The file `path` names made under the root and the head written
    /// to it: every missing directory on the way by `mkdir`, then
    /// `create`. A file that appeared since is E13 unless `bang` says
    /// to write over it; anything that refuses to make the file, the
    /// host and a file where a directory is wanted alike, is E212.
    fn written_to(
        &self,
        ctx: &dyn Calls,
        path: &str,
        bang: bool,
    ) -> Result<bool, Error> {
        match self.making(ctx, path, bang) {
            Ok(Some(target)) => self.published_to(ctx, target),
            Ok(None) => {
                self.says(E13);
                Ok(false)
            }
            // Refused, or asked of an rd that makes nothing: either
            // way there is no file to write.
            Err(_) => {
                self.says(E212);
                Ok(false)
            }
        }
    }

    /// The head written to `target`, which a Buffer over nothing goes
    /// on as, watched by its Client from then on.
    fn published_to(
        &self,
        mut ctx: &dyn Calls,
        target: Rd,
    ) -> Result<bool, Error> {
        self.fixing(ctx)?;
        publish::Call(&mut ctx, self.handle()?).save_as(target)?;
        let mut vim = self.vim.borrow_mut();
        vim.source = vim.source.or(Some(target));
        Ok(true)
    }

    /// The newline `fixendofline` writes at the end of a last line
    /// that has none, before the write itself.
    fn fixing(&self, ctx: &dyn Calls) -> Result<(), Error> {
        let edit = self.vim.borrow_mut().fixed_eol();
        edit.map_or(Ok(()), |edit| self.send(ctx, edit))
    }

    /// A put waiting for the clipboard: what it holds, then the put
    /// itself, which is an effect like any other.
    fn pasting(&self, ctx: &dyn Calls) -> Result<(), Error> {
        let text = self.clipboard(ctx)?;
        let effect = self.vim.borrow_mut().pasted(text);
        self.act(ctx, effect)
    }

    /// What the clipboard holds, through its rd; `None` when the
    /// Client serves no cell or the machine has no tool.
    fn clipboard(&self, ctx: &dyn Calls) -> Result<Option<String>, Error> {
        let Some(rd) = self.vim.borrow().clip else { return Ok(None) };
        match ops::bytes(ctx, rd) {
            Ok(bytes) => Ok(Some(String::from_utf8_lossy(&bytes).into_owned())),
            Err(Error::Unsupported | Error::Busy) => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// What a `"+` yank left for the clipboard, written to its rd; no
    /// cell or no tool is a message.
    fn write_clipped(&self, ctx: &dyn Calls) -> Result<(), Error> {
        let (text, clip) = {
            let mut vim = self.vim.borrow_mut();
            (vim.clipped(), vim.clip)
        };
        let Some(text) = text else { return Ok(()) };
        let unsupported = Err(Error::Unsupported);
        let written =
            clip.map_or(unsupported, |rd| ops::put(ctx, rd, text.into_bytes()));
        match written {
            Ok(_) => Ok(()),
            Err(Error::Unsupported) => {
                self.says("clipboard: no tool on PATH");
                Ok(())
            }
            Err(Error::Busy) => {
                self.says("clipboard: the tool failed");
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    /// `:q` while the Buffer holds what its source has not; `:q!` and
    /// `ZQ` whatever it holds; `:wq`, `:x` and `ZZ` written first,
    /// which over nothing leaves an untouched Buffer and makes the
    /// file of a changed one, or is E32 without a name; `:cq` with 1,
    /// so the caller uses nothing.
    fn quit(&self, ctx: &dyn Calls, how: Quit) -> Result<(), Error> {
        let unsaved = self.unsaved(ctx)?;
        if how == Quit::Ask && unsaved {
            self.says(E37);
            return Ok(());
        }
        let code = match how {
            Quit::Abort => Some(1),
            Quit::Write => self.written(ctx, unsaved)?,
            _ => Some(0),
        };
        // The write said why it did not happen; the editor stays.
        let Some(code) = code else { return Ok(()) };
        self.region()?.ends(ctx, code)
    }

    /// What `:wq`, `:x` and `ZZ` write before they leave: the source
    /// of a Buffer that has one, and the file a changed Buffer over
    /// nothing was opened for. `None` when that write did not happen,
    /// and the editor stays where it is having said why.
    fn written(
        &self,
        ctx: &dyn Calls,
        unsaved: bool,
    ) -> Result<Option<u8>, Error> {
        // Over nothing and untouched: `:x` writes nothing, as Vim has
        // it, and there is no file to make.
        if self.vim.borrow().source.is_none() && !unsaved {
            return Ok(Some(0));
        }
        Ok(self.save(ctx, false)?.then_some(0))
    }

    /// `:exp[lore][!] [NAME]`: the Client asked to show an rd in this
    /// region, which is the end of this Vim. `NAME` is resolved where
    /// the editor stands, as `:w NAME` is, and with
    /// no name it is the directory the file is in. Whatever is there
    /// goes to the Client's table, a Container to a listing and a cell
    /// to another Vim; a name that is not there is E344. The swap
    /// takes the draft with it, so unsaved changes are E37 without the
    /// bang, as `:q` has it.
    fn explore(
        &self,
        ctx: &dyn Calls,
        name: Option<&str>,
        bang: bool,
    ) -> Result<(), Error> {
        let Some(name) = name else {
            return self.shown(ctx, self.here(ctx)?, "", bang);
        };
        // Nothing is being left for a name that leads nowhere, so
        // there is nothing for the bang to override yet.
        let Some(target) = self.at(ctx, name)? else {
            self.says(&format!("{E344} \"{name}\" in cdpath"));
            return Ok(());
        };
        self.shown(ctx, target, "", bang)
    }

    /// `:e[dit][!] PATH`: the file that name holds in this region,
    /// which the Client's table gives another Vim. The name is
    /// resolved as `:explore`'s is; one that is not there is E345.
    fn edited(
        &self,
        ctx: &dyn Calls,
        name: &str,
        bang: bool,
    ) -> Result<(), Error> {
        let Some(target) = self.at(ctx, name)? else {
            self.says(&format!("{E345} \"{name}\" in path"));
            return Ok(());
        };
        self.shown(ctx, target, "", bang)
    }

    /// `:cli[!]`: a shell in this region, which the Client's table
    /// gives by name and no shape gives at all. It opens where the `:`
    /// line's own lines stand, the Shell's cwd, so `:cd` moves what
    /// `:cli` opens over as it moves what `:ls` lists.
    fn cli(&self, ctx: &dyn Calls, bang: bool) -> Result<(), Error> {
        let cwd = self.vim.borrow().shell.cwd();
        self.shown(ctx, cwd, "cli", bang)
    }

    /// The Client asked to show `target` in this region, in the Plugin
    /// `plugin` names or, with an empty name, in the one its table
    /// gives that rd's shape. This is the end of this Vim: the swap
    /// takes the draft with it, so unsaved changes are E37 without the
    /// bang, as `:q` has it. Nothing is shown again here; the Plugin
    /// that comes shows itself.
    fn shown(
        &self,
        ctx: &dyn Calls,
        target: Rd,
        plugin: &str,
        bang: bool,
    ) -> Result<(), Error> {
        if !bang && self.unsaved(ctx)? {
            self.says(E37);
            return Ok(());
        }
        self.region()?.opens(ctx, target, plugin)
    }

    /// One `:` line past its range: the name its first token stands
    /// for, run under Vim's own `bin` with the rest of the line as one
    /// argument and the range and the bang as the environment; a first
    /// token no name of Vim's begins is the shell's own line, whole and
    /// as it was typed.
    ///
    /// The Shell is a copy for the run, taken and not borrowed: the
    /// Driver it reaches is this Plugin, whose state the key that typed
    /// the line is running over, and which finds it as it left it.
    fn ran(&self, ctx: &dyn Calls, ex: &Ex) -> Result<(), Error> {
        let Typed { name, bang, arg } = typed(&ex.line);
        let Some(name) = self.expanded(ctx, name)? else {
            return self.shelled(ctx, &ex.line);
        };
        let mut shell = self.vim.borrow().shell.clone();
        ranged(&mut shell, ex.from, ex.to, bang);
        // The rest of the line is one argument by Vim's own rule, and
        // an empty one is no argument at all.
        let one = [arg];
        let args = if arg.is_empty() { &one[..0] } else { &one[..] };
        let out = match sh::ran(ctx, &shell, &name, args) {
            Ok(ran) => printed(ctx, ran),
            Err(miss) => format!("{miss}"),
        };
        self.says(out.lines().next().unwrap_or_default());
        Ok(())
    }

    /// A line no name of Vim's begins, typed on the `:` line: run
    /// through clients/sh over this tree, from where the Shell stands,
    /// and its first line of output said in the status row, which is
    /// all one row shows. A first token PATH has not is E492, which is
    /// Vim's own; every other `Miss` is said as the shell says it.
    ///
    /// A run that came back open is a Driver that follows: what its
    /// cell holds at this one read is what is said, and the cell is
    /// closed after it, one row having nowhere to put more.
    fn shelled(&self, ctx: &dyn Calls, line: &str) -> Result<(), Error> {
        // The borrow is gone before the line runs: a Driver of this
        // Client's own may come back in here.
        let mut shell = core::mem::take(&mut self.vim.borrow_mut().shell);
        let was = shell.cwd();
        let outcome = sh::run(ctx, &mut shell, line);
        let out = match outcome {
            Ok(ran) => printed(ctx, ran),
            Err(miss) if miss.unknown => format!("{E492} {line}"),
            Err(miss) => format!("{miss}"),
        };
        let moved = shell.cwd();
        self.vim.borrow_mut().shell = shell;
        self.says(out.lines().next().unwrap_or_default());
        if moved == was { Ok(()) } else { self.rebased(ctx, moved) }
    }

    /// Where `:cd` left the Shell, taken as the directory Vim works
    /// in too, which is what nvim's `:cd` does: `:w NAME`, `:edit
    /// NAME` and `:explore NAME` resolve there from now on, and Tab
    /// completes there. A cwd outside the mirror leaves it where it
    /// was, no file of the editor's being under one.
    fn rebased(&self, ctx: &dyn Calls, cwd: Rd) -> Result<(), Error> {
        let root = self.vim.borrow().root.unwrap_or(ROOT);
        let mut at = cwd;
        while at != root && at != ROOT {
            at = ops::parent(ctx, at)?;
        }
        if at == root {
            self.vim.borrow_mut().cwd = Some(cwd);
        }
        Ok(())
    }

    /// Tab on the `:` line: what may stand where the last token does,
    /// found over the tree, which the line and the wildmenu then take.
    ///
    /// The first token is the names PATH holds, whose first directory
    /// is Vim's own `bin`, so a completion offers first what the line
    /// would run and one alone says the usage line it declares. Past
    /// it, the name is written out and its argument is `set`'s options,
    /// a name under the directory the editor works in, or nothing at
    /// all; a line no name of Vim's begins is the shell's, which
    /// completes it from where the Shell stands.
    fn completing(&self, ctx: &dyn Calls, line: &str) -> Result<(), Error> {
        let shell = self.vim.borrow().shell.clone();
        let Some((first, rest)) = line.split_once(' ') else {
            let found = sh::complete(ctx, &shell, line);
            let alone = match &found.candidates[..] {
                [one] => sh::form(ctx, &shell, one),
                _ => None,
            };
            self.vim.borrow_mut().listed(found.from, found.candidates, alone);
            return Ok(());
        };
        let Some(name) = self.expanded(ctx, first)? else {
            let found = sh::complete(ctx, &shell, line);
            self.vim.borrow_mut().offer(found.from, found.candidates);
            return Ok(());
        };
        let line = format!("{name} {rest}");
        let from = line.rfind(' ').map_or(0, |at| at + 1);
        let word = String::from(&line[from..]);
        self.vim.borrow_mut().expand(&line);
        let found = match name.as_str() {
            "set" => options(&word),
            "write" | "edit" | "explore" => self.names(ctx, &word),
            // Every other name of Vim's takes nothing from the tree.
            _ => Vec::new(),
        };
        self.vim.borrow_mut().offer(from, found);
        Ok(())
    }

    /// The names that may stand where `word` does: the children of
    /// where the editor stands, or of what `word` names under it.
    ///
    /// A leading `/` is the mirror's own root, where `:w /NAME`
    /// writes, and not the tree's, so it comes off the word and goes
    /// back on what is found.
    fn names(&self, ctx: &dyn Calls, word: &str) -> Vec<String> {
        let (from, rest) = self.from(word);
        let found = ops::paths(ctx, from, rest);
        if !word.starts_with('/') {
            return found;
        }
        found.into_iter().map(|name| format!("/{name}")).collect()
    }

    /// Where `:explore` with no name looks: the directory the Buffer's
    /// source is in, or, over nothing, the one the editor stands in.
    fn here(&self, ctx: &dyn Calls) -> Result<Rd, Error> {
        let (source, cwd) = {
            let vim = self.vim.borrow();
            (vim.source, vim.cwd.unwrap_or(ROOT))
        };
        source.map_or(Ok(cwd), |source| ops::parent(ctx, source))
    }

    /// The rd `path` names, or `None` where the tree has nothing
    /// there: the walk stops at the first name that is not.
    fn at(&self, ctx: &dyn Calls, path: &str) -> Result<Option<Rd>, Error> {
        let (from, rest) = self.from(path);
        match cell::resolve(ctx, from, rest) {
            Err(_) => Ok(None),
            found => found.map(Some),
        }
    }

    /// Where a path of the person's starts and what is left of it: the
    /// mirror's own root for a leading `/`, where `:w /NAME` writes
    /// and not the tree's, else where the editor stands.
    fn from<'a>(&self, path: &'a str) -> (Rd, &'a str) {
        let vim = self.vim.borrow();
        let (from, rest) = path
            .strip_prefix('/')
            .map_or((vim.cwd, path), |rest| (vim.root, rest));
        (from.unwrap_or(ROOT), rest)
    }

    /// Whether `path` is a directory in the tree; the empty path
    /// names where the editor stands, which always is.
    fn directory(&self, ctx: &dyn Calls, path: &str) -> Result<bool, Error> {
        let Some(rd) = self.at(ctx, path)? else { return Ok(false) };
        Ok(ops::info(ctx, rd)?.flags.contains(Flags::CONTAINER))
    }

    /// The rd of the file `path` names, made: every missing directory
    /// on the way by `mkdir`, then `create` for the file itself.
    /// `None` when a file is there already and `bang` does not say to
    /// write over it.
    fn making(
        &self,
        ctx: &dyn Calls,
        path: &str,
        bang: bool,
    ) -> Result<Option<Rd>, Error> {
        let (from, rest) = self.from(path);
        let steps = rest.split('/').filter(|n| !n.is_empty() && *n != ".");
        let names: Vec<&str> = steps.collect();
        let Some((name, dirs)) = names.split_last() else {
            return Err(Error::Invalid);
        };
        let mut dir = from;
        for step in dirs {
            dir = below(ctx, dir, step)?;
        }
        match made(ctx, dir, "create", name) {
            Err(Error::Stale) if bang => ops::lookup(ctx, dir, name).map(Some),
            Err(Error::Stale) => Ok(None),
            made => made.map(Some),
        }
    }

    /// Whether the Buffer holds what its source has not.
    fn unsaved(&self, mut ctx: &dyn Calls) -> Result<bool, Error> {
        let revisions = history::Call(&mut ctx, self.handle()?).tree()?;
        Ok(revisions.head != revisions.published)
    }
}

/// The file as the person named it: `name`, a path under the root,
/// read from `dir`, the directory the Client started in, when it is
/// below it, as nvim says the path it was given.
fn as_given<'a>(dir: &str, name: &'a str) -> &'a str {
    name.strip_prefix(dir)
        .and_then(|rest| rest.strip_prefix('/'))
        .unwrap_or(name)
}

/// What Vim registers under its home, in the order `ls` shows them:
/// its frame, the state it holds read out of the engine, and `bin`,
/// whose eleven names are the `:` line's own.
///
/// Every register that can exist is registered here, filled or not,
/// because the Host asks a Container to list itself once and answers
/// from its table ever after: one first filled later would never be
/// listed.
///
/// A name under `bin` is a Container, so a run's output cell goes under
/// it, and READ, so whoever runs it may hold the right `run` asks for.
fn registered(ctx: &dyn Calls, home: Rd) -> Result<Cells, Error> {
    let node = |at, name: &str, flags, decls: &[Decl]| {
        let rd = ctx.register(DRIVER, at, Some(name))?;
        ctx.report(rd, flags, decls)?;
        Ok::<Rd, Error>(rd)
    };
    let read = |at, name: &str, flags| node(at, name, flags, &[]);
    let one = Flags::READ;
    let held = one.union(Flags::CONTAINER);
    let view = read(home, "view", one)?;
    let mode = read(home, "mode", one)?;
    let pending = read(home, "pending", one)?;
    let under = read(home, "registers", held)?;
    let mut registers = BTreeMap::new();
    for name in core::iter::once('"').chain('a'..='z') {
        registers.insert(read(under, &format!("{name}"), one)?, name);
    }
    let bin = read(home, "bin", held)?;
    for (name, decl) in bins::declared() {
        node(bin, name, held, core::slice::from_ref(&decl))?;
    }
    Ok(Cells { view, mode, pending, registers, bin })
}

/// Vim's own language in the environment of the Shell a run reads: the
/// range and the bang, which the `:` line reads off its front before
/// the name and never spells as tokens.
///
/// They stand there from `start`, so no usage line of one of Vim's
/// names holds them either, and every line writes its own over them.
fn ranged(shell: &mut Shell, from: u64, to: u64, bang: bool) {
    shell.give("from", Value::U64(from));
    shell.give("to", Value::U64(to));
    shell.give("bang", Value::Bool(bang));
}

/// The name an rd stands at, which is which of the eleven a call is on.
///
/// # Errors
/// As `Op::Info`; `Unsupported` for an rd with no name.
pub fn named(ctx: &dyn Calls, rd: Rd) -> Result<String, Error> {
    ops::info(ctx, rd)?.name.ok_or(Error::Unsupported)
}

/// `path`, which is under the mirror's own root, as a path of the
/// person's: the leading `/` that says to read it from there and not
/// from where the editor stands.
fn rooted(path: &str) -> String {
    format!("/{path}")
}

/// The directory `name` under `dir`, made when it is not there yet;
/// `..` is the walk's own move and makes nothing.
fn below(ctx: &dyn Calls, dir: Rd, name: &str) -> Result<Rd, Error> {
    if name == ".." {
        return ops::parent(ctx, dir);
    }
    match ops::lookup(ctx, dir, name) {
        Err(Error::Gone) => made(ctx, dir, "mkdir", name),
        found => found,
    }
}

/// `create` or `mkdir` of the fs api on the directory `dir`: the rd
/// it made.
fn made(
    ctx: &dyn Calls,
    dir: Rd,
    method: &str,
    name: &str,
) -> Result<Rd, Error> {
    let input = encode(&String::from(name));
    let out = ops::call_by_name(ctx, dir, method, input)?;
    decode(&out)
}

/// What a run printed, its cell closed where the run came back open: a
/// Driver that follows is read this once and no more, one row of a
/// status line having nowhere to put more.
fn printed(ctx: &dyn Calls, ran: Ran) -> String {
    let Ran { out, open } = ran;
    if let Some(mut open) = open {
        let _ = open.close(ctx);
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A Buffer over `source`, or over nothing: the one the Plugin shows,
/// made through `/module/text/open` and kept by the handle `start`
/// opens on it.
fn buffer_over(mut ctx: &dyn Calls, source: Option<Rd>) -> Result<Rd, Error> {
    let opener = cell::resolve(ctx, ROOT, "/module/text/open")?;
    let opening = ctx.open(opener, Rights::WRITE)?;
    let mut call = text_open::Call(&mut ctx, opening);
    let buffer = match source {
        Some(source) => call.open(source),
        None => call.new(),
    };
    ops::close(ctx, opening)?;
    buffer
}
