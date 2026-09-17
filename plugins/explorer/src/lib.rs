#![no_std]
#![forbid(unsafe_code)]
//! The explorer: a Container listed in a Buffer of its own, and the
//! few keys that walk it; the `Plugin` of clients/core.
//!
//! It knows no filesystem and no Module. A Container is whatever the
//! tree says is one, and its listing is `children`, `info` and the
//! name each child has under it: `../` first where there is a parent,
//! then the Containers with a `/` after the name, then the cells. The
//! listing goes into a scratch Buffer the explorer opens for itself,
//! whole, so a refill is one revision, and the View names that Buffer,
//! which is what the Client lays out.
//!
//! Nothing here opens what it lists. Enter asks the Client to show the
//! entry under the cursor, naming no Plugin, so which Plugin a
//! directory or a file gets is the Client's table's; `-` asks for the
//! parent the same way. The keys are netrw's hand-full, not an engine.

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use reovim_client::{
    BLOCK, Caps, Ctx, Error, Event, Fault, Flags, Handle, Hash, Key, Kind,
    Plugin, Rd, Rights, Start, Tree, grid, keys, open, process,
};
use reovim_text_api::{open as text_open, text};

/// What the prompt says to a line that is no command of the
/// explorer's, as nvim says it.
const E492: &str = "E492: Not an editor command: ";

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
pub struct Explorer {
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
    /// The rows the region has, as `grid` last said.
    rows: usize,
    /// The `:` line without its colon, while one is being typed.
    prompt: Option<String>,
    /// What the status says until the next key.
    message: String,
    /// A `g` seen, waiting for the second one.
    half_g: bool,
    region: Option<Handle>,
    opening: Option<Handle>,
    ending: Option<Handle>,
}

impl Plugin for Explorer {
    /// The Container the region was opened on, listed: a scratch
    /// Buffer of its own, a watch on the Container, the region asked
    /// how big it is, and the listing filled in and shown.
    ///
    /// # Errors
    /// `Unsupported` over a cell or over nothing, the region being
    /// none of the explorer's; otherwise what a call it made says.
    fn start(&mut self, ctx: Ctx<'_>, at: Start) -> Result<(), Fault> {
        let Ctx { tree, caps } = ctx;
        let dir = container(tree, at.over)?;
        self.dir = Some(dir);
        let buffer = scratch(tree)?;
        let rights = Rights::READ.union(Rights::WRITE);
        self.buffer = Some(buffer);
        self.h = Some(tree.open(buffer, rights)?);
        let watched = tree.open(dir, Rights::READ)?;
        tree.watch(watched, true)?;
        self.region = caps.open(grid::hash());
        self.opening = caps.open(open::hash());
        self.ending = caps.open(process::hash());
        if let Some(region) = self.region {
            self.rows = count(grid::Call(caps, region).size()?.rows);
        }
        self.fill(tree)?;
        self.show(caps)
    }

    /// A key walked or typed, or a resize taken for what the View is
    /// laid out over. An input of anything else is not the explorer's.
    ///
    /// # Errors
    /// As [`Self::start`].
    fn input(
        &mut self,
        ctx: Ctx<'_>,
        from: Hash,
        bytes: &[u8],
    ) -> Result<(), Fault> {
        let Ctx { tree, caps } = ctx;
        if from == grid::hash() {
            let resized: grid::Resized = reovim_sdk::decode(bytes)?;
            self.rows = count(resized.rows);
            return self.show(caps);
        }
        if from != keys::hash() {
            return Ok(());
        }
        let press: keys::Press = reovim_sdk::decode(bytes)?;
        let Some(key) = Key::of(press.key) else { return Ok(()) };
        // What the last key had to say stood until this one.
        self.message = String::new();
        self.pressed(tree, caps, key)?;
        self.show(caps)
    }

