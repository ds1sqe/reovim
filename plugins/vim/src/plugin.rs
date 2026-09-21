//! Vim as a Module a Client loads: what the keys asked for, done.
//!
//! The engine below is pure: a key becomes an `Effect`, which has
//! already happened to Vim's own copy of the text; here it becomes
//! calls. An edit is a `replace` on the draft `/self/dev/buffers`
//! gave, which costs no wire message and joins the revision the draft
//! holds; undo and redo move the Buffer's head and the text is read
//! back; a save publishes it to its source; a put reads the clipboard
//! rd and a `"+` yank writes it; a quit writes `/self/dev/exit`, and
//! what all of it looks like is the View that ends the frame.
//!
//! Where a revision ends is Vim's to say, in two lines: an edit that
//! does not join the one under way flushes first, and a frame that
//! ends out of insert flushes, unless the stay is already stale. Where
//! the flush answers `Stale` — another Client's run landing in front
//! of this one — nothing is sent again: the draft reads back whole,
//! so an edit it refused is off the screen too, and the row says W12
//! until the next key.
//!
//! The cells served are `view` and Vim's own state. A whole read of
//! `view` is one frame: captured startup lines once, then the keys Vim
//! has not taken; the flush, the rows the region has now; then the
//! View. The rest — the mode, the keys of a command that
//! has not fired, the registers — are made at the read out of what the
//! engine holds, so nothing is stored twice and nothing goes stale. No
//! borrow of the engine outlives an op, so a Driver of Vim's own that
//! a run re-enters finds the state free.

use alloc::collections::{BTreeMap, VecDeque};
use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::{Cell, Ref, RefCell};
use core::fmt::Write as _;

use reovim_client_api::{NONE, buffers, draft, open, part};
use reovim_client_region::{Region, Surface, count, drafted};
use reovim_client_sh::{self as sh, Ran, Shell};
use reovim_kernel_interface::{
    Booted, Calls, Decl, Driver, DriverId, Error, Flags, Handle, Hash, Module,
    Rd, Reply, Rights,
};
use reovim_sdk::dynamic::Value;
use reovim_sdk::walk::ROOT;
use reovim_sdk::{cell, decode, encode, ops};
use reovim_text_api::{history, publish, text};

use crate::command::{Typed, options, typed};
use crate::keymap::{Held, Stroke};
use crate::macros::BUDGET;
use crate::{
    E32, Edit, Effect, Ex, Key, Land, Mode, Pos, Quit, Switch, Vim, bins,
};

/// What `:q` says to unsaved changes.
const E37: &str = "E37: No write since last change (add ! to override)";
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
/// What `:edit` says to an unusable or unnameable path.
const E345: &str = "E345: Can't find file";
/// What undo and redo say at the ends of the history.
const OLDEST: &str = "Already at oldest change";
const NEWEST: &str = "Already at newest change";
/// What a stale draft says: the Buffer on the screen, which the status
/// row already names, so the message names nothing itself and fits
/// beside a short file name at 80 columns.
const W12: &str = "W12: changed in another Client and here; :e! takes theirs";

/// The one Driver of this Module: every rd it registers is served by
/// it, its own cells and the output of every run alike.
const DRIVER: DriverId = DriverId(0);

/// Vim at a Client: the engine, which one stay after another is made
/// over, and one stay at the Plugin.
///
/// The engine is not part of a stay: registers, the last search and
/// the rest of what is kept across a restart must outlive it, and the
/// per-Buffer record `left` (lib.rs) keeps outlives it too. `stay` is
/// `None` before the first `start` and while one has failed; a second
/// `Rc` clone of it never outlives an op, so no read finds it held.
#[derive(Default)]
pub struct Editor {
    vim: RefCell<Vim>,
    stay: RefCell<Option<Rc<Stay>>>,
    queue: RefCell<VecDeque<Queued>>,
    spent: Cell<usize>,
    closing: Cell<Option<(Rd, Option<Rd>)>>,
    /// Captured startup lines, taken once at the first Vim frame.
    init: RefCell<Option<String>>,
}

/// Provenance controls recording, cancellation and the execution budget.
#[derive(Clone, Copy)]
enum Queued {
    Typed(Key),
    Fed(Key),
    /// A key a failed prefix let go, carrying the origin it was taken
    /// with. It is not looked up a second time, and it is charged no
    /// second time either: the input it came from paid for it.
    Past(Stroke),
}

/// A source-backed draft, in the Client's listing order.
struct Listed {
    draft: Rd,
    name: String,
    source: Rd,
    path: String,
}

/// What one stay at the Plugin is: the region this Vim holds for it,
/// the rds registered under its home, and the output of its own runs.
/// Made whole at `start`, once every fallible call it needs has
/// answered, and dropped at `stop`.
struct Stay {
    surface: Surface,
    /// The rds under Vim's home, as `start` was answered with them.
    cells: Cells,
    /// What the runs of Vim's own names printed, until each is read
    /// and closed.
    outputs: cell::Cells,
    /// Set while the draft is stale: `framed` makes no closing `sync`
    /// then, so W12 is one row until the next key and not every
    /// frame's. False again at the next `start`.
    stale: Cell<bool>,
    /// A file switch leaves the queued input for the following stay.
    leaving: Cell<bool>,
    /// A successful request to replace this stay or leave the editor.
    asked: Cell<bool>,
}

/// What a read of one of Vim's own rds is a read of: the rds `start`
/// registered under Vim's home.
struct Cells {
    view: Rd,
    mode: Rd,
    pending: Rd,
    /// One rd per register that can exist, by its name.
    registers: BTreeMap<Rd, char>,
    /// A line a mark: `NAME LINE COLUMN`, a file mark's followed by
    /// where its source is.
    marks: Rd,
    /// `jumps_listed`'s own rd.
    jumps: Rd,
    recording: Rd,
    /// One rd per option, by its name: a reading of the engine's own
    /// field, `set` being the one writer.
    options: BTreeMap<Rd, &'static str>,
    /// `/self/vim/keymap`'s own rd.
    keymap: Rd,
    /// `/self/vim/point`'s own rd: `point` with no argument.
    point: Rd,
    /// `/self/vim/bin`, whose listing is the order a `:` line resolves
    /// a name in.
    bin: Rd,
}

impl Editor {
    /// One, which a table gives a Client and a test reads back.
    #[must_use]
    pub fn new() -> Rc<Self> {
        Rc::new(Self::default())
    }

    /// Startup lines held until this editor's first frame, run only once.
    /// The caller captures the file; this portable Plugin reads no file.
    ///
    /// ```
    /// let editor = reovim_plugin_vim::Editor::with_init("set ic\n".into());
    /// let module = editor.booted();
    /// ```
    #[must_use]
    pub fn with_init(text: String) -> Rc<Self> {
        Rc::new(Self { init: RefCell::new(Some(text)), ..Self::default() })
    }

    /// Its Module and its one Driver, for a table to give. Every start
    /// is this same Editor's, so the state kept across a restart
    /// outlives every swap of the region.
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

    /// The current stay, a clone of the `Rc` so no borrow of it
    /// outlives an op; `Closed` before `start` or after `stop`.
    fn stay(&self) -> Result<Rc<Stay>, Error> {
        self.stay.borrow().clone().ok_or(Error::Closed)
    }
}

