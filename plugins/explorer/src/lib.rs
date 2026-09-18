#![no_std]
#![forbid(unsafe_code)]
//! The explorer: a Container listed in a Buffer of its own, and the
//! few keys that walk it; a Module a Client loads.
//!
//! It knows no filesystem and no Module. A Container is whatever the
//! tree says is one, and its listing is `children`, `info` and the
//! name each child has under it: `../` first where there is a parent,
//! then the Containers with a `/` after the name, then the cells. The
//! listing goes into a scratch draft of `/self/dev/buffers`, whole, so
//! a refill is one write and no wire message, and the View names that
//! draft, which is what the Client lays out.
//!
//! Nothing here opens what it lists. Enter asks the Client to show the
//! entry under the cursor, naming no Plugin, so which Plugin a
//! directory or a file gets is the Client's table's; `-` asks for the
//! parent the same way. The keys are netrw's hand-full, not an engine.
//!
//! One cell is served, `view`: a whole read of it is one frame, and
//! the frame is where the work happens. The keys it has not taken, what
//! changed under the Container since its last frame, and the View.

extern crate alloc;

use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::{OnceCell, RefCell};

use reovim_client_api::{
    BLOCK, Frames, Key, NONE, Region, count, drafted, window,
};
use reovim_kernel_interface::{
    Booted, Calls, Driver, DriverId, Error, Event, Flags, Handle, Kind, Module,
    Rd, Reply, Rights,
};
use reovim_sdk::{encode, ops};
use reovim_text_api::text;

/// What the prompt says to a line that is no command of the
/// explorer's, as nvim says it.
const E492: &str = "E492: Not an editor command: ";

/// The explorer, ready for a Client to load.
#[must_use]
pub fn boot() -> Booted {
    let explorer = Rc::new(Loaded::default());
    Booted { module: explorer.clone(), drivers: alloc::vec![explorer] }
}

/// One line of the listing: what it is called, what Enter opens, and
/// whether it is itself a Container.
struct Entry {
    /// What Enter asks the Client to show: the child, or the parent
    /// for `../`.
    rd: Rd,
    /// The name under the Container; `..` for the parent.
    name: String,
    /// A Container is shown with a `/` after its name.
    container: bool,
}

impl Entry {
    /// The line the listing shows for it.
    fn line(&self) -> String {
        let slash = if self.container { "/" } else { "" };
        format!("{}{slash}", self.name)
    }

    /// What that line takes, its newline apart.
    fn bytes(&self) -> usize {
        self.name.len() + usize::from(self.container)
    }
}

/// A Container listed in a Buffer of the explorer's own.
///
/// `start` takes the rd the region was opened on, which is a Container
/// or the region is none of the explorer's; everything after it is the
/// selection, the rows the region has, and the `:` line while one is
/// being typed.
#[derive(Default)]
struct Explorer {
    /// The Container listed, and watched for what appears under it.
    dir: Option<Rd>,
    /// The scratch Buffer the listing is in, and the handle on it.
    buffer: Option<Rd>,
    h: Option<Handle>,
    /// What that Buffer holds, in bytes: what the next refill replaces.
    len: u64,
    entries: Vec<Entry>,
    /// The selected entry, and the first one shown.
    at: usize,
    top: usize,
    /// The rows the region has, as `size` last said.
    rows: usize,
    /// The `:` line without its colon, while one is being typed.
    prompt: Option<String>,
    /// What the status says until the next key.
    message: String,
    /// A `g` seen, waiting for the second one.
    half_g: bool,
    /// Where it stands in the one stream of events.
    cursor: u64,
}

/// The explorer's state behind one `RefCell`: no borrow of it outlives
/// an op, so a Driver that re-enters finds it free.
#[derive(Default)]
struct Loaded {
    state: RefCell<Explorer>,
    /// The cells of `/self/dev`, found once at `start`: they never
    /// change, so no borrow of the state is held over an op.
    region: OnceCell<Region>,
    view: Frames,
}

