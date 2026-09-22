#![no_std]
#![forbid(unsafe_code)]
//! Vim as a Module a Client loads: what the keys asked for, done.
//!
//! The resolver is [`reovim_plugin_vim`], a crate of its own, and
//! this one is its host: it takes the [`Effect`] a key answered and
//! performs it. The boundary is the compiler's own — nothing here can
//! name a field of `Vim`, only the readings the engine offers.
//!
//! ```compile_fail,E0616
//! // The host reads no field of the engine: this does not compile.
//! let engine = reovim_plugin_vim::Vim::default();
//! let _ = engine.message;
//! ```
//!
//! Nor are the host's own handles and paths the engine's to hold: the
//! shell, the root, the working directory, the clipboard's rd, the
//! draft's handle, the file a new Buffer was opened for and the
//! greeting the next stay shows are this crate's own fields, and the
//! engine has no field of any of those names for anyone to reach —
//! one block each, since a block naming eight would fail on the
//! first of them alone.
//!
//! ```compile_fail,E0609
//! let _ = reovim_plugin_vim::Vim::default().shell;
//! ```
//! ```compile_fail,E0609
//! let _ = reovim_plugin_vim::Vim::default().root;
//! ```
//! ```compile_fail,E0609
//! let _ = reovim_plugin_vim::Vim::default().cwd;
//! ```
//! ```compile_fail,E0609
//! let _ = reovim_plugin_vim::Vim::default().clip;
//! ```
//! ```compile_fail,E0609
//! let _ = reovim_plugin_vim::Vim::default().dir;
//! ```
//! ```compile_fail,E0609
//! let _ = reovim_plugin_vim::Vim::default().name;
//! ```
//! ```compile_fail,E0609
//! let _ = reovim_plugin_vim::Vim::default().h;
//! ```
//! ```compile_fail,E0609
//! let _ = reovim_plugin_vim::Vim::default().greeting;
//! ```
//!
//! The engine is pure: a key becomes an `Effect`, which has
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
//! until the next key. Where the Buffer instead has no room for the
//! state the operation would make, the draft takes the server's head
//! back, the row says which operation was refused, and the editor
//! stands: only an answer that is nobody's about this text ends it.
//!
//! The cells served are `view` and Vim's own state. A whole read of
//! `view` is one frame: captured startup lines once, then the keys Vim
//! has not taken; the flush, the rows the region has now; then the
//! View. The rest — the mode, the keys of a command that
//! has not fired, the registers — are made at the read out of what the
//! engine holds, so nothing is stored twice and nothing goes stale. No
//! borrow of the engine outlives an op, so a Driver of Vim's own that
//! a run re-enters finds the state free.

extern crate alloc;

mod bins;

use alloc::collections::{BTreeMap, VecDeque};
use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::{Cell, RefCell};
use core::fmt::Write as _;

use reovim_client_api::keyboard;
use reovim_client_api::origin::TYPED;
use reovim_client_api::resolver::Input;
use reovim_client_api::{
    NONE, buffers, draft, open, origin, part, reply, resolver,
};
use reovim_client_region::{Region, Surface, count, drafted};
use reovim_client_sh::{self as sh, Ran, Shell};
use reovim_kernel_interface::{
    Booted, Calls, Decl, Driver, DriverId, Error, Flags, Handle, Hash, Module,
    Rd, Reply, Rights,
};
use reovim_sdk::path;
use reovim_sdk::walk::ROOT;
use reovim_sdk::{cell, decode, encode, ops, progress};
use reovim_search_api::{ENTRY, entries, search};
use reovim_text_api::{history, publish, text};

use reovim_plugin_vim::bins::{abbreviated, canonical, declared};
use reovim_plugin_vim::{
    BUDGET, E32, Edit, Effect, Filled, Goto, Key, Mode, Quit, Register, Switch,
    Vim, Want, options,
};

/// How many entries one read of `matches` asks for. The 48 KiB they
/// come to is no multiple of the 64 KiB a local read copies, and
/// needs to be none.
const READ: usize = 2048;

/// How many reads one frame makes for a list nobody waits for, which
/// is `hlsearch`'s. Four slices of 2^14 are 14 to 24 ms.
const READS: usize = 4;

/// One key a macro or a `.` feeds, as the record spells it: a code,
/// carrying the press of the key that asked for it. The engine reads
/// a `Key` of it at its own entry, and the host reads none.
const fn fed(key: u64, origin: u64, from: &keyboard::Press) -> Input {
    let press = keyboard::Press { key, when: from.when, source: from.source };
    Input { press, origin, released: false }
}

/// A view held open and its `matches` found, as the opener holds
/// them. A failure here closes the handle that holds the view, which
/// is what ends it.
fn holding(ctx: &dyn Calls, view: Rd, want: &Want) -> Result<Sought, Error> {
    let held = ctx.open(view, Rights::READ)?;
    let found = || {
        let rd = cell::resolve(ctx, view, "matches")?;
        let matches = ctx.open(rd, Rights::READ)?;
        Ok::<Sought, Error>(Sought {
            want: want.clone(),
            view,
            held,
            matches,
            at: 0,
            progress: cell::resolve(ctx, view, "progress")?,
        })
    };
    found().inspect_err(|_| {
        let _ = ops::close(ctx, held);
    })
}

/// The view Vim holds ended: `matches`, then the handle whose close
/// ends the view itself.
fn unsought(ctx: &dyn Calls, stay: &Stay) {
    let Some(sought) = stay.sought.borrow_mut().take() else { return };
    let _ = ops::close(ctx, sought.matches);
    let _ = ops::close(ctx, sought.held);
}

/// The view read on by `read` bytes.
fn onward(stay: &Stay, read: usize) {
    if let Some(sought) = stay.sought.borrow_mut().as_mut() {
        sought.at += read as u64;
    }
}

/// The view a later key of the same line seeks, over the draft it was
/// opened on: `Invalid` keeps it, its old list still there for the
/// next key; anything else closes it, so the next `Want` opens
/// another the way the first key's did.
fn seeking(
    ctx: &dyn Calls,
    stay: &Stay,
    open: Rd,
    view: Rd,
    want: &Want,
) -> Result<(), Error> {
    let h = ctx.open(open, Rights::WRITE)?;
    let mut calls = ctx;
    let args = search::Seek {
        view,
        pattern: want.pattern.clone(),
        ignorecase: want.ignorecase,
        smartcase: want.smartcase,
        from: want.from,
    };
    let sought = search::Call(&mut calls, h).seek(args);
    ops::close(ctx, h)?;
    match sought {
        Ok(()) => {
            resought(stay, want);
            Ok(())
        }
        Err(Error::Invalid) => Err(Error::Invalid),
        Err(error) => {
            unsought(ctx, stay);
            Err(error)
        }
    }
}

/// A successful seek's own bookkeeping: the want it was sought for,
/// and the list it started over reset to its own head.
fn resought(stay: &Stay, want: &Want) {
    if let Some(sought) = stay.sought.borrow_mut().as_mut() {
        sought.want = want.clone();
        sought.at = 0;
    }
}

/// What a refusal of the Module's says: the two causes it does not
/// tell apart are the engine's to word.
const fn refused(error: Error) -> Filled {
    match error {
        Error::NoSpace => Filled::Spent,
        _ => Filled::Refused,
    }
}

/// What the row calls the operation an edit going down is, which is
/// what a flush of the draft carries whoever asked for it.
const EDIT: &str = "edit";

/// The Buffer operation `effect` is, as the row names it; `None` for
/// an effect that asks the Buffer nothing, whose every refusal
/// belongs to somebody else and ends the editor.
const fn asked(effect: &Effect) -> Option<&'static str> {
    match effect {
        Effect::Edits(_) => Some(EDIT),
        Effect::Undo => Some("undo"),
        Effect::Redo => Some("redo"),
        Effect::Save(_) | Effect::SaveAs(..) | Effect::SaveAll => Some("write"),
        Effect::Reload(_) => Some("reload"),
        _ => None,
    }
}