impl Module for Editor {
    /// A stay at the Plugin: its own `Surface`, cells and outputs,
    /// made once every fallible call this needs has answered, so a
    /// `start` that fails leaves nothing half made and no borrow of
    /// the engine outstanding; the engine is filled only after, and
    /// `announce` alone comes later still.
    ///
    /// A draft of its own over what the region was opened on, read
    /// and written through a handle it keeps, the clipboard found if
    /// there is one, and the text shown from the line it was told to
    /// begin at, or the `Place` this draft kept from its last stay.
    /// `dir`, `root`, `cwd`, `shell` and `clip` are kept across every
    /// stay: `root` is `None` only before the very first `start`, so
    /// they are found once there and never again, `:cd` and the
    /// shell's own state living on through every swap since.
    ///
    /// `arrive` (see that field) is taken here first, before any of
    /// this can fail: a `start` that fails after drops it with the
    /// local it became, never `opened`'s to see.
    fn start(&self, ctx: &dyn Calls, home: Rd) -> Result<(), Error> {
        let closing = self.closing.take();
        let greeting = self.vim.borrow_mut().greeting.take();
        let arrive = self.vim.borrow_mut().arrive.take();
        if let Some((draft, source)) = closing {
            let list = cell::resolve(ctx, ROOT, "/self/dev/buffers")?;
            let h = ctx.open(list, Rights::WRITE)?;
            let mut calls = ctx;
            let closed = buffers::Call(&mut calls, h).close(draft);
            ops::close(ctx, h)?;
            closed_draft(closed)?;
            self.vim.borrow_mut().forgot(draft, source);
        }
        let surface = Surface::default();
        let view = surface.start(ctx, home)?;
        let cells = registered(ctx, home, view)?;
        let region = surface.region()?;
        let ask = region.ask(ctx)?;
        let source = (ask.target != NONE).then_some(ask.target);
        let buffer = drafted(ctx, source)?;
        let rights = Rights::READ.union(Rights::WRITE);
        let h = ctx.open(buffer, rights)?;
        let fresh = self.vim.borrow().root.is_none();
        let kept = fresh.then(|| kept_of(ctx, region)).transpose()?;
        let rows = count(region.size(ctx)?.rows);
        let bytes = ops::whole(ctx, h)?;
        let stay = Stay {
            surface,
            cells,
            outputs: cell::Cells::default(),
            stale: Cell::new(false),
            leaving: Cell::new(false),
            asked: Cell::new(false),
        };
        *self.stay.borrow_mut() = Some(Rc::new(stay));
        let mut vim = self.vim.borrow_mut();
        vim.arrived(source);
        vim.greeting = greeting;
        vim.buffer = Some(buffer);
        vim.h = Some(h);
        vim.source = source;
        vim.name = (!ask.name.is_empty()).then(|| ask.name.clone());
        if let Some(kept) = kept {
            vim.dir = kept.dir;
            vim.root = Some(kept.root);
            vim.cwd = Some(kept.cwd);
            vim.shell = kept.shell;
            vim.clip = kept.clip;
        }
        vim.rows = rows;
        vim.opened(&bytes, ask.line, arrive);
        drop(vim);
        self.announce(ctx)?;
        if self.spent.get() >= BUDGET {
            self.says(&format!("@: stopped after {BUDGET} keys"));
        }
        Ok(())
    }

    /// Before unload, by every door a stay ends by: what belongs to
    /// the Buffer just left is kept for its return, and the stay
    /// itself goes.
    fn stop(&self) {
        self.vim.borrow_mut().leave();
        let leaving = self
            .stay
            .borrow_mut()
            .take()
            .is_some_and(|stay| stay.leaving.get());
        if !leaving {
            self.queue.borrow_mut().clear();
        }
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
        let Ok(stay) = self.stay() else { return Reply::err(Error::Closed) };
        let rd = Rd(h);
        if rd == stay.cells.view {
            return stay.surface.read(at, len, || self.framed(ctx));
        }
        let output = || stay.outputs.read(ctx, rd, at, len).into();
        self.held(ctx, &stay.cells, rd)
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
        self.stay().and_then(|stay| stay.outputs.close(ctx, Rd(h))).into()
    }
}

impl Editor {
    /// What a cell of Vim's state reads, made out of the engine at the
    /// read: the borrow is this statement's, and Vim's own frame has
    /// returned by the time another Plugin asks. `None` for an rd that
    /// is no cell of the state's.
    fn held(&self, ctx: &dyn Calls, cells: &Cells, rd: Rd) -> Option<Vec<u8>> {
        let vim = self.vim.borrow();
        let read = if rd == cells.mode {
            format!("{}\n", vim.mode().name())
        } else if rd == cells.pending {
            vim.showcmd()
        } else if rd == cells.marks {
            marks_listed(ctx, &vim)
        } else if rd == cells.jumps {
            jumps_listed(ctx, &vim)
        } else if rd == cells.recording {
            vim.macros
                .recording
                .map_or_else(String::new, |name| format!("{name}\n"))
        } else if let Some(name) = cells.options.get(&rd) {
            vim.shown(name)?
        } else if rd == cells.keymap {
            vim.keymap.text()
        } else if rd == cells.point {
            format!("{}\n", self.pointed(ctx, false, false, None))
        } else {
            vim.stored(*cells.registers.get(&rd)?)
        };
        Some(read.into_bytes())
    }

    /// One frame: every key it has not taken, run; the revision they
    /// made ended where the mode says it is over; the rows the region
    /// has now; the View.
    fn framed(&self, ctx: &dyn Calls) -> Result<Vec<u8>, Error> {
        let rows = self.sized(ctx)?;
        let init = self.init.borrow_mut().take();
        if let Some(text) = init {
            self.began(ctx, &text)?;
        }
        if !self.stay()?.asked.get() {
            let keys = self.stay()?.surface.region()?.keys(ctx)?;
            self.queue.borrow_mut().extend(keys.into_iter().map(Queued::Typed));
        }
        while self.step(ctx)? {}
        // The second of the two flushes: an insert session ends at its
        // Esc, and every other edit at the frame that made it. A stay
        // already stale skips it: the row already says W12, and a
        // flush of a draft already known stale would cost a read of
        // it back for nothing.
        let mode = self.vim.borrow().mode();
        if mode != Mode::Insert && !self.stay()?.stale.get() {
            self.stale(ctx, |ctx| self.sync(ctx))?;
        }
        Ok(encode(&self.vim.borrow_mut().view(rows)))
    }

    /// Run captured lines once, preserving the opening frame's message.
    fn began(&self, ctx: &dyn Calls, text: &str) -> Result<(), Error> {
        let opening = core::mem::take(&mut self.vim.borrow_mut().message);
        let result = init_lines(self, ctx, text);
        self.vim.borrow_mut().message = opening;
        result
    }