    /// A child that appeared, went or was renamed under the Container
    /// is a listing that has moved, and so is a ring that overflowed
    /// past the explorer, which names no rd of its own and stands for
    /// whatever was dropped. The Container's own going leaves the last
    /// listing where it is: the tree says so at the next key that asks
    /// it anything.
    ///
    /// # Errors
    /// As [`Self::start`].
    fn changed(&mut self, ctx: Ctx<'_>, events: &[Event]) -> Result<(), Fault> {
        let Ctx { tree, caps } = ctx;
        let Some(dir) = self.dir else { return Ok(()) };
        let moved = |e: &Event| {
            let below =
                matches!(e.kind, Kind::Create | Kind::Delete | Kind::Move);
            e.kind == Kind::Overflow || (below && e.rd == dir)
        };
        if !events.iter().any(moved) {
            return Ok(());
        }
        self.fill(tree)?;
        self.show(caps)
    }
}

impl Explorer {
    /// The Buffer `start` opened; `Closed` before it has.
    fn handle(&self) -> Result<Handle, Fault> {
        self.h.ok_or(Fault::Kernel(Error::Closed))
    }

    /// One key: the prompt's while one is open, the second half of a
    /// `gg`, or the listing's own.
    fn pressed(
        &mut self,
        tree: &mut Tree,
        caps: &mut Caps,
        key: Key,
    ) -> Result<(), Fault> {
        if self.prompt.is_some() {
            return self.typed(caps, key);
        }
        if core::mem::take(&mut self.half_g) {
            self.after_g(key);
            return Ok(());
        }
        match key {
            Key::Char('j') | Key::Down => self.at = self.below(),
            Key::Char('k') | Key::Up => self.at = self.at.saturating_sub(1),
            Key::Char('G') => self.at = self.last(),
            Key::Char('g') => self.half_g = true,
            Key::Char(':') => self.prompt = Some(String::new()),
            Key::Char('-') => return self.parent(tree, caps),
            Key::Enter => return self.entered(caps),
            _ => {}
        }
        Ok(())
    }

    /// The key after a `g`: `gg` is the first entry, and a `g` before
    /// anything else is nothing at all.
    fn after_g(&mut self, key: Key) {
        if key == Key::Char('g') {
            self.at = 0;
        }
    }

    /// One key at the `:` line: it is typed into, dropped, or run.
    /// Backspace over an empty line drops it, as nvim has it.
    fn typed(&mut self, caps: &mut Caps, key: Key) -> Result<(), Fault> {
        let Some(mut line) = self.prompt.take() else { return Ok(()) };
        match key {
            Key::Char(c) => line.push(c),
            Key::Backspace if line.is_empty() => return Ok(()),
            Key::Backspace => {
                line.pop();
            }
            Key::Esc => return Ok(()),
            Key::Enter => return self.ran(caps, &line),
            _ => {}
        }
        self.prompt = Some(line);
        Ok(())
    }

    /// The line at the `:` prompt, run: `q` and `q!` leave with 0, the
    /// listing holding nothing anybody could lose. An empty line is
    /// nothing at all, and anything else is what nvim says to a
    /// command it has not got.
    fn ran(&mut self, caps: &mut Caps, line: &str) -> Result<(), Fault> {
        if line.is_empty() {
            return Ok(());
        }
        if line != "q" && line != "q!" {
            self.message = format!("{E492}{line}");
            return Ok(());
        }
        let Some(ending) = self.ending else { return Ok(()) };
        process::Call(caps, ending).exit(0)
    }

    /// Enter: the entry under the cursor, asked of the Client.
    fn entered(&self, caps: &mut Caps) -> Result<(), Fault> {
        let Some(entry) = self.entries.get(self.at) else { return Ok(()) };
        self.asks(caps, entry.rd)
    }

    /// `-`: the Container above this one, which the tree's root has
    /// not got.
    fn parent(&self, tree: &mut Tree, caps: &mut Caps) -> Result<(), Fault> {
        let Some(dir) = self.dir else { return Ok(()) };
        let Some(parent) = parent_of(tree, dir)? else { return Ok(()) };
        self.asks(caps, parent)
    }