/// What `:q` says to unsaved changes.
const E37: &str = "E37: No write since last change (add ! to override)";
/// What a write says when the file it would make is there already.
const E13: &str = "E13: File exists (add ! to override)";
/// What a write says when the host would not make the file.
const E212: &str = "E212: Can't open file for writing";
/// What a write of part of a draft says when the file it would write
/// is the draft's own and no bang says to take it.
const E140: &str = "E140: Use ! to write partial buffer";
/// What a name whose grammar holds no range says to one.
const E481: &str = "E481: No range allowed";
/// Vim's own word for the range, read off a `:` line by the engine's
/// `ex`, put back on it as a flag, and named an option of the Shell
/// here so no positional ever takes it ([`kept_of`]). A record with
/// no field of this name takes no range at all, which is what the
/// shell's `undeclared` Miss says and [`Editor::ran`] words E481.
const ADDRESS: &str = "address";
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
    queue: RefCell<VecDeque<Input>>,
    spent: Cell<usize>,
    closing: Cell<Option<(Rd, Option<Rd>)>>,
    /// Captured startup lines, taken once at the first Vim frame.
    init: RefCell<Option<String>>,
    /// Where the Client started, under the root: what the announce
    /// says a new file's name by, and nothing else.
    dir: RefCell<String>,
    /// The mirror's own root, where a leading `/` starts, and where
    /// the editor stands, which `:cd` moves: both `None` until the
    /// first `start` has found them.
    root: Cell<Option<Rd>>,
    cwd: Cell<Option<Rd>>,
    /// Where a line of the shell's typed on the `:` line stands: the
    /// mirror of the directory the Client started in, which `:cd`
    /// moves and `:pwd` says.
    shell: RefCell<Shell>,
    /// `/self/dev/clipboard`, when the Client serves one.
    clip: Cell<Option<Rd>>,
    /// What a stay left for the next one to say in its opening row.
    greeting: RefCell<Option<String>>,
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
    /// The draft this stay is over, READ and WRITE: every read and
    /// write of the Buffer goes through it, and its close is the
    /// stay's end.
    h: Handle,
    /// The file a Buffer over nothing was opened for while there is
    /// no such file: what its first write makes.
    name: Option<String>,
    /// The search Module's own `open`, where this Client serves one:
    /// what a list is opened through. `None` leaves every search
    /// saying so.
    search: Option<Rd>,
    /// The view Vim holds open, and where it has read to.
    sought: RefCell<Option<Sought>>,
    /// How many reads of `matches` this frame has made: zeroed at the
    /// head of `framed`, and spent by a key's own `act`, by a wait's
    /// crank and by `wanted` alike, so no frame reads more than
    /// `READS` times however the reads are asked for.
    reads: Cell<usize>,
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

/// The view Vim holds over its own draft, as the opener holds it:
/// the key it is of, the view's rd and the handle whose close ends
/// it, the handle `matches` is read through, and how far that has
/// been read.
struct Sought {
    want: Want,
    view: Rd,
    held: Handle,
    matches: Handle,
    at: u64,
    /// The view's own `progress`, which the row's three numbers are
    /// read from after each slice. A read of it cranks nothing.
    progress: Rd,
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
    /// `/self/vim/search`: the rd of the view Vim holds, or `NONE`.
    search: Rd,
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
    /// use reovim_plugin_vim_host::Editor;
    /// let editor = Editor::with_init("set ic\n".into());
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

    /// What the engine holds now, for a test of this crate to judge
    /// what the keys did: ten readings and no engine, so nothing of
    /// the resolver this host happens to hold is in the host's own
    /// api. Each is the engine's own reading, taken and let go inside
    /// the call, no borrow of it outliving an op.
    ///
    /// # Panics
    /// While a frame of its own holds the engine, which no caller of
    /// these can be inside.
    #[must_use]
    pub fn text(&self) -> String {
        String::from(self.vim.borrow().text())
    }

    /// The cursor, as line and column.
    #[must_use]
    pub fn position(&self) -> (usize, usize) {
        self.vim.borrow().position()
    }

    /// The one record a frame reads: the mode, the keys of a command
    /// under way, and the wait it is under.
    #[must_use]
    pub fn state(&self) -> resolver::State {
        self.vim.borrow().state()
    }

    /// The row's own word.
    #[must_use]
    pub fn status(&self) -> String {
        self.vim.borrow().status()
    }

    /// What the engine says until the next key.
    #[must_use]
    pub fn message(&self) -> String {
        String::from(self.vim.borrow().said())
    }

    /// Whether a motion is waiting on the search's list.
    #[must_use]
    pub fn waiting(&self) -> bool {
        self.vim.borrow().waiting()
    }

    /// Which match the cursor is on, of how many, where the list is
    /// whole; `None` while it is not.
    #[must_use]
    pub fn counted(&self) -> Option<(usize, usize)> {
        self.vim.borrow().counted()
    }

    /// The epoch of the list the engine holds.
    #[must_use]
    pub fn epoch(&self) -> u64 {
        self.vim.borrow().epoch()
    }

    /// The input the engine took last, kept whole.
    #[must_use]
    pub fn taken(&self) -> Option<Input> {
        self.vim.borrow().taken().cloned()
    }

    /// Whether a macro is feeding keys back in.
    #[must_use]
    pub fn fed(&self) -> bool {
        self.vim.borrow().fed()
    }