    /// Each line has its own execution boundary, but typed-command effects.
    fn init_line(
        &self,
        ctx: &dyn Calls,
        number: usize,
        line: &str,
        more: bool,
    ) -> Result<bool, Error> {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            return Ok(true);
        }
        let effect = {
            let mut vim = self.vim.borrow_mut();
            vim.macros.fed = false;
            vim.macros.joined = false;
            vim.ex(line)
        };
        let result = self.act(ctx, effect);
        let (message, failed) = {
            let mut vim = self.vim.borrow_mut();
            (core::mem::take(&mut vim.message), vim.rung())
        };
        result?;
        let level = if failed { 4 } else { 6 };
        if !message.is_empty() {
            logged(ctx, level, &format!("init:{number}: {message}"))?;
        }
        let asked = self.stay()?.asked.get();
        if asked && more {
            logged(
                ctx,
                4,
                &format!(
                    "init:{number}: left; the lines after it were not run"
                ),
            )?;
        }
        Ok(!asked)
    }

    /// Run a key without holding a queue borrow across Driver calls.
    fn step(&self, ctx: &dyn Calls) -> Result<bool, Error> {
        if self.stay()?.asked.get() {
            return Ok(false);
        }
        let Some(queued) = self.queue.borrow_mut().pop_front() else {
            return Ok(false);
        };
        let effect = self.key(queued);
        let result =
            self.act(ctx, effect).and_then(|()| self.write_clipped(ctx));
        self.vim.borrow_mut().macros.fed = false;
        if result.is_err() {
            self.queue.borrow_mut().clear();
            self.vim.borrow_mut().held = Held::default();
        }
        result?;
        // What this key let go goes to the queue's front, before the
        // bell's check below, so a bell drops a macro's released keys
        // with the rest of its run. A released key that lets go of
        // more, an Alt among them, is drained by its own step.
        let released = self.vim.borrow_mut().released();
        let mut queue = self.queue.borrow_mut();
        for stroke in released.into_iter().rev() {
            queue.push_front(Queued::Past(stroke));
        }
        drop(queue);
        let exhausted = self.spent.get() >= BUDGET;
        if self.vim.borrow_mut().rung() || exhausted {
            self.queue.borrow_mut().retain(|queued| match queued {
                Queued::Typed(_) => true,
                Queued::Fed(_) => false,
                Queued::Past(stroke) => !stroke.fed,
            });
            self.vim.borrow_mut().cancel();
        }
        if exhausted {
            self.says(&format!("@: stopped after {BUDGET} keys"));
        }
        Ok(true)
    }

    /// Fed context survives the engine call until its whole effect finishes.
    fn key(&self, queued: Queued) -> Effect {
        let mut vim = self.vim.borrow_mut();
        vim.macros.fed = matches!(queued, Queued::Fed(_));
        match queued {
            Queued::Typed(key) => {
                self.spent.set(0);
                vim.key(key)
            }
            Queued::Fed(key) => {
                self.spent.set(self.spent.get() + 1);
                vim.fed(key, false)
            }
            // Neither counted nor zeroing: the key that let this one
            // go was counted, or zeroed, already.
            Queued::Past(stroke) => vim.past(stroke),
        }
    }

    /// Keep only the fed prefix the remaining budget can reach, plus a
    /// sentinel. Typed input is never cancelled by a replay's bound.
    fn replayed(&self, mut keys: Vec<Key>) {
        let capacity = (BUDGET + 1).saturating_sub(self.spent.get());
        keys.truncate(capacity);
        let mut spare = capacity - keys.len();
        let mut queue = self.queue.borrow_mut();
        queue.retain(|key| {
            let fed = matches!(key, Queued::Fed(_));
            let keep = !fed || spare != 0;
            spare = spare.saturating_sub(usize::from(fed));
            keep
        });
        for key in keys.into_iter().rev() {
            queue.push_front(Queued::Fed(key));
        }
    }

    /// The draft flushed: what it holds goes down as one revision, or
    /// `Stale` where the server refused it — read by [`Self::stale`],
    /// the one place that answer is read.
    fn sync(&self, mut ctx: &dyn Calls) -> Result<(), Error> {
        draft::Call(&mut ctx, self.handle()?).sync()
    }

    /// Every `Err(Stale)` `done` may answer, caught in one place: the
    /// draft read back whole, so an edit it refused is off the screen
    /// too; W12 said until the next key; the stay marked so `framed`'s
    /// own closing `sync` says it once and not every frame's. Any
    /// other answer, `done`'s own.
    fn stale(
        &self,
        ctx: &dyn Calls,
        done: impl FnOnce(&dyn Calls) -> Result<(), Error>,
    ) -> Result<(), Error> {
        match done(ctx) {
            Err(Error::Stale) => {
                self.reloaded(ctx)?;
                self.fails(W12);
                self.stay()?.stale.set(true);
                Ok(())
            }
            other => other,
        }
    }

    /// The draft read whole into the engine: what a flush that moved
    /// the bytes and a walk of the history both end with.
    fn reloaded(&self, ctx: &dyn Calls) -> Result<(), Error> {
        let bytes = ops::whole(ctx, self.handle()?)?;
        self.vim.borrow_mut().loaded(&bytes);
        Ok(())
    }

    /// The rows the region has now, as `windows/0/size` says.
    fn sized(&self, ctx: &dyn Calls) -> Result<usize, Error> {
        let rows = count(self.stay()?.surface.region()?.size(ctx)?.rows);
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

    /// A refused operation ends every pending replay.
    fn fails(&self, message: &str) {
        self.vim.borrow_mut().error(message);
    }

    /// What `effect` asks of the Client, every `Stale` it may answer
    /// caught in [`Self::stale`].
    fn act(&self, ctx: &dyn Calls, effect: Effect) -> Result<(), Error> {
        self.stale(ctx, |ctx| match effect {
            Effect::Nothing | Effect::Moved => Ok(()),
            Effect::Replay(keys) => {
                self.replayed(keys);
                Ok(())
            }
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
            Effect::Switch(to) => self.switched(ctx, to),
            Effect::Close(name, bang) => self.closed(ctx, &name, bang),
            Effect::List => self.list(ctx),
            Effect::SaveAll => self.save_all(ctx).map(drop),
            Effect::Explore(name, bang) => {
                self.explore(ctx, name.as_deref(), bang)
            }
            Effect::Open(name, bang) => self.edited(ctx, &name, bang),
            Effect::Reload(bang) => self.reload(ctx, bang),
            Effect::Line(ex) => self.ran(ctx, &ex),
            Effect::Cli(bang) => self.cli(ctx, bang),
            Effect::Complete(line) => self.completing(ctx, &line),
            // The empty name, as `edited` uses: the Client's table
            // answers by the rd's shape, there being no second Vim to
            // ask for by name; see `arrive` for the rest.
            Effect::Goto(rd, pos, land) => self.arriving(ctx, rd, pos, land),
        })
    }

    /// `Effect::Goto`'s own arm, apart for clippy's nesting bound. A
    /// refused ask sets `arrive` nothing, so there is nothing here for
    /// a later, unrelated `leave` to mistake for one.
    fn arriving(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        pos: Pos,
        land: Land,
    ) -> Result<(), Error> {
        if self.shown(ctx, rd, "", false, "")? {
            self.vim.borrow_mut().arrive = Some((rd, pos, land));
        }
        Ok(())
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
            let mut effect = made(&mut vim);
            // `did` is `:s` and Vim's own `bin` names, never typing.
            vim.changing(before, &mut effect, false);
            effect
        };
        if let Err(error) = self.act(ctx, effect) {
            self.reloaded(ctx)?;
            return Err(error);
        }
        let said = core::mem::take(&mut self.vim.borrow_mut().message);
        self.stay()?.outputs.open(ctx, DRIVER, rd, said.into_bytes())
    }

    /// Where the cursor or the selection is, as `/self/vim/bin/point`
    /// and `/self/vim/point` both read it: the path the Plugin
    /// resolves and the locator the engine spells, or what a Buffer
    /// over nothing says instead.
    pub(crate) fn pointed(
        &self,
        ctx: &dyn Calls,
        col: bool,
        relative: bool,
        range: Option<(usize, usize)>,
    ) -> String {
        let path = self.wherever(ctx, relative).ok().flatten();
        let vim = self.vim.borrow();
        vim.point(path.as_deref(), col, range).unwrap_or_else(String::from)
    }

    /// The file the Buffer is over as a path of the person's:
    /// absolute under the mirror's root, or from where the editor
    /// stands under `relative`. `None` for a Buffer over nothing.
    fn wherever(
        &self,
        ctx: &dyn Calls,
        relative: bool,
    ) -> Result<Option<String>, Error> {
        let (source, root, cwd) = {
            let vim = self.vim.borrow();
            (vim.source, vim.root.unwrap_or(ROOT), vim.cwd.unwrap_or(ROOT))
        };
        let Some(source) = source else { return Ok(None) };
        let root = ops::where_is(ctx, root)?;
        let under = |rd| -> Result<String, Error> {
            let at = ops::where_is(ctx, rd)?;
            let rest = at.strip_prefix(&root).unwrap_or(&at);
            Ok(rooted(rest.trim_start_matches('/')))
        };
        let path = under(source)?;
        if !relative {
            return Ok(Some(path));
        }
        Ok(Some(from_dir(&under(cwd)?, &path)))
    }

    /// Mark first, then run each line after releasing the engine borrow.
    pub(crate) fn globally(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        range: Option<(usize, usize)>,
        spec: &str,
        invert: bool,
    ) -> Result<Rd, Error> {
        let picked = {
            let mut vim = self.vim.borrow_mut();
            vim.message.clear();
            let range = range.unwrap_or_else(|| (0, vim.line_count() - 1));
            vim.picked(range, spec, invert)
        };
        if picked {
            self.global_lines(ctx)?;
        }
        let said = core::mem::take(&mut self.vim.borrow_mut().message);
        self.stay()?.outputs.open(ctx, DRIVER, rd, said.into_bytes())
    }

    /// Always retire the selection, including a failed call's exit.
    fn global_lines(&self, ctx: &dyn Calls) -> Result<(), Error> {
        let mut result = self.global_step(ctx);
        while result == Ok(true) {
            result = self.global_step(ctx);
        }
        let result = result.map(|_| ());
        self.vim.borrow_mut().global = crate::global::Global::default();
        if result.is_err() {
            self.reloaded(ctx)?;
        }
        result
    }

    /// No borrow is held while a line calls back through the tree. A
    /// line that rang, or asked the region away, is the run's last.
    fn global_step(&self, ctx: &dyn Calls) -> Result<bool, Error> {
        let next = self.vim.borrow_mut().next_marked();
        let Some(effect) = next else { return Ok(false) };
        self.act(ctx, effect)?;
        Ok(!self.vim.borrow().ringing() && !self.stay()?.asked.get())
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
        let under = ops::children(ctx, self.stay()?.cells.bin)?;
        let held = |rd: Rd| named(ctx, rd).ok();
        let mut listed = under.into_iter().filter_map(held);
        Ok(listed.find(|name| name.starts_with(token)))
    }

    /// `edit` into the revision the draft holds. The first of the two
    /// flushes: one that does not join ends the revision that was
    /// there, whatever made it.
    fn send(&self, mut ctx: &dyn Calls, edit: Edit) -> Result<(), Error> {
        let Edit { at, len, text, join } = edit;
        if !join {
            self.sync(ctx)?;
        }
        let edit = text::Edit { at: at as u64, len: len as u64, text };
        text::Call(&mut ctx, self.handle()?).replace(edit)
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
                self.fails(end);
                return Ok(());
            }
            Err(error) => return Err(error),
        }
        self.reloaded(ctx)
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
            self.fails(E32);
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
                self.fails(E13);
                Ok(false)
            }
            // Refused, or asked of an rd that makes nothing: either
            // way there is no file to write.
            Err(_) => {
                self.fails(E212);
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
        self.vim.borrow_mut().sourced(target);
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

    /// Check this draft, then other source-backed drafts before exit.
    /// A forced quit skips this draft; only forced quit-all skips all.
    /// Write-all visits every modified candidate before deciding to exit.
    fn quit(&self, ctx: &dyn Calls, how: Quit) -> Result<(), Error> {
        match how {
            Quit::Abort => return self.ended(ctx, 1),
            Quit::All(true) => return self.ended(ctx, 0),
            Quit::WriteAll => return self.quit_written_all(ctx),
            _ => {}
        }
        let unsaved = self.unsaved(ctx)?;
        if matches!(how, Quit::Ask | Quit::All(false)) && unsaved {
            self.fails(E37);
            return Ok(());
        }
        if how == Quit::Write && self.written(ctx, unsaved)?.is_none() {
            return Ok(());
        }
        if self.guard_others(ctx, true)? {
            return Ok(());
        }
        self.ended(ctx, 0)
    }

    fn quit_written_all(&self, ctx: &dyn Calls) -> Result<(), Error> {
        if self.save_all(ctx)? {
            self.ended(ctx, 0)?;
        }
        Ok(())
    }

    /// The way out taken: the region ended, and this stay asked, so
    /// no line or key after runs over it.
    fn ended(&self, ctx: &dyn Calls, code: u8) -> Result<(), Error> {
        let stay = self.stay()?;
        stay.surface.region()?.ends(ctx, code)?;
        stay.asked.set(true);
        Ok(())
    }

    /// List only drafts that expose an actual source identity: one
    /// whose `source` is not there ([`missing`]) is passed over.
    fn listed(&self, ctx: &dyn Calls) -> Result<Vec<Listed>, Error> {
        let (root, dir) = {
            let vim = self.vim.borrow();
            (vim.root.unwrap_or(ROOT), vim.dir.clone())
        };
        let root = format!("{}/", ops::where_is(ctx, root)?);
        let list = cell::resolve(ctx, ROOT, "/self/dev/buffers")?;
        let mut listed = Vec::new();
        for draft in ops::children(ctx, list)? {
            let child = match ops::lookup(ctx, draft, "source") {
                Ok(child) => child,
                Err(error) if missing(error) => continue,
                Err(error) => return Err(error),
            };
            let source = decode(&ops::bytes(ctx, child)?)?;
            let path = ops::where_is(ctx, source)?;
            let path =
                as_given(&dir, path.strip_prefix(&root).unwrap_or(&path));
            listed.push(Listed {
                draft,
                source,
                name: named(ctx, draft)?,
                path: String::from(path),
            });
        }
        Ok(listed)
    }

    fn list(&self, ctx: &dyn Calls) -> Result<(), Error> {
        let (current, alternate) = {
            let vim = self.vim.borrow();
            (vim.buffer, vim.alt)
        };
        let mut out = String::new();
        for item in self.listed(ctx)? {
            let line =
                item.line(current, alternate, unsaved_draft(ctx, item.draft)?);
            let _ = writeln!(out, "{line}");
        }
        self.says(&out);
        Ok(())
    }

    fn switched(&self, ctx: &dyn Calls, to: Switch) -> Result<(), Error> {
        let current = self.vim.borrow().buffer;
        let list = self.listed(ctx)?;
        let source = match to {
            Switch::Alternate => self.alternate(),
            Switch::Named(name) if name == "#" => self.alternate(),
            Switch::Named(name) => self.by_name(&list, &name),
            Switch::Next => neighbour(&list, current, false),
            Switch::Previous => neighbour(&list, current, true),
        };
        source.map_or(Ok(()), |source| {
            self.shown(ctx, source, "", false, "").map(drop)
        })
    }

    fn alternate(&self) -> Option<Rd> {
        let alternate = self.vim.borrow().alt;
        if alternate.is_none() {
            self.fails("E23: No alternate file");
        }
        alternate
    }

    fn by_name(&self, list: &[Listed], name: &str) -> Option<Rd> {
        let found = list.iter().find(|item| item.name == name);
        if found.is_none() {
            self.fails(&format!("E86: Buffer {name} does not exist"));
        }
        found.map(|item| item.source)
    }

    /// A quit follows the unsaved draft; a Container door stays put.
    fn guard_others(
        &self,
        ctx: &dyn Calls,
        follow: bool,
    ) -> Result<bool, Error> {
        let current = self.vim.borrow().buffer;
        let Some(other) = other_unsaved(ctx, self.listed(ctx)?, current)?
        else {
            return Ok(false);
        };
        let message = format!(
            "E162: No write since last change for buffer \"{}\"",
            other.path
        );
        self.fails(&message);
        if follow && self.shown(ctx, other.source, "", true, "")? {
            self.vim.borrow_mut().greets(message);
        }
        Ok(true)
    }

    fn closed(
        &self,
        mut ctx: &dyn Calls,
        name: &str,
        bang: bool,
    ) -> Result<(), Error> {
        let current = self.vim.borrow().buffer.ok_or(Error::Closed)?;
        let list = self.listed(ctx)?;
        let own = name.is_empty() || named(ctx, current)? == name;
        let selected = list.iter().find(|item| item.name == name);
        let (draft, source) = match (own, selected) {
            (true, _) => (current, self.vim.borrow().source),
            (false, Some(item)) => (item.draft, Some(item.source)),
            (false, None) => {
                self.fails("E516: No buffers were deleted");
                return Ok(());
            }
        };
        if !bang && unsaved_draft(ctx, draft)? {
            self.fails(&format!(
                "E89: No write since last change for buffer {} \
                 (add ! to override)",
                named(ctx, draft)?
            ));
            return Ok(());
        }
        if own {
            return self.close_current(ctx, draft, source, &list);
        }
        let root = cell::resolve(ctx, ROOT, "/self/dev/buffers")?;
        let h = ctx.open(root, Rights::WRITE)?;
        let result = buffers::Call(&mut ctx, h).close(draft);
        ops::close(ctx, h)?;
        closed_draft(result)?;
        self.vim.borrow_mut().forgot(draft, source);
        Ok(())
    }

    fn close_current(
        &self,
        ctx: &dyn Calls,
        draft: Rd,
        source: Option<Rd>,
        list: &[Listed],
    ) -> Result<(), Error> {
        let alternate = self.vim.borrow().alt;
        let to = list
            .iter()
            .find(|item| Some(item.source) == alternate)
            .map(|item| item.source)
            .or_else(|| neighbour(list, Some(draft), false))
            .filter(|to| Some(*to) != source)
            .unwrap_or(NONE);
        if self.shown(ctx, to, "", true, "")? {
            self.closing.set(Some((draft, source)));
        }
        Ok(())
    }

    fn save_all(&self, ctx: &dyn Calls) -> Result<bool, Error> {
        let current = self.vim.borrow().buffer.ok_or(Error::Closed)?;
        let listed = self.listed(ctx)?;
        let root = cell::resolve(ctx, ROOT, "/self/dev/buffers")?;
        let candidates =
            ops::children(ctx, root)?.into_iter().filter(|draft| {
                *draft == current
                    || listed.iter().any(|item| item.draft == *draft)
            });
        let mut failure = None;
        for draft in candidates {
            let result = self.write_candidate(ctx, draft, current);
            let success = match result {
                Ok(success) => success,
                Err(error) => self.write_failed(ctx, draft, current, error)?,
            };
            self.keep_failure(success, &mut failure);
        }
        if let Some(message) = failure {
            self.fails(&message);
            return Ok(false);
        }
        Ok(true)
    }

    fn keep_failure(&self, success: bool, failure: &mut Option<String>) {
        if !success && failure.is_none() {
            *failure = Some(self.vim.borrow().message.clone());
        }
    }

    fn write_candidate(
        &self,
        ctx: &dyn Calls,
        draft: Rd,
        current: Rd,
    ) -> Result<bool, Error> {
        if !unsaved_draft(ctx, draft)? {
            return Ok(true);
        }
        if draft != current {
            return Self::wrote(ctx, draft).map(|()| true);
        }
        let unnamed = {
            let vim = self.vim.borrow();
            vim.source.is_none() && vim.name.is_none()
        };
        if unnamed {
            self.fails(&format!(
                "E141: No file name for buffer {}",
                named(ctx, draft)?
            ));
            return Ok(false);
        }
        self.save(ctx, false)
    }

    /// A write that answered `Stale`: W12 said of that draft, its own
    /// marked stale as any refused flush marks it. Any other error is
    /// the frame's.
    fn write_failed(
        &self,
        ctx: &dyn Calls,
        draft: Rd,
        current: Rd,
        error: Error,
    ) -> Result<bool, Error> {
        if error != Error::Stale {
            return Err(error);
        }
        if draft == current {
            self.stale(ctx, |_| Err(Error::Stale))?;
        }
        self.fails(&format!(
            "W12: buffer {} changed in another Client and here",
            named(ctx, draft)?
        ));
        Ok(false)
    }

    /// A hidden draft gets its own handle, closed on every save outcome.
    fn wrote(ctx: &dyn Calls, draft: Rd) -> Result<(), Error> {
        let h = ctx.open(draft, Rights::READ.union(Rights::WRITE))?;
        let result = save_draft(ctx, h);
        let closed = ops::close(ctx, h);
        result.and(closed)
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
    /// retains file drafts. A Container door refuses unsaved drafts
    /// unless forced, since the next Plugin may end the Client.
    fn explore(
        &self,
        ctx: &dyn Calls,
        name: Option<&str>,
        bang: bool,
    ) -> Result<(), Error> {
        let Some(name) = name else {
            return self.shown(ctx, self.here(ctx)?, "", bang, "").map(drop);
        };
        // Nothing is being left for a name that leads nowhere, so
        // there is nothing for the bang to override yet.
        let Some(target) = self.at(ctx, name)? else {
            self.fails(&format!("{E344} \"{name}\" in cdpath"));
            return Ok(());
        };
        self.shown(ctx, target, "", bang, "").map(drop)
    }

    /// `:e[dit][!] PATH`: the file that name holds in this region,
    /// which the Client's table gives another Vim. The name is
    /// resolved as `:explore`'s is; a missing ordinary path becomes
    /// a named draft without creating a file.
    fn edited(
        &self,
        ctx: &dyn Calls,
        name: &str,
        bang: bool,
    ) -> Result<(), Error> {
        if name == "#" {
            return self.switched(ctx, Switch::Alternate);
        }
        let (from, rest) = self.from(name);
        match cell::resolve(ctx, from, rest) {
            Ok(target) => self.shown(ctx, target, "", bang, "").map(drop),
            Err(Error::Gone) => self.edit_missing(ctx, name, bang),
            Err(Error::Unsupported) => {
                self.fails(&format!("{E345} \"{name}\" in path"));
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    fn edit_missing(
        &self,
        ctx: &dyn Calls,
        name: &str,
        bang: bool,
    ) -> Result<(), Error> {
        let Some(anchored) = self.new_name(ctx, name)? else {
            self.fails(&format!("{E345} \"{name}\" in path"));
            return Ok(());
        };
        self.shown(ctx, NONE, "", bang, &anchored).map(drop)
    }

    /// Normalize in tree coordinates before checking the mirror boundary.
    fn new_name(
        &self,
        ctx: &dyn Calls,
        name: &str,
    ) -> Result<Option<String>, Error> {
        let (from, rest) = self.from(name);
        let first = rest.split('/').next().unwrap_or_default();
        if first
            .strip_prefix("rd:")
            .and_then(|n| n.parse::<u64>().ok())
            .is_some()
        {
            return Ok(None);
        }
        let base = ops::where_is(ctx, from)?;
        let path = reovim_sdk::path::under(&base, rest);
        let root = self.vim.borrow().root.unwrap_or(ROOT);
        let root = ops::where_is(ctx, root)?;
        let prefix = format!("{}/", root.trim_start_matches('/'));
        Ok(path
            .strip_prefix(&prefix)
            .filter(|name| !name.is_empty())
            .map(String::from))
    }

    /// `:e` with no name: the Buffer read again, taking what another
    /// Client did. Without the bang a pending edit is flushed first,
    /// so a stale draft says W12 and reads nothing; the bang drops
    /// what has not gone down whatever it is. No E37: only what has
    /// not gone down is dropped, and a draft that is not stale has
    /// nothing pending at a `:` line.
    fn reload(&self, mut ctx: &dyn Calls, bang: bool) -> Result<(), Error> {
        if !bang {
            self.sync(ctx)?;
        }
        draft::Call(&mut ctx, self.handle()?).reload()?;
        self.stay()?.stale.set(false);
        self.reloaded(ctx)
    }

    /// `:cli[!]`: a shell in this region, which the Client's table
    /// gives by name and no shape gives at all. It opens where the `:`
    /// line's own lines stand, the Shell's cwd, so `:cd` moves what
    /// `:cli` opens over as it moves what `:ls` lists.
    fn cli(&self, ctx: &dyn Calls, bang: bool) -> Result<(), Error> {
        let cwd = self.vim.borrow().shell.cwd();
        self.shown(ctx, cwd, "cli", bang, "").map(drop)
    }

    /// The Client asked to show `target` in this region, in the Plugin
    /// `plugin` names or, with an empty name, in the one its table
    /// gives that rd's shape. This is the end of this Vim: the swap
    /// retains file drafts; a modified source-less draft needs force.
    /// A Container door also guards other modified drafts. False
    /// means no ask went out, for a refusal or same-source selection;
    /// the caller then stores no arrival or closing state.
    fn shown(
        &self,
        ctx: &dyn Calls,
        target: Rd,
        plugin: &str,
        bang: bool,
        name: &str,
    ) -> Result<bool, Error> {
        if Some(target) == self.vim.borrow().source && plugin.is_empty() {
            return Ok(false);
        }
        let file = target == NONE
            || !ops::info(ctx, target)?.flags.contains(Flags::CONTAINER);
        let unnamed = self.vim.borrow().source.is_none();
        if !bang && (unnamed || !file) && self.unsaved(ctx)? {
            self.fails(E37);
            return Ok(false);
        }
        if !bang && !file && self.guard_others(ctx, false)? {
            return Ok(false);
        }
        let stay = self.stay()?;
        stay.surface.region()?.asks(
            ctx,
            open::Ask {
                target,
                plugin: String::from(plugin),
                name: String::from(name),
                line: 0,
            },
        )?;
        stay.asked.set(true);
        if file && matches!(plugin, "" | "vim") {
            stay.leaving.set(true);
        }
        Ok(true)
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
        ranged(&mut shell, ex.from, ex.to, bang, ex.ranged);
        // The rest of the line fills this name's own positionals: a
        // token each at blanks and the last taking the rest, by Vim's
        // own rule, so `:w my file` is one name still. A name that
        // takes no text has no such rule, so what follows it is the
        // shell's own line — its words, its `>` and its `|` — over
        // Vim's names, which stand first on PATH.
        let taken = bins::positionals(&name);
        let ran = if taken == 0 && !arg.is_empty() {
            sh::run(ctx, &mut shell, &format!("{name} {arg}"))
        } else {
            sh::ran(ctx, &shell, &name, &cut(arg, taken))
        };
        let out = match ran {
            Ok(ran) => printed(ctx, ran),
            Err(miss) => {
                self.fails(&format!("{miss}"));
                return Ok(());
            }
        };
        self.says(&out.lines().collect::<Vec<_>>().join("  "));
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
        let moved = shell.cwd();
        self.vim.borrow_mut().shell = shell;
        match outcome {
            Ok(ran) => {
                self.says(printed(ctx, ran).lines().next().unwrap_or_default());
            }
            Err(miss) if miss.unknown => self.fails(&format!("{E492} {line}")),
            Err(miss) => self.fails(&format!("{miss}")),
        }
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
        let (from, rest) =
            path.strip_prefix('/').map_or((vim.cwd, path), |rest| {
                (vim.root, rest.trim_start_matches('/'))
            });
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

const fn closed_draft(result: Result<(), Error>) -> Result<(), Error> {
    match result {
        Ok(()) | Err(Error::Stale) => Ok(()),
        Err(error) => Err(error),
    }
}

fn other_unsaved(
    ctx: &dyn Calls,
    listed: Vec<Listed>,
    current: Option<Rd>,
) -> Result<Option<Listed>, Error> {
    for item in listed.into_iter().filter(|item| Some(item.draft) != current) {
        if unsaved_draft(ctx, item.draft)? {
            return Ok(Some(item));
        }
    }
    Ok(None)
}

impl Listed {
    fn line(
        &self,
        current: Option<Rd>,
        alternate: Option<Rd>,
        dirty: bool,
    ) -> String {
        let marker = if Some(self.draft) == current {
            '%'
        } else if Some(self.source) == alternate {
            '#'
        } else {
            ' '
        };
        let modified = if dirty { '+' } else { ' ' };
        format!("{} {marker}{modified} {}", self.name, self.path)
    }
}

fn neighbour(
    list: &[Listed],
    current: Option<Rd>,
    previous: bool,
) -> Option<Rd> {
    let at = list.iter().position(|item| Some(item.draft) == current);
    let len = list.len();
    if len == 0 {
        return None;
    }
    let index = match (at, previous) {
        (Some(at), false) => (at + 1) % len,
        (Some(at), true) => (at + len - 1) % len,
        (None, false) => 0,
        (None, true) => len - 1,
    };
    Some(list[index].source)
}

fn unsaved_draft(mut ctx: &dyn Calls, draft: Rd) -> Result<bool, Error> {
    let h = ctx.open(draft, Rights::READ)?;
    let result = history::Call(&mut ctx, h).tree();
    let closed = ops::close(ctx, h);
    let tree = result?;
    closed?;
    Ok(tree.head != tree.published)
}

fn save_draft(mut ctx: &dyn Calls, h: Handle) -> Result<(), Error> {
    let bytes = ops::whole(ctx, h)?;
    if !bytes.is_empty() && !bytes.ends_with(b"\n") {
        text::Call(&mut ctx, h).replace(text::Edit {
            at: bytes.len() as u64,
            len: 0,
            text: String::from("\n"),
        })?;
    }
    publish::Call(&mut ctx, h).save()
}

/// The file as the person named it: `name`, a path under the root,
/// read from `dir`, the directory the Client started in, when it is
/// below it, as nvim says the path it was given.
fn as_given<'a>(dir: &str, name: &'a str) -> &'a str {
    name.strip_prefix(dir)
        .and_then(|rest| rest.strip_prefix('/'))
        .unwrap_or(name)
}

/// What `start` finds once, the very first time, and every later
/// `start` leaves as `:cd` and the shell's own runs left it.
struct Kept {
    dir: String,
    root: Rd,
    cwd: Rd,
    shell: Shell,
    clip: Option<Rd>,
}

/// Where the editor stands, found from `region`: the directory the
/// Client started in, the mirror's own root, where that directory is
/// under it, a Shell over it with Vim's own `bin` first on its PATH,
/// and the clipboard rd when the Client serves one.
fn kept_of(ctx: &dyn Calls, region: &Region) -> Result<Kept, Error> {
    let dir = region.cwd(ctx)?;
    // The fs Module mirrors the directory the editor runs over, so
    // every name of the person's is under it and a leading `/` is its
    // root. A tree with no mirror leaves the editor at the tree's own
    // root, which is a place to stand.
    let root = cell::resolve(ctx, ROOT, "/module/fs/root").unwrap_or(ROOT);
    let cwd = cell::resolve(ctx, root, &dir).unwrap_or(root);
    let mut shell = Shell::at(cwd, ops::where_is(ctx, cwd)?);
    shell.first("/self/vim/bin");
    ranged(&mut shell, 0, 0, false, false);
    let clip = cell::resolve(ctx, ROOT, "/self/dev/clipboard").ok();
    Ok(Kept { dir, root, cwd, shell, clip })
}

/// What Vim registers under its home, in the order `ls` shows them,
/// after the `view` it is handed: the state it holds read out of the
/// engine, and `bin`, whose names are the `:` line's own.
///
/// Every register that can exist is registered here, filled or not,
/// because the Host asks a Container to list itself once and answers
/// from its table ever after: one first filled later would never be
/// listed.
///
/// A name under `bin` is a Container, so a run's output cell goes under
/// it, and READ, so whoever runs it may hold the right `run` asks for.
fn registered(ctx: &dyn Calls, home: Rd, view: Rd) -> Result<Cells, Error> {
    let node = |at, name: &str, flags, decls: &[Decl]| {
        let rd = ctx.register(DRIVER, at, Some(name))?;
        ctx.report(rd, flags, decls)?;
        Ok::<Rd, Error>(rd)
    };
    let read = |at, name: &str, flags| node(at, name, flags, &[]);
    let one = Flags::READ;
    let held = one.union(Flags::CONTAINER);
    let mode = read(home, "mode", one)?;
    let pending = read(home, "pending", one)?;
    let under = read(home, "registers", held)?;
    let mut registers = BTreeMap::new();
    for name in core::iter::once('"')
        .chain('0'..='9')
        .chain('a'..='z')
        .chain(core::iter::once('-'))
    {
        registers.insert(read(under, &format!("{name}"), one)?, name);
    }
    let marks = read(home, "marks", one)?;
    let jumps = read(home, "jumps", one)?;
    let recording = read(home, "recording", one)?;
    let over = read(home, "options", held)?;
    let mut options = BTreeMap::new();
    for name in ["ignorecase", "smartcase", "hlsearch", "vi", "leader"] {
        options.insert(read(over, name, one)?, name);
    }
    let keymap = read(home, "keymap", one)?;
    let point = read(home, "point", one)?;
    let bin = read(home, "bin", held)?;
    for (name, decl) in bins::declared() {
        node(bin, name, held, core::slice::from_ref(&decl))?;
    }
    Ok(Cells {
        view,
        mode,
        pending,
        registers,
        marks,
        jumps,
        recording,
        options,
        keymap,
        point,
        bin,
    })
}

/// `/self/vim/marks`: a line a mark, `NAME LINE COLUMN`, the line
/// from 1 and the column from 0 as nvim's `:marks` has them, a file
/// mark's followed by where its source is. Letters come before the
/// punctuation marks not because nvim orders them so, but because a
/// test that only reads the first row (smoke wants `a 1 0`) needs a
/// letter mark there when one is set.
fn marks_listed(ctx: &dyn Calls, vim: &Vim) -> String {
    let mut out = String::new();
    let order = ('a'..='z').chain(['\'', '.', '[', ']', '<', '>']);
    for name in order {
        let Some(&pos) = vim.marks.local.get(&name) else { continue };
        let _ = writeln!(out, "{name} {} {}", pos.line + 1, pos.column);
    }
    for (&name, &(source, pos)) in &vim.files {
        let path = ops::where_is(ctx, source).unwrap_or_default();
        let _ = writeln!(out, "{name} {} {} {path}", pos.line + 1, pos.column);
    }
    out
}

/// `/self/vim/jumps`: a line a jump, oldest first, `>` before the one
/// the list stands at and a space before the rest, then `LINE COLUMN`
/// as the `marks` cell counts them, then where a jump of another
/// source than this Buffer's is.
fn jumps_listed(ctx: &dyn Calls, vim: &Vim) -> String {
    let mut out = String::new();
    for (i, jump) in vim.jumps.list.iter().enumerate() {
        let mark = if i == vim.jumps.at { '>' } else { ' ' };
        let _ = write!(out, "{mark}{} {}", jump.pos.line + 1, jump.pos.column);
        if let Some(source) = jump.source.filter(|&s| Some(s) != vim.source) {
            let path = ops::where_is(ctx, source).unwrap_or_default();
            let _ = write!(out, " {path}");
        }
        let _ = writeln!(out);
    }
    out
}

/// The rest of a `:` line as the `count` arguments a name takes: a
/// token each at blanks and the last taking the rest. An empty rest
/// is no argument at all, and a name that takes none is handed what
/// it was given, so the run says why the line cannot stand.
fn cut(arg: &str, count: usize) -> Vec<&str> {
    let mut args = Vec::new();
    let mut rest = arg;
    while !rest.is_empty() && args.len() + 1 < count {
        let end = rest.find(char::is_whitespace).unwrap_or(rest.len());
        args.push(&rest[..end]);
        rest = rest[end..].trim_start();
    }
    if !rest.is_empty() {
        args.push(rest);
    }
    args
}

/// Vim's own language in the environment of the Shell a run reads: the
/// range and the bang, which the `:` line reads off its front before
/// the name and never spells as tokens.
///
/// They stand there from `start`, so no usage line of one of Vim's
/// names holds them either, and every line writes its own over them.
fn ranged(shell: &mut Shell, from: u64, to: u64, bang: bool, ranged: bool) {
    shell.give("ranged", Value::Bool(ranged));
    shell.give("from", Value::U64(from));
    shell.give("to", Value::U64(to));
    shell.give("bang", Value::Bool(bang));
}

/// The name an rd stands at, which is which name a call is on.
///
/// # Errors
/// As `Op::Info`; `Unsupported` for an rd with no name.
pub fn named(ctx: &dyn Calls, rd: Rd) -> Result<String, Error> {
    ops::info(ctx, rd)?.name.ok_or(Error::Unsupported)
}

/// Whether a failed lookup says the name is simply not there: a draft
/// over nothing holds no `source` row, which the Client answers
/// `Invalid` for as the Kernel's own lookup does, and one whose row
/// went while a listing walked is `Gone`.
const fn missing(error: Error) -> bool {
    matches!(error, Error::Gone | Error::Invalid)
}

/// `path`, which is under the mirror's own root, as a path of the
/// person's: the leading `/` that says to read it from there and not
/// from where the editor stands.
fn rooted(path: &str) -> String {
    format!("/{path}")
}

/// `path` from `dir`, both names under the mirror's root: the names
/// they do not share, and `..` for each of `dir`'s that is left.
fn from_dir(dir: &str, path: &str) -> String {
    let split = |at: &str| -> Vec<String> {
        at.split('/')
            .filter(|part| !part.is_empty())
            .map(String::from)
            .collect()
    };
    let (dir, path) = (split(dir), split(path));
    let same = dir.iter().zip(&path).take_while(|(a, b)| a == b).count();
    let mut out: Vec<&str> = alloc::vec![".."; dir.len() - same];
    out.extend(path[same..].iter().map(String::as_str));
    out.join("/")
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

/// Stop on a successful departure without interpreting another line.
fn init_lines(
    editor: &Editor,
    ctx: &dyn Calls,
    text: &str,
) -> Result<(), Error> {
    let mut lines = text.lines().enumerate().peekable();
    while let Some((index, line)) = lines.next() {
        if !editor.init_line(ctx, index + 1, line, lines.peek().is_some())? {
            break;
        }
    }
    Ok(())
}

/// Startup diagnostics share the Kernel's bounded log and severity levels.
fn logged(ctx: &dyn Calls, level: u8, text: &str) -> Result<(), Error> {
    let log = cell::resolve(ctx, ROOT, "/sys/log")?;
    ops::put(ctx, log, format!("<{level}>{text}").into_bytes()).map(|_| ())
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
#[cfg(test)]
mod macro_queue_tests {
    use super::*;

    #[test]
    fn recursive_tails_are_bounded_and_keep_typed_input() {
        let editor = Editor::default();
        editor.spent.set(BUDGET - 3);
        editor.queue.borrow_mut().push_back(Queued::Typed(Key::Char('x')));
        editor.replayed(alloc::vec![Key::Char('@'); 100]);
        editor.replayed(alloc::vec![Key::Char('a'); 100]);
        let queue = editor.queue.borrow();
        assert_eq!(
            queue.iter().filter(|q| matches!(q, Queued::Fed(_))).count(),
            4,
            "only the remaining budget and its sentinel"
        );
        assert!(matches!(queue.back(), Some(Queued::Typed(Key::Char('x')))));
    }

    #[test]
    fn a_key_a_failed_prefix_let_go_is_charged_no_second_time() {
        let (_rig, editor, host, _tui) = scene("bind-past-charge");
        editor.vim.borrow_mut().bind("xy", "nohlsearch");
        editor.spent.set(BUDGET - 4);
        let mut queue = editor.queue.borrow_mut();
        queue.push_back(Queued::Fed(Key::Char('x')));
        queue.push_back(Queued::Fed(Key::Char('p')));
        drop(queue);
        let ctx = host.own();
        while editor.step(&ctx).unwrap() {}
        assert_eq!(editor.engine().text(), "bac\n", "both keys ran");
        assert_eq!(
            editor.spent.get(),
            BUDGET - 2,
            "the two fed keys, and neither of the two they let go"
        );
    }

    struct Refused<'a> {
        calls: &'a dyn Calls,
        info: Option<Rd>,
        opening: Option<Rd>,
        lookup: Option<(Rd, Error)>,
    }

    impl Calls for Refused<'_> {
        fn register(
            &self,
            driver: DriverId,
            parent: Rd,
            name: Option<&str>,
        ) -> Result<Rd, Error> {
            self.calls.register(driver, parent, name)
        }
        fn name(&self, rd: Rd, name: Option<&str>) -> Result<(), Error> {
            self.calls.name(rd, name)
        }
        fn report(
            &self,
            rd: Rd,
            flags: Flags,
            apis: &[Decl],
        ) -> Result<(), Error> {
            self.calls.report(rd, flags, apis)
        }
        fn unregister(&self, rd: Rd) -> Result<(), Error> {
            self.calls.unregister(rd)
        }
        fn changed(
            &self,
            rd: Rd,
            kind: reovim_kernel_interface::Kind,
        ) -> Result<(), Error> {
            self.calls.changed(rd, kind)
        }
        fn done(
            &self,
            token: reovim_kernel_interface::Token,
            outcome: reovim_kernel_interface::Outcome,
        ) -> Result<(), Error> {
            self.calls.done(token, outcome)
        }
        fn open(&self, rd: Rd, rights: Rights) -> Result<Handle, Error> {
            self.opening.filter(|bad| *bad == rd).map_or_else(
                || self.calls.open(rd, rights),
                |_| Err(Error::Denied),
            )
        }
        fn submit(
            &self,
            op: reovim_kernel_interface::Op,
        ) -> Reply<reovim_kernel_interface::Out> {
            use reovim_kernel_interface::Op;
            let denied = Reply::err(Error::Denied);
            let blocked = self.lookup.map(|(rd, _)| rd);
            let error = self.lookup.map_or(Error::Denied, |(_, error)| error);
            let lookup = Reply::err(error);
            match op {
                Op::Info { rd } if self.info == Some(rd) => denied,
                Op::Lookup { rd, .. } if blocked == Some(rd) => lookup,
                other => self.calls.submit(other),
            }
        }
    }

    fn scene(
        name: &str,
    ) -> (
        reovim_rig::Rig,
        Rc<Editor>,
        Rc<reovim_client::Host>,
        reovim_client_tui::Tui,
    ) {
        let rig = reovim_rig::Rig::new(name);
        rig.file("a", b"abc\n");
        rig.file("b", b"def\n");
        let host = rig.host();
        let editor = Editor::new();
        let held = editor.clone();
        let table: reovim_client::dev::Table =
            alloc::boxed::Box::new(move |_| {
                Some((String::from("vim"), held.booted()))
            });
        let mut tui = reovim_rig::opened(
            &host,
            table,
            reovim_client::dev::Clip::none(),
            "",
            Some("a"),
            None,
        )
        .unwrap();
        tui.frame(80, 24).unwrap();
        (rig, editor, host, tui)
    }

    #[test]
    fn init_departure_does_not_run_queued_input_in_the_old_stay() {
        for command in ["cli", "q", "explore"] {
            let (_rig, editor, host, _tui) = scene(command);
            *editor.init.borrow_mut() = Some(String::from(command));
            editor.queue.borrow_mut().push_back(Queued::Typed(Key::Char('x')));
            editor.framed(&host.own()).unwrap();
            assert_eq!(editor.engine().text(), "abc\n", "{command}");
            assert!(editor.stay().unwrap().asked.get());
            assert!(matches!(
                editor.queue.borrow().front(),
                Some(Queued::Typed(Key::Char('x')))
            ));
        }
    }

    #[test]
    fn metadata_failure_does_not_schedule_a_buffer_switch() {
        let (_rig, editor, host, mut tui) = scene("macro-info-failure");
        let ctx = host.own();
        let b = cell::resolve(&ctx, ROOT, "/module/fs/root/b").unwrap();
        let refused =
            Refused { calls: &ctx, info: Some(b), opening: None, lookup: None };
        assert_eq!(editor.shown(&refused, b, "", true, ""), Err(Error::Denied));
        assert!(!editor.stay().unwrap().leaving.get());
        tui.tick().unwrap();
        assert_eq!(reovim_rig::lines(&mut tui), ["abc"]);
    }

    #[test]
    fn clipboard_error_discards_the_remaining_run() {
        let (_rig, editor, host, _tui) = scene("macro-clipboard-failure");
        let ctx = host.own();
        let clipboard = Rd(u64::MAX);
        editor.vim.borrow_mut().clip = Some(clipboard);
        let refused = Refused {
            calls: &ctx,
            info: None,
            opening: Some(clipboard),
            lookup: None,
        };
        editor.replayed("\"+yyx".chars().map(Key::Char).collect());
        for _ in 0..3 {
            assert_eq!(editor.step(&refused), Ok(true));
        }
        assert_eq!(editor.step(&refused), Err(Error::Denied));
        assert!(!editor.vim.borrow().macros.fed);
        assert!(editor.queue.borrow().is_empty(), "failed run was retained");
        assert_eq!(editor.step(&ctx), Ok(false));
    }

    #[test]
    fn a_refused_step_lets_go_of_a_half_held_prefix() {
        let (_rig, editor, host, _tui) = scene("bind-refused-held");
        let ctx = host.own();
        {
            let mut vim = editor.vim.borrow_mut();
            vim.bind("dx", "nohlsearch");
            // A clipboard write waiting on an rd the tree does not
            // hold: the step after this one fails, whatever its key.
            vim.clip = Some(Rd(u64::MAX));
            vim.clipboard = Some(String::from("held"));
        }
        editor.queue.borrow_mut().push_back(Queued::Typed(Key::Char('d')));
        assert!(editor.step(&ctx).is_err(), "the clipboard write failed");

        // The `d` the prefix was holding went with the queue, so the
        // next key is itself and takes its char.
        editor.queue.borrow_mut().push_back(Queued::Typed(Key::Char('x')));
        assert_eq!(editor.step(&ctx), Ok(true));
        assert_eq!(editor.engine().text(), "bc\n");
    }

    fn failed_lookup(error: Error) {
        let (_rig, editor, host, mut tui) =
            scene(&format!("buffers-lookup-{error:?}"));
        let ctx = host.own();
        let cwd = editor.vim.borrow().cwd.unwrap();
        let refused = Refused {
            calls: &ctx,
            info: None,
            opening: None,
            lookup: Some((cwd, error)),
        };
        assert_eq!(editor.edited(&refused, "missing", false), Err(error));
        tui.tick().unwrap();
        assert_eq!(reovim_rig::lines(&mut tui), ["abc"]);
    }

    #[test]
    fn denied_edit_lookup_is_not_a_missing_file() {
        failed_lookup(Error::Denied);
    }

    #[test]
    fn busy_edit_lookup_is_not_a_missing_file() {
        failed_lookup(Error::Busy);
    }
}