impl Module for Loaded {
    /// The Container the region was opened on, listed: a scratch
    /// Buffer of its own, a watch on the Container, and the one cell a
    /// Plugin serves, `view`.
    fn start(&self, ctx: &dyn Calls, home: Rd) -> Result<(), Error> {
        let view = ctx.register(DriverId(0), home, Some("view"))?;
        ctx.report(view, Flags::READ, &[])?;
        let region = Region::of(ctx)?;
        let dir = container(ctx, region.ask(ctx)?.target)?;
        let buffer = drafted(ctx, None)?;
        let rights = Rights::READ.union(Rights::WRITE);
        let h = ctx.open(buffer, rights)?;
        ops::watch(ctx, dir)?;
        let mut it = self.state.borrow_mut();
        it.dir = Some(dir);
        it.buffer = Some(buffer);
        it.h = Some(h);
        drop(it);
        let _ = self.region.set(region);
        self.sized(ctx)?;
        self.fill(ctx)?;
        // Where the stream stands once the first listing is in: a
        // cursor of 0 against a ring that never reached it is one
        // Overflow, and what the listing itself registered under the
        // Container is already in it.
        let (_, cursor) = ops::since(ctx, 0)?;
        self.state.borrow_mut().cursor = cursor;
        Ok(())
    }
}

impl Driver for Loaded {
    fn open(&self, _: &dyn Calls, rd: Rd, _: Rights) -> Reply<u64> {
        Reply::ok(rd.0)
    }

    fn read(
        &self,
        ctx: &dyn Calls,
        _: u64,
        at: u64,
        len: usize,
    ) -> Reply<Vec<u8>> {
        self.view.read(at, len, || self.framed(ctx))
    }
}

impl Loaded {
    /// One frame: every key it has not taken, what appeared or went
    /// under the Container since the last one, the rows it has now,
    /// and the View.
    fn framed(&self, ctx: &dyn Calls) -> Result<Vec<u8>, Error> {
        let keys = self.region()?.keys(ctx)?;
        for key in keys {
            // What the last key had to say stood until this one.
            self.state.borrow_mut().message = String::new();
            self.pressed(ctx, key)?;
        }
        self.changed(ctx)?;
        self.sized(ctx)?;
        Ok(encode(&self.state.borrow_mut().view()))
    }

    /// A child that appeared, went or was renamed under the Container
    /// is a listing that has moved, and so is a ring that overflowed
    /// past the explorer, which names no rd of its own and stands for
    /// whatever was dropped. The Container's own going leaves the last
    /// listing where it is: the tree says so at the next key that asks
    /// it anything.
    fn changed(&self, ctx: &dyn Calls) -> Result<(), Error> {
        let cursor = self.state.borrow().cursor;
        let (events, next) = ops::since(ctx, cursor)?;
        if !self.state.borrow_mut().moved(next, &events) {
            return Ok(());
        }
        self.fill(ctx)
    }

    /// The rows the region has now, as `windows/0/size` says.
    fn sized(&self, ctx: &dyn Calls) -> Result<(), Error> {
        let rows = self.region()?.size(ctx)?.rows;
        self.state.borrow_mut().rows = count(rows);
        Ok(())
    }

    /// The cells of `/self/dev` this Plugin holds; `Closed` before
    /// `start` has found them.
    fn region(&self) -> Result<&Region, Error> {
        self.region.get().ok_or(Error::Closed)
    }

    /// One key: the prompt's while one is open, the second half of a
    /// `gg`, or the listing's own.
    fn pressed(&self, ctx: &dyn Calls, key: Key) -> Result<(), Error> {
        let mut it = self.state.borrow_mut();
        if it.prompt.is_some() {
            let line = it.typed(key);
            drop(it);
            return line.map_or(Ok(()), |line| self.ran(ctx, &line));
        }
        if core::mem::take(&mut it.half_g) {
            it.after_g(key);
            return Ok(());
        }
        match key {
            Key::Char('j') | Key::Down => it.at = it.below(),
            Key::Char('k') | Key::Up => it.at = it.at.saturating_sub(1),
            Key::Char('G') => it.at = it.last(),
            Key::Char('g') => it.half_g = true,
            Key::Char(':') => it.prompt = Some(String::new()),
            Key::Char('-') => {
                let dir = it.dir;
                drop(it);
                return self.parent(ctx, dir);
            }
            Key::Enter => {
                let entry = it.entries.get(it.at).map(|entry| entry.rd);
                drop(it);
                return entry.map_or(Ok(()), |rd| self.asks(ctx, rd));
            }
            _ => {}
        }
        Ok(())
    }