    /// What the register a yank or a delete last filled holds.
    #[must_use]
    pub fn register(&self) -> Option<Register> {
        self.vim.borrow().register().cloned()
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
    /// The ask's `locator`, where there is one, lands after that
    /// line, so `reovim README.md:12` and `:e README.md:12` are one
    /// door; a locator this view cannot read says so in the row it
    /// opens with.
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
        let greeting = self.greeting.borrow_mut().take();
        let arrive = self.vim.borrow_mut().arrival();
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
        let fresh = self.root.get().is_none();
        let kept = fresh.then(|| kept_of(ctx, region)).transpose()?;
        let rows = count(region.size(ctx)?.rows);
        let bytes = ops::whole(ctx, h)?;
        let stay = Stay {
            surface,
            h,
            name: (!ask.name.is_empty()).then(|| ask.name.clone()),
            search: cell::resolve(ctx, ROOT, "/self/search/open").ok(),
            sought: RefCell::new(None),
            reads: Cell::new(0),
            cells,
            outputs: cell::Cells::default(),
            stale: Cell::new(false),
            leaving: Cell::new(false),
            asked: Cell::new(false),
        };
        *self.stay.borrow_mut() = Some(Rc::new(stay));
        let mut vim = self.vim.borrow_mut();
        vim.opened(buffer, source, &bytes, rows, ask.line, arrive);
        if let Some(greeting) = greeting {
            vim.say(greeting);
        }
        if !ask.locator.is_empty() {
            vim.goto(&ask.locator);
        }
        drop(vim);
        if let Some(kept) = kept {
            self.dir.replace(kept.dir);
            self.root.set(Some(kept.root));
            self.cwd.set(Some(kept.cwd));
            self.shell.replace(kept.shell);
            self.clip.set(kept.clip);
        }
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
        self.held(ctx, &stay, rd)
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
    fn held(&self, ctx: &dyn Calls, stay: &Stay, rd: Rd) -> Option<Vec<u8>> {
        let cells = &stay.cells;
        if rd == cells.search {
            let held = stay.sought.borrow();
            let view = held.as_ref().map_or(NONE, |sought| sought.view);
            return Some(encode(&view));
        }
        let vim = self.vim.borrow();
        let read = if rd == cells.mode {
            format!("{}\n", Mode::of(vim.state().mode).name())
        } else if rd == cells.pending {
            vim.state().showcmd
        } else if rd == cells.marks {
            marks_listed(ctx, &vim)
        } else if rd == cells.jumps {
            jumps_listed(ctx, &vim)
        } else if rd == cells.recording {
            vim.recording().map_or_else(String::new, |n| format!("{n}\n"))
        } else if let Some(name) = cells.options.get(&rd) {
            vim.shown(name)?
        } else if rd == cells.keymap {
            vim.bindings()
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
        self.stay()?.reads.set(0);
        let init = self.init.borrow_mut().take();
        if let Some(text) = init {
            self.began(ctx, &text)?;
        }
        if !self.stay()?.asked.get() {
            let keys = self.stay()?.surface.region()?.keys(ctx)?;
            self.queue.borrow_mut().extend(
                keys.into_iter().map(|press| Input {
                    press,
                    origin: TYPED,
                    released: false,
                }),
            );
        }
        while self.step(ctx)? {}
        self.wanted(ctx)?;
        // The second of the two flushes: an insert session ends at its
        // Esc, and every other edit at the frame that made it. A stay
        // already stale skips it: the row already says W12, and a
        // flush of a draft already known stale would cost a read of
        // it back for nothing.
        // A frame that ends waiting ends no revision either: a run
        // that waits at a search is one revision still, and the edit
        // typed just before it goes down at the landing.
        let state = self.vim.borrow().state();
        let insert = state.mode == Mode::Insert.code();
        if !insert && !self.stay()?.stale.get() && state.waiting == 0 {
            self.stale(ctx, Some(EDIT), |ctx| self.sync(ctx))?;
        }
        Ok(encode(&self.vim.borrow_mut().view(rows)))
    }

    /// Run captured lines once, preserving the opening frame's message.
    fn began(&self, ctx: &dyn Calls, text: &str) -> Result<(), Error> {
        let opening = self.vim.borrow_mut().saying();
        let result = init_lines(self, ctx, text);
        self.vim.borrow_mut().say(opening);
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
        let effect = self.vim.borrow_mut().init(line);
        let result = self.act(ctx, effect);
        let (message, failed) = {
            let mut vim = self.vim.borrow_mut();
            (vim.saying(), vim.rung())
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
        // While a search waits the queue stands: the frame cranks the
        // fill instead, and the keys behind it run in the frame the
        // landing happens in.
        if self.vim.borrow().waiting() {
            return self.waited(ctx);
        }
        let Some(queued) = self.queue.borrow_mut().pop_front() else {
            return Ok(false);
        };
        let effect = self.key(queued);
        let result = self.performed(ctx, effect);
        self.vim.borrow_mut().unfed();
        result?;
        self.tail();
        Ok(true)
    }

    /// One turn of a frame while a search waits: the key that ends it,
    /// or one crank of its fill. `false` where the frame has spent its
    /// reads, which is what ends the frame and leaves the rest to the
    /// next one.
    fn waited(&self, ctx: &dyn Calls) -> Result<bool, Error> {
        if let Some(cut) = self.interrupting() {
            // Drained through the key: the typed keys that stood
            // behind the search go with it, and what was typed after
            // it runs.
            self.queue.borrow_mut().drain(..=cut);
            self.vim.borrow_mut().interrupted();
            self.tail();
            return Ok(true);
        }
        if self.stay()?.reads.get() >= READS {
            return Ok(false);
        }
        let wanted = self.vim.borrow_mut().wants();
        let Some(want) = wanted else { return Ok(false) };
        let effect = self.sought(ctx, &want)?;
        self.performed(ctx, effect)?;
        self.tail();
        Ok(true)
    }

    /// Where the key that ends a wait stands in the queue: the first
    /// `<C-c>` anywhere in it, or an `<Esc>` that is its front key.
    ///
    /// An `<Esc>` behind another key belongs to that key, `ix<Esc>`
    /// typed while a search waits being an insert at the landing; a
    /// fed one is a key of its run and waits as any other does.
    fn interrupting(&self) -> Option<usize> {
        let queue = self.queue.borrow();
        let typed = |input: &Input, key: Key| {
            input.origin == TYPED && input.press.key == key.code()
        };
        let ctrl_c =
            queue.iter().position(|input| typed(input, Key::Ctrl('c')));
        let esc = queue.front().is_some_and(|input| typed(input, Key::Esc));
        ctrl_c.or_else(|| esc.then_some(0))
    }

    /// One effect performed as a key's is, with the clipboard after
    /// it; a failure leaves nothing queued behind it.
    fn performed(&self, ctx: &dyn Calls, effect: Effect) -> Result<(), Error> {
        let result =
            self.act(ctx, effect).and_then(|()| self.write_clipped(ctx));
        if result.is_err() {
            self.queue.borrow_mut().clear();
            self.vim.borrow_mut().unheld();
        }
        result
    }

    /// The tail every key and every crank has: what was let go back at
    /// the queue's front, then the bell or the budget dropping what a
    /// macro fed.
    fn tail(&self) {
        // The hand-off first: each released input carries the origin
        // it was taken with, so the sweep below reads the queue alone
        // and the engine keeps no copy to sweep beside it.
        let released = self.vim.borrow_mut().released();
        let mut queue = self.queue.borrow_mut();
        for input in released.into_iter().rev() {
            queue.push_front(input);
        }
        drop(queue);
        let exhausted = self.spent.get() >= BUDGET;
        if self.vim.borrow_mut().rung() || exhausted {
            self.queue.borrow_mut().retain(|input| input.origin == TYPED);
            self.vim.borrow_mut().cancel();
        }
        if exhausted {
            self.says(&format!("@: stopped after {BUDGET} keys"));
        }
    }

    /// One input, and what the budget makes of its origin: a typed
    /// key zeroes what a replay has spent, every key nobody typed
    /// counts, and a key a prefix let go neither.
    ///
    /// `TYPED` is the one origin that is a person's, so every other
    /// number is charged — `FED`, `REPEAT`, and any the api has not
    /// declared. A key of an unnamed origin is still a key the
    /// editor did not read from a keyboard, and reading it as free
    /// would make the bound false for every number but three.
    ///
    /// The engine sets its own fed context from the origin it takes,
    /// a released key's maker's among them, and it stands there until
    /// the whole effect of this input has been performed.
    fn key(&self, input: Input) -> Effect {
        match input.origin {
            // The input this key came back from was counted, or
            // zeroed, already.
            _ if input.released => {}
            TYPED => self.spent.set(0),
            _ => self.spent.set(self.spent.get() + 1),
        }
        self.vim.borrow_mut().key(input)
    }

    /// Keep only the fed prefix the remaining budget can reach, plus a
    /// sentinel. Typed input is never cancelled by a replay's bound.
    ///
    /// `repeat` is whether these are the keys `.` answered, which run
    /// as a repeat's: the queue carries that as far as `Vim::fed`, so
    /// a `.` inside `@a` ends where its own keys end.
    fn replayed(&self, mut keys: Vec<u64>, repeat: bool) {
        let capacity = (BUDGET + 1).saturating_sub(self.spent.get());
        keys.truncate(capacity);
        let mut spare = capacity - keys.len();
        // The press of the key that made them, which the engine holds
        // as the input it is running: the `@a` or the `.`, so the
        // host invents no keyboard for what it feeds.
        let Some(maker) = self.vim.borrow().taken().cloned() else { return };
        let origin = if repeat { origin::REPEAT } else { origin::FED };
        let mut queue = self.queue.borrow_mut();
        queue.retain(|input| {
            // A released key charges nothing, so the bound it was
            // charged under is already spent on the key it came from;
            // every other key nobody typed is one this bound holds,
            // as [`Self::key`] charges it.
            let fed = !input.released && input.origin != TYPED;
            let keep = !fed || spare != 0;
            spare = spare.saturating_sub(usize::from(fed));
            keep
        });
        for key in keys.into_iter().rev() {
            queue.push_front(fed(key, origin, &maker.press));
        }
    }

    /// The draft flushed: what it holds goes down as one revision, or
    /// `Stale` where the server refused it — read by [`Self::stale`],
    /// the one place that answer is read.
    fn sync(&self, mut ctx: &dyn Calls) -> Result<(), Error> {
        draft::Call(&mut ctx, self.handle()?).sync()
    }

    /// Every `Err` `done` may answer that the editor stands after,
    /// caught in one place.
    ///
    /// `Stale` — another Client's run landing in front of this one —
    /// is the draft read back whole, so an edit it refused is off the
    /// screen too; W12 said until the next key; the stay marked so
    /// `framed`'s own closing `sync` says it once and not every
    /// frame's.
    ///
    /// `asked` names the Buffer operation `done` is, where it is one.
    /// `NoSpace` there is the Buffer with no room for the state that
    /// operation would make, which is a word the editor stands after:
    /// [`Self::balked`]. Any other answer is `done`'s own and goes up,
    /// which ends the editor.
    fn stale(
        &self,
        ctx: &dyn Calls,
        asked: Option<&str>,
        done: impl FnOnce(&dyn Calls) -> Result<(), Error>,
    ) -> Result<(), Error> {
        let error = match done(ctx) {
            Err(error) => error,
            other => return other,
        };
        if error == Error::Stale {
            self.reloaded(ctx)?;
            self.fails(W12);
            self.stay()?.stale.set(true);
            return Ok(());
        }
        let Some(what) = asked.filter(|_| error == Error::NoSpace) else {
            return Err(error);
        };
        self.balked(ctx, what)
    }

    /// The Buffer had no room for what `what` would have made: the
    /// draft takes the server's head in place of it, so the screen is
    /// what went down; the row says which operation was refused, and
    /// rings, which ends any replay; and nothing typed behind the
    /// refusal runs, as at any other failed key.
    fn balked(&self, ctx: &dyn Calls, what: &str) -> Result<(), Error> {
        self.reload(ctx, true)?;
        let answer = resolver::Reply {
            continuation: 0,
            kind: reply::REFUSED,
            text: String::from(what),
            error: Error::NoSpace as u64,
        };
        let effect = self.vim.borrow_mut().reply(&answer);
        self.act(ctx, effect)?;
        self.queue.borrow_mut().clear();
        Ok(())
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
        self.vim.borrow_mut().resized(rows);
        Ok(rows)
    }

    /// The Buffer `start` opened; `Closed` before it has.
    fn handle(&self) -> Result<Handle, Error> {
        Ok(self.stay()?.h)
    }

    /// `message` until the next key.
    fn says(&self, message: &str) {
        self.vim.borrow_mut().say(String::from(message));
    }

    /// A refused operation ends every pending replay.
    fn fails(&self, message: &str) {
        self.vim.borrow_mut().error(message);
    }

    /// What `effect` asks of the Client, every answer the editor
    /// stands after caught in [`Self::stale`].
    fn act(&self, ctx: &dyn Calls, effect: Effect) -> Result<(), Error> {
        let asked = asked(&effect);
        self.stale(ctx, asked, |ctx| match effect {
            Effect::Nothing | Effect::Moved => Ok(()),
            Effect::Fed(fed) => {
                self.replayed(fed.keys, fed.repeat);
                Ok(())
            }
            Effect::Edits(edits) => self.send_each(ctx, edits),
            Effect::Undo => self.walk(ctx, true),
            Effect::Redo => self.walk(ctx, false),
            Effect::Save(bang) => self.save(ctx, bang).map(|_| ()),
            Effect::SaveAs(name, bang) => {
                self.save_as(ctx, &name, bang).map(|_| ())
            }
            Effect::Clipboard(continuation) => self.pasting(ctx, continuation),
            Effect::Search(want) => self.searching(ctx, want),
            Effect::Quit(how) => self.quit(ctx, how),
            Effect::Switch(to) => self.switched(ctx, to),
            Effect::Close(name, bang) => self.closed(ctx, &name, bang),
            Effect::List => self.list(ctx),
            Effect::SaveAll => self.save_all(ctx).map(drop),
            Effect::Explore(open) => {
                let name = (!open.path.is_empty()).then_some(open.path);
                self.explore(ctx, name.as_deref(), open.bang)
            }
            Effect::Open(open) => self.edited(ctx, &open.path, open.bang),
            Effect::Reload(bang) => self.reload(ctx, bang),
            Effect::Command(command) => self.ran(ctx, &command.line),
            Effect::Cli(bang) => self.cli(ctx, bang),
            Effect::Complete(line) => self.completing(ctx, &line),
            // The empty name, as `edited` uses: the Client's table
            // answers by the rd's shape, there being no second Vim to
            // ask for by name; see `arrive` for the rest.
            Effect::Goto(goto) => self.arriving(ctx, goto),
        })
    }

    /// The bridge: the view `want` names, read until the engine is
    /// answered. Every wait ends inside the turn that began it, the
    /// motion running again through `matched` each time.
    fn searching(&self, ctx: &dyn Calls, mut want: Want) -> Result<(), Error> {
        loop {
            match self.sought(ctx, &want)? {
                Effect::Search(next) => want = next,
                effect => return self.act(ctx, effect),
            }
        }
    }

    /// One turn of it: the view for `want`, one read of its
    /// `matches`, and the `Effect` the engine made of what came back.
    ///
    /// Every failure is answered to the engine before it goes up, so
    /// no wait outlives it.
    fn sought(&self, ctx: &dyn Calls, want: &Want) -> Result<Effect, Error> {
        let stay = self.stay()?;
        // The frame's own bound: with none left this reads nothing,
        // hands nothing to the engine and answers nothing, which ends
        // the loop above and leaves the rest to the next frame.
        let made = stay.reads.get();
        if made >= READS {
            return Ok(Effect::Nothing);
        }
        stay.reads.set(made + 1);
        let Some(open) = stay.search else {
            return Ok(self.answered(want.epoch, &[], Filled::Absent));
        };
        let Err(error) = self.viewing(ctx, &stay, open, want) else {
            return self.reading(ctx, &stay, want.epoch);
        };
        let filled = refused(error);
        let effect = self.answered(want.epoch, &[], filled);
        // `open`'s own two causes are the engine's to word; anything
        // else goes up, the wait already ended.
        let its_own = filled == Filled::Spent || error == Error::Invalid;
        if its_own { Ok(effect) } else { Err(error) }
    }

    /// The view Vim holds, for `want`: the one it has where that is
    /// this key's and this epoch's, sought again where it is not but
    /// there is one already, else opened fresh over the draft.
    fn viewing(
        &self,
        ctx: &dyn Calls,
        stay: &Stay,
        open: Rd,
        want: &Want,
    ) -> Result<(), Error> {
        if stay.sought.borrow().as_ref().is_some_and(|s| s.want == *want) {
            return Ok(());
        }
        let view = stay.sought.borrow().as_ref().map(|sought| sought.view);
        view.map_or_else(
            || self.viewed(ctx, stay, open, want),
            |view| seeking(ctx, stay, open, view, want),
        )
    }

    /// The view a line's first key opens, one Want's own: `open`
    /// called fresh over the draft, as `viewing` calls it where there
    /// is no view already held to seek instead.
    fn viewed(
        &self,
        ctx: &dyn Calls,
        stay: &Stay,
        open: Rd,
        want: &Want,
    ) -> Result<(), Error> {
        let source = self.vim.borrow().buffer().ok_or(Error::Closed)?;
        let h = ctx.open(open, Rights::WRITE)?;
        let mut calls = ctx;
        let args = search::Open {
            source,
            pattern: want.pattern.clone(),
            ignorecase: want.ignorecase,
            smartcase: want.smartcase,
            from: want.from,
        };
        let opened = search::Call(&mut calls, h).open(args);
        ops::close(ctx, h)?;
        let view = opened?;
        *stay.sought.borrow_mut() = Some(holding(ctx, view, want)?);
        Ok(())
    }

    /// One read of `matches` on from where the last one ended:
    /// entries are `More`, the empty read `Whole`, `Busy` more with
    /// none. A slice that failed left what it found behind it, so a
    /// read at the same place hands those on with the cause.
    fn reading(
        &self,
        ctx: &dyn Calls,
        stay: &Stay,
        epoch: u64,
    ) -> Result<Effect, Error> {
        let (h, at, progress) = {
            let held = stay.sought.borrow();
            let sought = held.as_ref().ok_or(Error::Closed)?;
            (sought.matches, sought.at, sought.progress)
        };
        let len = u32::try_from(READ * ENTRY).unwrap_or(u32::MAX);
        let (found, filled) = match ops::read(ctx, h, at, len) {
            Ok(bytes) if bytes.is_empty() => (Vec::new(), Filled::Whole),
            Ok(bytes) => {
                onward(stay, bytes.len());
                (entries(&bytes), Filled::More)
            }
            Err(Error::Busy) => (Vec::new(), Filled::More),
            Err(error @ (Error::NoSpace | Error::Invalid)) => {
                let kept = ops::read(ctx, h, at, len).unwrap_or_default();
                onward(stay, kept.len());
                (entries(&kept), refused(error))
            }
            Err(error) => {
                self.answered(epoch, &[], Filled::Refused);
                return Err(error);
            }
        };
        // The row's three numbers, while there is more to come. Both
        // reads are of a local rd and crank nothing.
        if filled == Filled::More {
            self.told(ctx, progress)?;
        }
        Ok(self.answered(epoch, &found, filled))
    }

    /// How far the fill has gone, for the row: the two numbers the
    /// view's `progress` holds, and what its own stamps make of them.
    /// A Host with no clock stamps one time for both, and there is no
    /// measure of what is left there.
    fn told(&self, ctx: &dyn Calls, progress: Rd) -> Result<(), Error> {
        let found = progress::read(ctx, progress)?;
        let info = ops::info(ctx, progress)?;
        let measured = info.mtime != info.ctime;
        let eta = measured.then(|| progress::eta(&info, &found)).flatten();
        self.vim.borrow_mut().filling(found.done, found.total, eta);
        Ok(())
    }

    /// What the engine made of one read: the matches themselves,
    /// through the search's own door, for the list the epoch names.
    /// Nothing of a match is declared — the resolver's contract holds
    /// no offset and no length — so the Module's own record is what
    /// crosses.
    fn answered(
        &self,
        epoch: u64,
        found: &[search::Match],
        filled: Filled,
    ) -> Effect {
        self.vim.borrow_mut().matched(epoch, found, filled)
    }

    /// What a frame wants once its keys have run: the view of an
    /// epoch that has gone closed, and the list `hlsearch` shows
    /// read on at most `READS` times.
    fn wanted(&self, ctx: &dyn Calls) -> Result<(), Error> {
        let stay = self.stay()?;
        let epoch = self.vim.borrow().epoch();
        let gone = stay
            .sought
            .borrow()
            .as_ref()
            .is_some_and(|sought| sought.want.epoch != epoch);
        if gone {
            unsought(ctx, &stay);
        }
        while self.filling(ctx)? {}
        Ok(())
    }

    /// One read for the list `hlsearch` shows, `false` where a frame
    /// wants none: the marks are off, the list is as full as it will
    /// get, or `:noh` hid it.
    fn filling(&self, ctx: &dyn Calls) -> Result<bool, Error> {
        if self.stay()?.reads.get() >= READS {
            return Ok(false);
        }
        let wanted = self.vim.borrow_mut().wants();
        let Some(want) = wanted else { return Ok(false) };
        let effect = self.sought(ctx, &want)?;
        self.act(ctx, effect)?;
        Ok(true)
    }

    /// `Effect::Goto`'s own arm, apart for clippy's nesting bound. A
    /// refused ask sets `arrive` nothing, so there is nothing here for
    /// a later, unrelated `leave` to mistake for one.
    fn arriving(&self, ctx: &dyn Calls, goto: Goto) -> Result<(), Error> {
        if self.shown(ctx, goto.over, "", false, "", "")? {
            self.vim.borrow_mut().arrive_at(goto);
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
        // `did` is `:s` and Vim's own `bin` names, never typing.
        let effect = self.vim.borrow_mut().ran(made);
        if let Err(error) = self.act(ctx, effect) {
            self.reloaded(ctx)?;
            return Err(error);
        }
        let said = self.vim.borrow_mut().saying();
        self.stay()?.outputs.open(ctx, DRIVER, rd, said.into_bytes())
    }

    /// A message already said, as a run's own output cell: the door a
    /// bin whose address failed before it could reach [`Self::did`]
    /// uses instead, `ran`'s own clear having nothing here to lose.
    pub(crate) fn said(
        &self,
        ctx: &dyn Calls,
        rd: Rd,
        text: String,
    ) -> Result<Rd, Error> {
        self.stay()?.outputs.open(ctx, DRIVER, rd, text.into_bytes())
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
        let source = self.vim.borrow().source();
        let (root, cwd) =
            (self.root.get().unwrap_or(ROOT), self.cwd.get().unwrap_or(ROOT));
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
            vim.say(String::new());
            let range = range.unwrap_or_else(|| (0, vim.line_count() - 1));
            vim.picked(range, spec, invert)
        };
        if picked {
            self.global_lines(ctx)?;
        }
        let said = self.vim.borrow_mut().saying();
        self.stay()?.outputs.open(ctx, DRIVER, rd, said.into_bytes())
    }

    /// Always retire the selection, including a failed call's exit.
    fn global_lines(&self, ctx: &dyn Calls) -> Result<(), Error> {
        let mut result = self.global_step(ctx);
        while result == Ok(true) {
            result = self.global_step(ctx);
        }
        let result = result.map(|_| ());
        self.vim.borrow_mut().unpicked();
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

    /// The full name `token` stands for, an exact lookup under Vim's
    /// own `bin` and [`canonical`] over it: `None` for an empty token
    /// and for one that names nothing there, which is the line the
    /// shell is handed to run, and to complete over PATH, as any
    /// other.
    ///
    /// The one lookup a `:` line's first token pays in this host,
    /// whichever door reads the line: whether the line is Vim's own,
    /// which is [`Self::ran`]'s row (a listing goes in whole only for
    /// Vim's own), and what its name is written out, which is
    /// [`Self::completing`]'s, are one answer and carried from here.
    ///
    /// No prefix is read here or anywhere in the host: `token` is
    /// already whole, one abbreviation among [`abbreviated`]'s own or
    /// a full name, and the shell's own lookup under `bin` is exact.
    fn vims_own<'a>(
        &self,
        ctx: &dyn Calls,
        token: &'a str,
    ) -> Result<Option<&'a str>, Error> {
        if token.is_empty() {
            return Ok(None);
        }
        if ops::lookup(ctx, self.stay()?.cells.bin, token).is_err() {
            return Ok(None);
        }
        Ok(Some(canonical(token)))
    }

    /// `edit` into the revision the draft holds. The first of the two
    /// flushes: one that does not join ends the revision that was
    /// there, whatever made it.
    fn send(&self, mut ctx: &dyn Calls, edit: Edit) -> Result<(), Error> {
        let Edit { at, len, text, join } = edit;
        if !join {
            self.sync(ctx)?;
        }
        let edit = text::Edit { at, len, text };
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
        let name = self.stay()?.name.clone();
        let Some(name) = name else { return Ok(()) };
        let dir = self.dir.borrow().clone();
        // The ask's name is under the mirror's own root, wherever the
        // editor stands, so it is read from there.
        let at = rooted(name.rsplit_once('/').map_or("", |(at, _)| at));
        let word =
            if self.directory(ctx, &at)? { "[New]" } else { "[New DIRECTORY]" };
        let said = as_given(&dir, &name);
        self.vim.borrow_mut().say(format!("\"{said}\" {word}"));
        Ok(())
    }

    /// The head published to its source; a Buffer over nothing writes
    /// the file it was opened for, and has no file name without one.
    /// False when it said why it did not write.
    fn save(&self, mut ctx: &dyn Calls, bang: bool) -> Result<bool, Error> {
        let source = self.vim.borrow().source();
        let name = self.stay()?.name.clone();
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
        if Some(target) == self.vim.borrow().source() {
            return self.save(ctx, bang);
        }
        self.published_to(ctx, target)
    }

    /// The file `path` names made under the root and the head written
    /// to it.
    fn written_to(
        &self,
        ctx: &dyn Calls,
        path: &str,
        bang: bool,
    ) -> Result<bool, Error> {
        self.onto(ctx, path, bang, |target| self.published_to(ctx, target))
    }

    /// The file `path` names made under the root — every missing
    /// directory on the way by `mkdir`, then `create` — and `write`
    /// run over it. A file that appeared since is E13 unless `bang`
    /// says to write over it; anything that refuses to make the file,
    /// the host and a file where a directory is wanted alike, is E212.
    fn onto(
        &self,
        ctx: &dyn Calls,
        path: &str,
        bang: bool,
        write: impl FnOnce(Rd) -> Result<bool, Error>,
    ) -> Result<bool, Error> {
        match self.making(ctx, path, bang) {
            Ok(Some(target)) => write(target),
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

    /// `:{range}w PATH`: `text`, the lines an address named, written
    /// to that file and nothing else, `PATH` resolved where `:w NAME`
    /// resolves it. The draft is not published and no edit is sent,
    /// so its source, its history and its modified flag stand exactly
    /// as they were — a ranged write is not a save, which is what
    /// nvim leaves behind.
    pub(crate) fn lines_to(
        &self,
        ctx: &dyn Calls,
        path: &str,
        bang: bool,
        text: String,
    ) -> Result<(), Error> {
        self.onto(ctx, path, bang, |target| {
            ops::put(ctx, target, text.into_bytes()).map(|_| true)
        })
        .map(drop)
    }

    /// `:{range}w!` with no path: those same lines over the draft's
    /// own file, which is where nvim puts them and which only the
    /// bang allows (E140 otherwise, said by the run). A Buffer over
    /// nothing writes the file it was opened for and has no file name
    /// without one, as [`Self::save`] has it.
    pub(crate) fn lines_over(
        &self,
        ctx: &dyn Calls,
        bang: bool,
        text: String,
    ) -> Result<(), Error> {
        let source = self.vim.borrow().source();
        if let Some(source) = source {
            return ops::put(ctx, source, text.into_bytes()).map(drop);
        }
        let Some(name) = self.stay()?.name.clone() else {
            self.fails(E32);
            return Ok(());
        };
        self.lines_to(ctx, &rooted(&name), bang, text)
    }

    /// Whether `path` names the very file the draft is over — nvim's
    /// "the current file", which a partial write of the draft reaches
    /// only under a bang (E140 otherwise), where another file that is
    /// there already is E13.
    pub(crate) fn own(
        &self,
        ctx: &dyn Calls,
        path: &str,
    ) -> Result<bool, Error> {
        let source = self.vim.borrow().source();
        Ok(source.is_some() && self.at(ctx, path)? == source)
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

    /// A put waiting for the clipboard: what it holds, answered to
    /// the continuation the put asked with, and the put itself, which
    /// is an effect like any other.
    ///
    /// Every end of the read answers, since the put is what waits: a
    /// read the host cannot make `PASTED` is a `REFUSED` reply
    /// carrying the error it met, and the engine words it and clears
    /// the put. Leaving here without a reply — which is what `?` on
    /// the read did — would leave the put waiting for the rest of the
    /// stay, `State.waiting` never back at 0, the frame's sync
    /// skipped and nothing typed afterwards reaching the server.
    fn pasting(&self, ctx: &dyn Calls, continuation: u64) -> Result<(), Error> {
        let (text, failed) = match self.clipboard(ctx) {
            Ok(text) => (text, None),
            Err(error) => (String::new(), Some(error)),
        };
        let answer = resolver::Reply {
            continuation,
            kind: if failed.is_none() { reply::PASTED } else { reply::REFUSED },
            text,
            error: failed.map_or(0, |error| error as u64),
        };
        let effect = self.vim.borrow_mut().reply(&answer);
        self.act(ctx, effect)
    }

    /// What the clipboard holds, through its rd; the error the read
    /// met otherwise, `Unsupported` where the Client serves no cell at
    /// all, as a machine with no tool on PATH answers.
    fn clipboard(&self, ctx: &dyn Calls) -> Result<String, Error> {
        let rd = self.clip.get().ok_or(Error::Unsupported)?;
        let bytes = ops::bytes(ctx, rd)?;
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    /// What a `"+` yank left for the clipboard, written to its rd; no
    /// cell or no tool is a message.
    fn write_clipped(&self, ctx: &dyn Calls) -> Result<(), Error> {
        let (text, clip) = (self.vim.borrow_mut().clipped(), self.clip.get());
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
        let (root, dir) =
            (self.root.get().unwrap_or(ROOT), self.dir.borrow().clone());
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
            (vim.buffer(), vim.alternate())
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
        let current = self.vim.borrow().buffer();
        let list = self.listed(ctx)?;
        let source = match to {
            Switch::Alternate => self.alternate(),
            Switch::Named(name) if name == "#" => self.alternate(),
            Switch::Named(name) => self.by_name(&list, &name),
            Switch::Next => neighbour(&list, current, false),
            Switch::Previous => neighbour(&list, current, true),
        };
        source.map_or(Ok(()), |source| {
            self.shown(ctx, source, "", false, "", "").map(drop)
        })
    }

    fn alternate(&self) -> Option<Rd> {
        let alternate = self.vim.borrow().alternate();
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
        let current = self.vim.borrow().buffer();
        let Some(other) = other_unsaved(ctx, self.listed(ctx)?, current)?
        else {
            return Ok(false);
        };
        let message = format!(
            "E162: No write since last change for buffer \"{}\"",
            other.path
        );
        self.fails(&message);
        if follow && self.shown(ctx, other.source, "", true, "", "")? {
            *self.greeting.borrow_mut() = Some(message);
        }
        Ok(true)
    }

    fn closed(
        &self,
        mut ctx: &dyn Calls,
        name: &str,
        bang: bool,
    ) -> Result<(), Error> {
        let current = self.vim.borrow().buffer().ok_or(Error::Closed)?;
        let list = self.listed(ctx)?;
        let own = name.is_empty() || named(ctx, current)? == name;
        let selected = list.iter().find(|item| item.name == name);
        let (draft, source) = match (own, selected) {
            (true, _) => (current, self.vim.borrow().source()),
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
        let alternate = self.vim.borrow().alternate();
        let to = list
            .iter()
            .find(|item| Some(item.source) == alternate)
            .map(|item| item.source)
            .or_else(|| neighbour(list, Some(draft), false))
            .filter(|to| Some(*to) != source)
            .unwrap_or(NONE);
        if self.shown(ctx, to, "", true, "", "")? {
            self.closing.set(Some((draft, source)));
        }
        Ok(())
    }

    fn save_all(&self, ctx: &dyn Calls) -> Result<bool, Error> {
        let current = self.vim.borrow().buffer().ok_or(Error::Closed)?;
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
            *failure = Some(String::from(self.vim.borrow().said()));
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
        let unnamed =
            self.vim.borrow().source().is_none() && self.stay()?.name.is_none();
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
            self.stale(ctx, None, |_| Err(Error::Stale))?;
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
        if self.vim.borrow().source().is_none() && !unsaved {
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
            return self
                .shown(ctx, self.here(ctx)?, "", bang, "", "")
                .map(drop);
        };
        // Nothing is being left for a name that leads nowhere, so
        // there is nothing for the bang to override yet.
        let Some(target) = self.at(ctx, name)? else {
            self.fails(&format!("{E344} \"{name}\" in cdpath"));
            return Ok(());
        };
        self.shown(ctx, target, "", bang, "", "").map(drop)
    }

    /// `:e[dit][!] PATH[:LOCATOR]`: the file that name holds in this
    /// region, which the Client's table gives another Vim. The name is
    /// resolved as `:explore`'s is; a missing ordinary path becomes
    /// a named draft without creating a file.
    ///
    /// A file of the whole name wins, so `:e a:b` over a file `a:b`
    /// opens it; only a name no file holds is split. `gF` comes in
    /// here too, its `PATH:LOCATOR` being the one the person typed.
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
            Ok(target) => self.opening(ctx, target, bang, ""),
            Err(Error::Gone) => self.located(ctx, name, bang),
            Err(Error::Unsupported) => {
                self.fails(&format!("{E345} \"{name}\" in path"));
                Ok(())
            }
            Err(error) => Err(error),
        }
    }

    /// The whole name was no file: `PATH:LOCATOR` when a file of the
    /// path is one, else the name is the new file it asks for.
    fn located(
        &self,
        ctx: &dyn Calls,
        name: &str,
        bang: bool,
    ) -> Result<(), Error> {
        let (path, Some(locator)) = path::split(name) else {
            return self.edit_missing(ctx, name, bang);
        };
        let (from, rest) = self.from(path);
        let Ok(target) = cell::resolve(ctx, from, rest) else {
            return self.edit_missing(ctx, name, bang);
        };
        self.opening(ctx, target, bang, locator)
    }

    /// The region over `target`, landing where `locator` says. The
    /// file already shown is not asked for again (see [`Self::shown`]),
    /// so its locator lands here instead of at a next stay's first
    /// frame: `:e README.md:12` goes to line 12 either way.
    fn opening(
        &self,
        ctx: &dyn Calls,
        target: Rd,
        bang: bool,
        locator: &str,
    ) -> Result<(), Error> {
        if !self.shown(ctx, target, "", bang, "", locator)?
            && !locator.is_empty()
        {
            self.vim.borrow_mut().goto(locator);
        }
        Ok(())
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
        self.shown(ctx, NONE, "", bang, &anchored, "").map(drop)
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
        let root = ops::where_is(ctx, self.root.get().unwrap_or(ROOT))?;
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
        let cwd = self.shell.borrow().cwd();
        self.shown(ctx, cwd, "cli", bang, "", "").map(drop)
    }

    /// The Client asked to show `target` in this region, in the Plugin
    /// `plugin` names or, with an empty name, in the one its table
    /// gives that rd's shape. This is the end of this Vim: the swap
    /// retains file drafts; a modified source-less draft needs force.
    /// A Container door also guards other modified drafts. False
    /// means no ask went out, for a refusal or same-source selection;
    /// the caller then stores no arrival or closing state.
    ///
    /// `locator` rides in the ask to whatever Plugin serves the
    /// target, which reads it at its own first frame; nothing here
    /// reads it.
    fn shown(
        &self,
        ctx: &dyn Calls,
        target: Rd,
        plugin: &str,
        bang: bool,
        name: &str,
        locator: &str,
    ) -> Result<bool, Error> {
        if Some(target) == self.vim.borrow().source() && plugin.is_empty() {
            return Ok(false);
        }
        let file = target == NONE
            || !ops::info(ctx, target)?.flags.contains(Flags::CONTAINER);
        let unnamed = self.vim.borrow().source().is_none();
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
                locator: String::from(locator),
            },
        )?;
        stay.asked.set(true);
        if file && matches!(plugin, "" | "vim") {
            stay.leaving.set(true);
        }
        Ok(true)
    }

    /// A `:` line the shell is to run, whole — Vim's own bin names
    /// stand first on the Shell's PATH (`kept_of`), so the one call
    /// reaches either kind alike, and a first token PATH has not is
    /// E492, which is Vim's own; a range on a name whose record
    /// declares no `address` is E481, which is Vim's own too, the
    /// shell having Missed on a flag the declaration lacks and said
    /// so; every other `Miss` is said as the shell says it. What Vim
    /// would have said arrives as the run's
    /// output cell, every one of its lines joined into the row, as
    /// `:buffers` needs for more than its own first; a line of the
    /// shell's own — a first token none of Vim's names is — shows
    /// only its first, a row having nowhere to put a listing whole.
    ///
    /// The Shell is a copy for the run, taken and not borrowed: the
    /// Driver it reaches is this Plugin, whose state the key that
    /// typed the line is running over, and which finds it as it left
    /// it — `:cli`'s own read of where it stands among them, still
    /// mid-run, `self.shell` held by no borrow to conflict with.
    fn ran(&self, ctx: &dyn Calls, line: &str) -> Result<(), Error> {
        let first = line.split_whitespace().next().unwrap_or_default();
        let vims_own = self.vims_own(ctx, first)?.is_some();
        let mut shell = self.shell.borrow().clone();
        let was = shell.cwd();
        let outcome = sh::run(ctx, &mut shell, line);
        let moved = shell.cwd();
        *self.shell.borrow_mut() = shell;
        match outcome {
            Ok(ran) => self.says(&said_of(&printed(ctx, ran), vims_own)),
            Err(miss) if miss.unknown => self.fails(&format!("{E492} {line}")),
            Err(miss) if no_range(&miss) => self.fails(E481),
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
        let root = self.root.get().unwrap_or(ROOT);
        let mut at = cwd;
        while at != root && at != ROOT {
            at = ops::parent(ctx, at)?;
        }
        if at == root {
            self.cwd.set(Some(cwd));
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
        let shell = self.shell.borrow().clone();
        let Some((first, rest)) = line.split_once(' ') else {
            let found = sh::complete(ctx, &shell, line);
            let candidates = canonical_only(found.candidates);
            let alone = match &candidates[..] {
                [one] => sh::form(ctx, &shell, one),
                _ => None,
            };
            self.vim.borrow_mut().listed(found.from, candidates, alone);
            return Ok(());
        };
        let Some(name) = self.vims_own(ctx, first)? else {
            let found = sh::complete(ctx, &shell, line);
            self.vim.borrow_mut().offer(found.from, found.candidates);
            return Ok(());
        };
        let line = format!("{name} {rest}");
        let from = line.rfind(' ').map_or(0, |at| at + 1);
        let word = String::from(&line[from..]);
        self.vim.borrow_mut().expand(&line);
        let found = match name {
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
        let source = self.vim.borrow().source();
        let cwd = self.cwd.get().unwrap_or(ROOT);
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
        let (from, rest) = path.strip_prefix('/').map_or_else(
            || (self.cwd.get(), path),
            |rest| (self.root.get(), rest.trim_start_matches('/')),
        );
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
/// under it, a Shell over it with Vim's own `bin` first on its PATH
/// and Vim's own `address` named an option of it, and the clipboard
/// rd when the Client serves one.
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
    // Vim's own word, said by Vim's own host: the address `ex` reads
    // off a `:` line and puts back on it as `--address` is never a
    // token a positional takes, which no shape of a declaration says
    // and the shell is right not to guess.
    shell.option(ADDRESS);
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
    let search = read(home, "search", one)?;
    let bin = read(home, "bin", held)?;
    let full = declared();
    for (name, decl) in &full {
        node(bin, name, held, core::slice::from_ref(decl))?;
    }
    // Every abbreviation nvim takes, the same declaration as the full
    // name it stands for: the shell's PATH lookup is exact, so each
    // is its own node and not a prefix resolved at the call.
    for (short, canon) in abbreviated() {
        let (_, decl) = full
            .iter()
            .find(|(name, _)| *name == canon)
            .expect("an abbreviation stands for a name `declared` also has");
        node(bin, short, held, core::slice::from_ref(decl))?;
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
        search,
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
    for (name, pos, source) in vim.marks() {
        let _ = write!(out, "{name} {} {}", pos.line + 1, pos.column);
        if let Some(source) = source {
            let path = ops::where_is(ctx, source).unwrap_or_default();
            let _ = write!(out, " {path}");
        }
        let _ = writeln!(out);
    }
    out
}

/// `/self/vim/jumps`: a line a jump, oldest first, `>` before the one
/// the list stands at and a space before the rest, then `LINE COLUMN`
/// as the `marks` cell counts them, then where a jump of another
/// source than this Buffer's is.
fn jumps_listed(ctx: &dyn Calls, vim: &Vim) -> String {
    let mut out = String::new();
    let (at, list) = vim.jumps();
    for (i, (pos, elsewhere)) in list.into_iter().enumerate() {
        let mark = if i == at { '>' } else { ' ' };
        let _ = write!(out, "{mark}{} {}", pos.line + 1, pos.column);
        if let Some(source) = elsewhere {
            let path = ops::where_is(ctx, source).unwrap_or_default();
            let _ = write!(out, " {path}");
        }
        let _ = writeln!(out);
    }
    out
}

/// The name an rd stands at, which is which name a call is on.
///
/// # Errors
/// As `Op::Info`; `Unsupported` for an rd with no name.
pub fn named(ctx: &dyn Calls, rd: Rd) -> Result<String, Error> {
    ops::info(ctx, rd)?.name.ok_or(Error::Unsupported)
}

/// Whether `miss` is a range handed to a name whose grammar has none:
/// the shell says a token under a dash names no field of the record,
/// and `--address` is the one such token Vim's own `ex` ever writes,
/// the word being the host's and named here and at [`kept_of`] alone.
fn no_range(miss: &sh::Miss) -> bool {
    miss.undeclared && miss.target.strip_prefix("--") == Some(ADDRESS)
}

/// A `ran` run's own output cell, `out`, read for the row: every line
/// joined, what Vim itself would have said needing more than its
/// first, as `:buffers` does; the shell's own line, a first token none
/// of Vim's own names, shows only its own first, a row having nowhere
/// to put a listing whole.
fn said_of(out: &str, whole: bool) -> String {
    if whole {
        out.lines().collect::<Vec<_>>().join("  ")
    } else {
        String::from(out.lines().next().unwrap_or_default())
    }
}

/// `candidates` without one of Vim's own abbreviations: Tab offers
/// what a person may type in full, `write` and `watch` alike, and not
/// every shorter node the shell's exact lookup also finds — a
/// candidate [`canonical`] answers back changed is one of those and
/// is dropped; one from another directory of PATH, `canonical`
/// finding no abbreviation of its own, answers itself and stands.
fn canonical_only(candidates: Vec<String>) -> Vec<String> {
    candidates.into_iter().filter(|name| canonical(name) == name).collect()
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

    /// One key of `origin`, its press of no clock and no source: what
    /// the engine's own tests call `tapped`, which is the engine's and
    /// not this crate's to hand out.
    fn tapped(key: Key, origin: u64) -> Input {
        let press = keyboard::Press { key: 0, when: 0, source: NONE };
        reovim_plugin_vim::keyed(key, origin, &press)
    }

    #[test]
    fn recursive_tails_are_bounded_and_keep_typed_input() {
        let editor = Editor::default();
        // The press its keys carry: the input the engine is running,
        // which here is one key a person typed and nothing else.
        editor.vim.borrow_mut().key(tapped(Key::Esc, TYPED));
        editor.spent.set(BUDGET - 3);
        editor.queue.borrow_mut().push_back(tapped(Key::Char('x'), TYPED));
        editor.replayed(alloc::vec![Key::Char('@').code(); 100], false);
        editor.replayed(alloc::vec![Key::Char('a').code(); 100], false);
        let queue = editor.queue.borrow();
        assert_eq!(
            queue.iter().filter(|q| q.origin == origin::FED).count(),
            4,
            "only the remaining budget and its sentinel"
        );
        assert_eq!(queue.back(), Some(&tapped(Key::Char('x'), TYPED)));
    }

    /// A `<C-c>` a macro fed is a key of its run and waits as any
    /// other does; a person's ends the wait wherever it stands.
    #[test]
    fn a_fed_ctrl_c_does_not_end_a_wait() {
        let (_rig, editor, _host, _tui) = scene("wait-fed-ctrl-c");
        let mut queue = editor.queue.borrow_mut();
        queue.push_back(tapped(Key::Ctrl('c'), origin::FED));
        drop(queue);
        assert_eq!(editor.interrupting(), None);
        editor.queue.borrow_mut().push_back(tapped(Key::Ctrl('c'), TYPED));
        assert_eq!(editor.interrupting(), Some(1));
    }

    /// The hand-off is the whole batch at once, in the order the keys
    /// were taken, and the engine keeps no copy of what it let go —
    /// so a frame that ends no key, a crank of a waiting search, does
    /// not queue them again.
    #[test]
    fn a_release_batch_is_queued_whole_in_order() {
        let (_rig, editor, _host, _tui) = scene("bind-released-batch");
        let mut vim = editor.vim.borrow_mut();
        vim.bind("dxy", "nohlsearch");
        for key in [Key::Char('d'), Key::Char('x'), Key::Char('z')] {
            vim.key(tapped(key, TYPED));
        }
        drop(vim);
        editor.tail();
        let queued = || -> Vec<u64> {
            editor.queue.borrow().iter().map(|input| input.press.key).collect()
        };
        let keys: Vec<u64> = [Key::Char('d'), Key::Char('x'), Key::Char('z')]
            .into_iter()
            .map(Key::code)
            .collect();
        assert_eq!(queued(), keys, "the whole batch, in the order taken");
        editor.tail();
        assert_eq!(queued(), keys, "and the engine keeps no second copy");
    }

    /// The replay bound keeps the fed prefix the remaining budget can
    /// reach; a released key charges nothing, so it is no part of
    /// that count and stands where a fed key past the bound goes.
    #[test]
    fn the_replay_bound_does_not_count_a_key_a_prefix_let_go() {
        let editor = Editor::default();
        editor.vim.borrow_mut().key(tapped(Key::Esc, TYPED));
        editor.spent.set(BUDGET);
        let released =
            Input { released: true, ..tapped(Key::Char('h'), origin::FED) };
        let mut queue = editor.queue.borrow_mut();
        queue.push_back(released.clone());
        queue.push_back(tapped(Key::Char('l'), origin::FED));
        drop(queue);
        editor.replayed(alloc::vec![Key::Char('j').code()], false);
        let queue = editor.queue.borrow();
        assert_eq!(
            queue.iter().skip(1).collect::<Vec<_>>(),
            [&released],
            "the fed key past the bound went; the released key stands"
        );
    }

    /// The queue alone says whose a released key is, so the bell and
    /// the budget sweep it by `origin` after the hand-off: what a
    /// macro fed goes with its run, what a person typed stands.
    #[test]
    fn a_bell_sweeps_the_queue_by_whose_a_released_key_is() {
        let (_rig, editor, _host, _tui) = scene("bind-released-bell");
        let mut vim = editor.vim.borrow_mut();
        vim.bind("dx", "nohlsearch");
        vim.key(tapped(Key::Char('d'), TYPED));
        vim.key(tapped(Key::Char('d'), TYPED));
        drop(vim);
        editor.spent.set(BUDGET);
        editor.tail();
        let origins = || -> Vec<u64> {
            editor.queue.borrow().iter().map(|input| input.origin).collect()
        };
        assert_eq!(origins(), [TYPED, TYPED], "a person's keys stand");
        assert!(
            editor.queue.borrow().iter().all(|input| input.released),
            "each of them a key the prefix let go"
        );

        editor.queue.borrow_mut().clear();
        let mut vim = editor.vim.borrow_mut();
        vim.key(tapped(Key::Char('d'), origin::FED));
        vim.key(tapped(Key::Char('d'), origin::FED));
        drop(vim);
        editor.tail();
        assert_eq!(origins(), [], "a macro's go with its run");
    }

    /// An origin no constant names is a key nobody typed: the budget
    /// charges it as it charges a fed one and the bell drops it with
    /// the rest of a run. Reading an unnamed number as "typed" would
    /// make the declared rule false for every value but three, and a
    /// replay that spelled one would edit for ever.
    #[test]
    fn an_origin_no_constant_names_is_charged_as_synthetic() {
        // A number no constant of the api's names.
        const OTHER: u64 = 7;
        let (_rig, editor, host, _tui) = scene("origin-unnamed");
        editor.spent.set(BUDGET - 1);
        let mut queue = editor.queue.borrow_mut();
        queue.push_back(tapped(Key::Char('x'), OTHER));
        queue.push_back(tapped(Key::Char('x'), OTHER));
        drop(queue);
        let ctx = host.own();
        while editor.step(&ctx).unwrap() {}
        assert_eq!(editor.spent.get(), BUDGET, "the first key was charged");
        assert_eq!(editor.text(), "bc\n", "and the second never ran");
        assert!(editor.queue.borrow().is_empty(), "the bound swept it");
    }

    #[test]
    fn a_key_a_failed_prefix_let_go_is_charged_no_second_time() {
        let (_rig, editor, host, _tui) = scene("bind-past-charge");
        editor.vim.borrow_mut().bind("xy", "nohlsearch");
        editor.spent.set(BUDGET - 4);
        let mut queue = editor.queue.borrow_mut();
        queue.push_back(tapped(Key::Char('x'), origin::FED));
        queue.push_back(tapped(Key::Char('p'), origin::FED));
        drop(queue);
        let ctx = host.own();
        while editor.step(&ctx).unwrap() {}
        assert_eq!(editor.text(), "bac\n", "both keys ran");
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

    /// The scene every case below stands on: `a` open, `b` beside it.
    fn scene(
        name: &str,
    ) -> (
        reovim_rig::Rig,
        Rc<Editor>,
        Rc<reovim_client::Host>,
        reovim_client_tui::Tui,
    ) {
        staged(reovim_rig::Rig::new(name))
    }

    /// The same scene over a text Module with room for `a` and no
    /// more: its four bytes are the whole limit, so the state a first
    /// edit would make is refused `NoSpace`.
    fn refusing(
        name: &str,
    ) -> (
        reovim_rig::Rig,
        Rc<Editor>,
        Rc<reovim_client::Host>,
        reovim_client_tui::Tui,
    ) {
        staged(reovim_rig::Rig::holding(name, b"abc\n".len()))
    }

    fn staged(
        rig: reovim_rig::Rig,
    ) -> (
        reovim_rig::Rig,
        Rc<Editor>,
        Rc<reovim_client::Host>,
        reovim_client_tui::Tui,
    ) {
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

    /// The Buffer refuses the state an `x` would make: the row says
    /// so, the text on the screen is what went down, nothing is left
    /// queued and the editor takes the next key.
    #[test]
    fn an_edit_the_buffer_refuses_is_a_word_and_the_editor_stands() {
        let (_rig, editor, _host, mut tui) = refusing("refused-edit");
        assert_eq!(reovim_rig::lines(&mut tui), ["abc"]);
        let step = reovim_rig::typed(&mut tui, "x");
        assert_eq!(step, reovim_client_tui::Step::Changed);
        assert_eq!(reovim_rig::said(&mut tui), "E: edit refused: no space");
        assert_eq!(reovim_rig::lines(&mut tui), ["abc"], "the text stands");
        assert!(editor.queue.borrow().is_empty(), "the queue stood");
        // The editor is still here: the next key runs, and says its
        // own word over the refusal's.
        reovim_rig::typed(&mut tui, ":nohlsearch\r");
        assert_eq!(reovim_rig::lines(&mut tui), ["abc"]);
        assert_ne!(reovim_rig::said(&mut tui), "E: edit refused: no space");
    }

    /// Keys behind a refused edit do not run: the second `x` flushes
    /// what the first spliced before its own edit, and that flush is
    /// what the Buffer refuses, so the `l` behind it goes with the
    /// queue and the row keeps the refusal's word.
    #[test]
    fn the_keys_behind_a_refused_edit_go_with_the_queue() {
        let (_rig, editor, host, _tui) = refusing("refused-queue");
        let keys = ['x', 'x', 'l'].map(|c| tapped(Key::Char(c), TYPED));
        editor.queue.borrow_mut().extend(keys);
        editor.framed(&host.own()).unwrap();
        assert_eq!(editor.text(), "abc\n", "nothing went down");
        assert!(editor.queue.borrow().is_empty(), "a key behind it stood");
        assert_eq!(
            editor.message(),
            "E: edit refused: no space",
            "a key behind the refusal ran and said its own word"
        );
    }

    #[test]
    fn init_departure_does_not_run_queued_input_in_the_old_stay() {
        for command in ["cli", "q", "explore"] {
            let (_rig, editor, host, _tui) = scene(command);
            *editor.init.borrow_mut() = Some(String::from(command));
            editor.queue.borrow_mut().push_back(tapped(Key::Char('x'), TYPED));
            editor.framed(&host.own()).unwrap();
            assert_eq!(editor.text(), "abc\n", "{command}");
            assert!(editor.stay().unwrap().asked.get());
            assert_eq!(
                editor.queue.borrow().front(),
                Some(&tapped(Key::Char('x'), TYPED))
            );
        }
    }

    #[test]
    fn metadata_failure_does_not_schedule_a_buffer_switch() {
        let (_rig, editor, host, mut tui) = scene("macro-info-failure");
        let ctx = host.own();
        let b = cell::resolve(&ctx, ROOT, "/module/fs/root/b").unwrap();
        let refused =
            Refused { calls: &ctx, info: Some(b), opening: None, lookup: None };
        assert_eq!(
            editor.shown(&refused, b, "", true, "", ""),
            Err(Error::Denied)
        );
        assert!(!editor.stay().unwrap().leaving.get());
        tui.tick().unwrap();
        assert_eq!(reovim_rig::lines(&mut tui), ["abc"]);
    }

    #[test]
    fn clipboard_error_discards_the_remaining_run() {
        let (_rig, editor, host, _tui) = scene("macro-clipboard-failure");
        let ctx = host.own();
        let clipboard = Rd(u64::MAX);
        editor.clip.set(Some(clipboard));
        let refused = Refused {
            calls: &ctx,
            info: None,
            opening: Some(clipboard),
            lookup: None,
        };
        // The press its keys carry: the input the engine is running,
        // which here is one key a person typed and nothing else.
        editor.vim.borrow_mut().key(tapped(Key::Esc, TYPED));
        let run = "\"+yyx".chars().map(|c| Key::Char(c).code()).collect();
        editor.replayed(run, false);
        for _ in 0..3 {
            assert_eq!(editor.step(&refused), Ok(true));
        }
        assert_eq!(editor.step(&refused), Err(Error::Denied));
        assert!(!editor.fed());
        assert!(editor.queue.borrow().is_empty(), "failed run was retained");
        assert_eq!(editor.step(&ctx), Ok(false));
    }

    #[test]
    fn a_refused_step_lets_go_of_a_half_held_prefix() {
        let (_rig, editor, host, _tui) = scene("bind-refused-held");
        let ctx = host.own();
        editor.vim.borrow_mut().bind("dx", "nohlsearch");
        // The door that fills the clipboard is the yank itself.
        for key in reovim_plugin_vim::notation::keys("\"+yy").unwrap() {
            editor.vim.borrow_mut().key(tapped(key, TYPED));
        }
        // A clipboard write waiting on an rd the tree does not hold:
        // the step after this one fails, whatever its key.
        editor.clip.set(Some(Rd(u64::MAX)));
        editor.queue.borrow_mut().push_back(tapped(Key::Char('d'), TYPED));
        assert!(editor.step(&ctx).is_err(), "the clipboard write failed");

        // The `d` the prefix was holding went with the queue, so the
        // next key is itself and takes its char.
        editor.queue.borrow_mut().push_back(tapped(Key::Char('x'), TYPED));
        assert_eq!(editor.step(&ctx), Ok(true));
        assert_eq!(editor.text(), "bc\n");
    }

    fn failed_lookup(error: Error) {
        let (_rig, editor, host, mut tui) =
            scene(&format!("buffers-lookup-{error:?}"));
        let ctx = host.own();
        let cwd = editor.cwd.get().unwrap();
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