    /// `open` naming no Plugin: which one `target` is shown in is the
    /// Client's table's, and the region is swapped once this call has
    /// returned. A Client with no region to open in shows nothing.
    fn asks(&self, caps: &mut Caps, target: Rd) -> Result<(), Fault> {
        let Some(opening) = self.opening else { return Ok(()) };
        let ask = open::Ask { target: target.0, plugin: String::new() };
        open::Call(caps, opening).open(ask)
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

    /// The listing read again and written whole, so that a refill is
    /// one revision. The selection stays on the name it was on while
    /// that name is still there, and is clamped where it is not.
    fn fill(&mut self, tree: &mut Tree) -> Result<(), Fault> {
        let Some(dir) = self.dir else { return Ok(()) };
        let was = self.entries.get(self.at).map(|entry| entry.name.clone());
        self.entries = listing(tree, dir)?;
        self.at = kept(&self.entries, was.as_deref(), self.at);
        let text = shown(&self.entries);
        let len = core::mem::replace(&mut self.len, text.len() as u64);
        let edit = text::Edit { at: 0, len, text };
        text::Call(tree, self.handle()?).replace(edit)
    }

    /// The View to the region, if the Client has one.
    fn show(&mut self, caps: &mut Caps) -> Result<(), Fault> {
        let Some(region) = self.region else { return Ok(()) };
        let view = self.view();
        grid::Call(caps, region).show(view)
    }

    /// What to show: the cursor on the first byte of the selected
    /// line, the first line shown moved to keep it in the rows the
    /// region has, and the `:` line or the last message as the status.
    fn view(&mut self) -> grid::View {
        let height = self.rows.max(1);
        if self.at < self.top {
            self.top = self.at;
        } else if self.at >= self.top + height {
            self.top = self.at + 1 - height;
        }
        grid::View {
            over: self.buffer.map_or(0, |buffer| buffer.0),
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
fn container(tree: &mut Tree, over: Option<Rd>) -> Result<Rd, Fault> {
    let refused = || Fault::Kernel(Error::Unsupported);
    let Some(rd) = over else { return Err(refused()) };
    if tree.info(rd)?.flags.contains(Flags::CONTAINER) {
        return Ok(rd);
    }
    Err(refused())
}

/// A Buffer of the explorer's own, over nothing: what the listing is
/// written into and what its View names.
fn scratch(tree: &mut Tree) -> Result<Rd, Fault> {
    let opener = tree.resolve("/module/text/open")?;
    let opening = tree.open(opener, Rights::WRITE)?;
    let buffer = text_open::Call(tree, opening).new();
    tree.close(opening)?;
    buffer
}

/// What the Container holds: `../` first where there is a parent, the
/// Containers below it, then the cells, each group by name as bytes,
/// which is the order `str` compares in.
fn listing(tree: &mut Tree, dir: Rd) -> Result<Vec<Entry>, Fault> {
    let mut entries = Vec::new();
    if let Some(parent) = parent_of(tree, dir)? {
        let name = String::from("..");
        entries.push(Entry { rd: parent, name, container: true });
    }
    let mut below = Vec::new();
    for rd in tree.children(dir)? {
        below.push(entry(tree, rd)?);
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
fn entry(tree: &mut Tree, rd: Rd) -> Result<Entry, Fault> {
    let info = tree.info(rd)?;
    let container = info.flags.contains(Flags::CONTAINER);
    Ok(Entry { rd, name: info.name.unwrap_or_default(), container })
}

/// The Container above `dir`, or nothing at the tree's root, which has
/// no parent to go to.
fn parent_of(tree: &mut Tree, dir: Rd) -> Result<Option<Rd>, Fault> {
    match tree.parent(dir) {
        Err(Fault::Kernel(Error::Invalid)) => Ok(None),
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

/// A count the Client gave, as far as this machine counts.
fn count(n: u64) -> usize {
    usize::try_from(n).unwrap_or(usize::MAX)
}