    /// The line at the `:` prompt, run: `q` and `q!` leave with 0, the
    /// listing holding nothing anybody could lose. An empty line is
    /// nothing at all, and anything else is what nvim says to a
    /// command it has not got.
    fn ran(&self, ctx: &dyn Calls, line: &str) -> Result<(), Error> {
        if line.is_empty() {
            return Ok(());
        }
        if line != "q" && line != "q!" {
            self.state.borrow_mut().message = format!("{E492}{line}");
            return Ok(());
        }
        self.region()?.ends(ctx, 0)
    }

    /// `-`: the Container above this one, which the tree's root has
    /// not got.
    fn parent(&self, ctx: &dyn Calls, dir: Option<Rd>) -> Result<(), Error> {
        let Some(dir) = dir else { return Ok(()) };
        let Some(parent) = parent_of(ctx, dir)? else { return Ok(()) };
        self.asks(ctx, parent)
    }

    /// `open` naming no Plugin: which one `target` is shown in is the
    /// Client's table's, and the region is swapped at the next frame.
    fn asks(&self, ctx: &dyn Calls, target: Rd) -> Result<(), Error> {
        self.region()?.opens(ctx, target, "")
    }

    /// The listing read again and written whole, so that a refill is
    /// one revision. The selection stays on the name it was on while
    /// that name is still there, and is clamped where it is not.
    fn fill(&self, mut ctx: &dyn Calls) -> Result<(), Error> {
        let Some(dir) = self.state.borrow().dir else { return Ok(()) };
        let was = {
            let it = self.state.borrow();
            it.entries.get(it.at).map(|entry| entry.name.clone())
        };
        let entries = listing(ctx, dir)?;
        let (h, edit) = {
            let mut it = self.state.borrow_mut();
            it.entries = entries;
            it.at = kept(&it.entries, was.as_deref(), it.at);
            let text = shown(&it.entries);
            let len = core::mem::replace(&mut it.len, text.len() as u64);
            (it.h.ok_or(Error::Closed)?, text::Edit { at: 0, len, text })
        };
        text::Call(&mut ctx, h).replace(edit)
    }
}

impl Explorer {
    /// Whether the events say the listing moved, the cursor taken on
    /// either way.
    fn moved(&mut self, next: u64, events: &[Event]) -> bool {
        self.cursor = next;
        let Some(dir) = self.dir else { return false };
        let moved = |e: &Event| {
            let below =
                matches!(e.kind, Kind::Create | Kind::Delete | Kind::Move);
            e.kind == Kind::Overflow || (below && e.rd == dir)
        };
        events.iter().any(moved)
    }

    /// The key after a `g`: `gg` is the first entry, and a `g` before
    /// anything else is nothing at all.
    fn after_g(&mut self, key: Key) {
        if key == Key::Char('g') {
            self.at = 0;
        }
    }

    /// One key at the `:` line: it is typed into, dropped, or run,
    /// which gives the line back. Backspace over an empty line drops
    /// it, as nvim has it.
    fn typed(&mut self, key: Key) -> Option<String> {
        let mut line = self.prompt.take()?;
        match key {
            Key::Char(c) => line.push(c),
            Key::Backspace if line.is_empty() => return None,
            Key::Backspace => {
                line.pop();
            }
            Key::Esc => return None,
            Key::Enter => return Some(line),
            _ => {}
        }
        self.prompt = Some(line);
        None
    }

    /// The entry below the selected one; the selection stops at the
    /// last, there being nothing under it.
    fn below(&self) -> usize {
        (self.at + 1).min(self.last())
    }

    /// The last entry there is, and the first for a listing of none.
    const fn last(&self) -> usize {
        self.entries.len().saturating_sub(1)
    }

    /// What to show: the cursor on the first byte of the selected
    /// line, the first line shown moved to keep it in the rows the
    /// region has, and the `:` line or the last message as the status.
    fn view(&mut self) -> window::View {
        let height = self.rows.max(1);
        if self.at < self.top {
            self.top = self.at;
        } else if self.at >= self.top + height {
            self.top = self.at + 1 - height;
        }
        window::View {
            over: self.buffer.unwrap_or(NONE),
            top: byte_at(&self.entries, self.top),
            cursor: byte_at(&self.entries, self.at),
            shape: BLOCK,
            spans: Vec::new(),
            status: self.status(),
        }
    }

    /// The status word: the `:` line as it was typed while one is
    /// open, else what the last key had to say.
    fn status(&self) -> String {
        let said = || self.message.clone();
        self.prompt.as_ref().map_or_else(said, |line| format!(":{line}"))
    }
}

/// The Container the region was opened on. A region over a cell or
/// over nothing is not the explorer's to show.
fn container(ctx: &dyn Calls, over: Rd) -> Result<Rd, Error> {
    if over != NONE && ops::info(ctx, over)?.flags.contains(Flags::CONTAINER) {
        return Ok(over);
    }
    Err(Error::Unsupported)
}

/// What the Container holds: `../` first where there is a parent, the
/// Containers below it, then the cells, each group by name as bytes,
/// which is the order `str` compares in.
fn listing(ctx: &dyn Calls, dir: Rd) -> Result<Vec<Entry>, Error> {
    let mut entries = Vec::new();
    if let Some(parent) = parent_of(ctx, dir)? {
        let name = String::from("..");
        entries.push(Entry { rd: parent, name, container: true });
    }
    let mut below = Vec::new();
    for rd in ops::children(ctx, dir)? {
        below.push(entry(ctx, rd)?);
    }
    let (mut dirs, mut cells): (Vec<Entry>, Vec<Entry>) =
        below.into_iter().partition(|entry| entry.container);
    dirs.sort_by(|a, b| a.name.cmp(&b.name));
    cells.sort_by(|a, b| a.name.cmp(&b.name));
    entries.append(&mut dirs);
    entries.append(&mut cells);
    Ok(entries)
}

/// One child as the listing has it.
fn entry(ctx: &dyn Calls, rd: Rd) -> Result<Entry, Error> {
    let info = ops::info(ctx, rd)?;
    let container = info.flags.contains(Flags::CONTAINER);
    Ok(Entry { rd, name: info.name.unwrap_or_default(), container })
}

/// The Container above `dir`, or nothing at the tree's root, which has
/// no parent to go to.
fn parent_of(ctx: &dyn Calls, dir: Rd) -> Result<Option<Rd>, Error> {
    match ops::parent(ctx, dir) {
        Err(Error::Invalid) => Ok(None),
        found => found.map(Some),
    }
}

/// The listing as the Buffer holds it: one entry per line.
fn shown(entries: &[Entry]) -> String {
    let mut text = String::new();
    for entry in entries {
        text.push_str(&entry.line());
        text.push('\n');
    }
    text
}

/// The first byte of the line entry `n` is on.
fn byte_at(entries: &[Entry], n: usize) -> u64 {
    let bytes: usize = entries.iter().take(n).map(|e| e.bytes() + 1).sum();
    bytes as u64
}

/// Where the selection lands after a refill: on the name it was on
/// while that name is still there, else where it was, clamped to the
/// last entry.
fn kept(entries: &[Entry], was: Option<&str>, at: usize) -> usize {
    let same = |name: &str| entries.iter().position(|e| e.name == name);
    let clamped = || at.min(entries.len().saturating_sub(1));
    was.and_then(same).unwrap_or_else(clamped)
}
