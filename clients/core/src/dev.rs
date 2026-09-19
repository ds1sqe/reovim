//! `/self/dev`, the Client's own devices as cells.
//!
//! One Module, two Drivers, one `RefCell` each. What a Client used to
//! answer in Rust is a tree here: where it stands, what was typed, how
//! big the region is, what is shown in it, and the code it leaves with.
//!
//! ```text
//! /self/dev/
//!   buffers          call: the Client's own Buffers, crate::buffers
//!   clipboard        read/write: the machine's clipboard, whole
//!   cwd              read: where the Client stands, text
//!   display/0/info/rows  read/write: the terminal's own rows, decimal
//!   display/0/info/cols  read/write: the terminal's own cols, decimal
//!   display/0/text   read/write: the Screen the Client last drew
//!   exit             read/write: the code in decimal; empty for none
//!   input/keyboard   read/write: the stream of presses
//!   link             read: what the wire has carried, labelled text
//!   open             call: open(Ask), back()
//!   windows/0/ask    read: the Ask this region was opened with
//!   windows/0/size   read/write: the Size record
//!   windows/0/view   read: the view of the Plugin holding the region
//! ```
//!
//! The Plugin holding the region is a Module the Client loaded under
//! its own name at `/self`, and a read of `windows/0/view` is the read
//! of that Plugin's own `view`, forwarded. An `open` swaps them: the
//! ask is recorded where it is made and acted on at the head of the
//! next read of the view, which is after the Plugin that asked has
//! returned.

use alloc::boxed::Box;
use alloc::collections::{BTreeMap, VecDeque};
use alloc::format;
use alloc::rc::{Rc, Weak};
use alloc::string::{String, ToString};
use alloc::vec::Vec;
use core::cell::RefCell;

use reovim_client_api::{NONE, keyboard, open, part, window};
use reovim_kernel_interface::{
    Booted, Calls, Decl, Driver, DriverId, Error, Flags, Handle, Hash, Kind,
    Module, Rd, Reply, Rights,
};
use reovim_sdk::{cell, decode, encode, ops};

use crate::buffers::Buffers;
use crate::client::ROOT;
use crate::fault::word;
use crate::host::{Host, SELF};

/// Presses the stream keeps: a burst nobody reads cannot grow it, and
/// no burst a person can type is lost.
const RING: usize = 1024;

/// The size the region is taken for until something writes one.
const FIRST: window::Size = window::Size { cols: 80, rows: 23 };

/// The bytes of one key code, which a write of the keyboard is a run
/// of.
const KEY: usize = 8;

/// The level an entry `dev` writes to `/sys/log` goes in at.
const WARN: &str = "<4>";

/// Where the fs Module mirrors the directory the Client runs over.
const MIRROR: &str = "/module/fs/root";

/// What a Client asks its table for.
///
/// A name is the one a Plugin gave `open`; a shape is what the rd to
/// be shown is, a Container or a cell, for an `open` that named no
/// Plugin.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Want<'a> {
    Named(&'a str),
    Container,
    Cell,
}

/// How the region's Plugin is made, which is the app's to say: the name
/// it is loaded under at `/self`, the Module and its Drivers. A Want
/// the app has no Plugin for is `None`.
pub type Table = Box<dyn Fn(Want<'_>) -> Option<(String, Booted)>>;

/// What reads the machine's clipboard.
pub type Read = Box<dyn Fn() -> Result<Vec<u8>, Error>>;

/// What writes it.
pub type Write = Box<dyn Fn(&[u8]) -> Result<(), Error>>;

/// The clipboard of the machine the person sits at, as the pair of
/// closures the app injects.
///
/// The Client links no host crate, so the tool is reached this way and
/// its failures arrive as the Kernel's: no tool is `Unsupported`, a
/// tool that failed is `Busy`.
pub struct Clip {
    read: Read,
    write: Write,
}

impl Clip {
    /// One over `read` and `write`.
    #[must_use]
    pub const fn new(read: Read, write: Write) -> Self {
        Self { read, write }
    }

    /// A Client that was given no machine: both `Unsupported`, which
    /// is what a host with no tool on PATH answers.
    #[must_use]
    pub fn none() -> Self {
        Self::new(
            Box::new(|| Err(Error::Unsupported)),
            Box::new(|_| Err(Error::Unsupported)),
        )
    }
}

/// Which cell of `display/0` an rd is: `rows` and `cols` its own size
/// (the whole terminal, not the region's) and `text` the frame it
/// shows.
#[derive(Clone, Copy, Eq, PartialEq)]
enum Part {
    Rows,
    Cols,
    Text,
}

impl Part {
    /// Where it stands in the display's own bytes, one slot each.
    const fn slot(self) -> usize {
        match self {
            Self::Rows => 0,
            Self::Cols => 1,
            Self::Text => 2,
        }
    }
}

/// Which cell of `dev` an rd is.
#[derive(Clone, Copy, Eq, PartialEq)]
enum Held {
    Clipboard,
    Cwd,
    Display(Part),
    Exit,
    Keyboard,
    Link,
    Open,
    Ask,
    Size,
    View,
    /// `input`, `windows`, `display` and `display/0`, which hold the
    /// rest.
    Dir,
}

/// One handle `dev` gave out: what it is on, and, for the keyboard, how
/// far down the stream it has read.
struct Slot {
    rd: Rd,
    at: u64,
    /// For a cell made at the read, what this handle read at 0, which
    /// every read past it is served from; empty for the rest.
    snap: Vec<u8>,
}

/// The stream of presses, and how many have fallen off its front.
struct Stream {
    held: VecDeque<keyboard::Press>,
    first: u64,
}

impl Stream {
    /// The place a handle opened now starts at, which is the end: keys
    /// typed at another Plugin are not replayed.
    fn end(&self) -> u64 {
        self.first + self.held.len() as u64
    }

    /// One press appended, the oldest dropped past [`RING`].
    fn push(&mut self, press: keyboard::Press) {
        self.held.push_back(press);
        while self.held.len() > RING {
            self.held.pop_front();
            self.first += 1;
        }
    }

    /// The press at `at`, or at the oldest still held when the stream
    /// has moved past it; `None` at the end.
    fn at(&self, at: u64) -> Option<(u64, keyboard::Press)> {
        let at = at.max(self.first);
        let index = usize::try_from(at - self.first).ok()?;
        self.held.get(index).map(|press| (at, press.clone()))
    }
}

/// The Plugin holding the region: the name it is loaded under, and the
/// handle on the `view` it serves.
struct Region {
    name: String,
    view: Handle,
}

/// Everything `dev` holds.
struct State {
    home: Option<Rd>,
    cells: BTreeMap<Rd, Held>,
    handles: BTreeMap<u64, Slot>,
    next: u64,
    /// The rd of this Client at the server, which stamps a press.
    client: Rd,
    dir: String,
    size: window::Size,
    /// `display/0`'s own bytes, one slot a [`Part`]: the frame the
    /// Client last drew and the two facts of its own size, decimal
    /// text; empty until the drawer has written that part, a display
    /// holding only the facts it has.
    display: [Vec<u8>; 3],
    /// What the region was opened with, as `windows/0/ask` holds it.
    ask: open::Ask,
    exit: Option<u8>,
    /// What `open` asked for, until the next read of the view.
    asked: Option<open::Ask>,
    /// Whether `back` was asked, likewise.
    back: bool,
    /// What the region was opened with before the last swap, which
    /// `back` goes to: one step, not a stack.
    before: Option<open::Ask>,
    region: Option<Region>,
    keys: Stream,
}

impl State {
    /// The ask the region takes now, the Plugin that held it let go: a
    /// swap forward leaves the step `back` takes, and the step back
    /// takes none of its own.
    fn asking(&mut self, ask: open::Ask, step: bool) -> Option<Region> {
        let old = self.region.take();
        if step && old.is_some() {
            self.before = Some(self.ask.clone());
        }
        self.ask = ask;
        old
    }

    /// `display/0`'s bytes of `part`.
    fn shown(&self, part: Part) -> Vec<u8> {
        self.display[part.slot()].clone()
    }

    /// `display/0`'s bytes of `part`, set whole.
    fn set(&mut self, part: Part, bytes: Vec<u8>) {
        self.display[part.slot()] = bytes;
    }
}

/// The Client's own Driver, and its Module.
pub struct Dev {
    /// The Host that loaded it: a swap loads and unloads a Plugin
    /// there, which `Calls` cannot spell.
    host: Weak<Host>,
    table: Table,
    clip: Clip,
    /// The second Driver, which holds the drafts and nothing else.
    buffers: Rc<Buffers>,
    state: RefCell<State>,
}

/// `/self/dev` loaded at `host`, over the region `file` names: its rd.
///
/// `clip` is the machine's clipboard, [`Clip::none`] for a Client that
/// was given none. `dir` is where the Client stands, `file` a name
/// under the root or nothing, and `line` the line to begin at, counted
/// from 1. The first Plugin comes of the ask this leaves in
/// `windows/0/ask`, at the first read of the view.
///
/// # Errors
/// What `load` says, and `Invalid` if `dev` did not register.
pub fn boot(
    host: &Rc<Host>,
    table: Table,
    clip: Clip,
    dir: &str,
    file: Option<&str>,
    line: Option<u64>,
) -> Result<Rd, Error> {
    let dev = Rc::new(Dev::new(host, table, clip, dir, file, line));
    let drivers: Vec<Rc<dyn Driver>> =
        alloc::vec![dev.clone(), dev.buffers.clone()];
    host.load("dev", dev.clone(), drivers)?;
    // A Client with no devices reads no key and draws no frame, so the
    // name is nobody's to unload.
    host.keeps("dev");
    dev.state.borrow().home.ok_or(Error::Invalid)
}

impl Dev {
    fn new(
        host: &Rc<Host>,
        table: Table,
        clip: Clip,
        dir: &str,
        file: Option<&str>,
        line: Option<u64>,
    ) -> Self {
        // The ask `start` fills in once it knows what the file is.
        let ask = open::Ask {
            target: NONE,
            plugin: String::new(),
            name: file.map(ToString::to_string).unwrap_or_default(),
            line: line.unwrap_or(0),
        };
        let state = State {
            home: None,
            cells: BTreeMap::new(),
            handles: BTreeMap::new(),
            next: 1,
            client: Rd(0),
            dir: String::from(dir),
            size: FIRST,
            display: [Vec::new(), Vec::new(), Vec::new()],
            ask,
            exit: None,
            asked: None,
            back: false,
            before: None,
            region: None,
            keys: Stream { held: VecDeque::new(), first: 0 },
        };
        Self {
            host: Rc::downgrade(host),
            table,
            clip,
            buffers: Rc::new(Buffers::new()),
            state: RefCell::new(state),
        }
    }

    /// One cell under `parent`, kept by what it is.
    fn add(
        &self,
        ctx: &dyn Calls,
        parent: Rd,
        name: &str,
        (flags, apis): (Flags, &[Decl]),
        held: Held,
    ) -> Result<Rd, Error> {
        let rd = ctx.register(DriverId(0), parent, Some(name))?;
        ctx.report(rd, flags, apis)?;
        self.state.borrow_mut().cells.insert(rd, held);
        Ok(rd)
    }

    /// What `h` is open on, and where it stands.
    fn slot(&self, h: u64) -> Result<(Held, Rd, u64), Error> {
        let s = self.state.borrow();
        let slot = s.handles.get(&h).ok_or(Error::Invalid)?;
        let held = *s.cells.get(&slot.rd).ok_or(Error::Gone)?;
        Ok((held, slot.rd, slot.at))
    }

    /// The bytes a cell that holds a record or a line of text answers.
    fn kept(&self, held: Held) -> Result<Vec<u8>, Error> {
        let s = self.state.borrow();
        Ok(match held {
            Held::Cwd => s.dir.clone().into_bytes(),
            Held::Exit => s
                .exit
                .map_or_else(Vec::new, |code| format!("{code}").into_bytes()),
            Held::Ask => encode(&s.ask),
            Held::Size => encode(&s.size),
            _ => return Err(Error::Unsupported),
        })
    }

    /// One press, taken for this handle alone: what it has not taken,
    /// and `Busy` where there is none, the stream not being over.
    fn pressed(&self, h: u64, at: u64, len: usize) -> Result<Vec<u8>, Error> {
        let mut s = self.state.borrow_mut();
        let Some((at, press)) = s.keys.at(at) else { return Err(Error::Busy) };
        let bytes = encode(&press);
        // A press is taken whole or not at all: half a record is no
        // record, so a read that cannot carry one takes nothing.
        if len < bytes.len() {
            return Err(Error::Invalid);
        }
        if let Some(slot) = s.handles.get_mut(&h) {
            slot.at = at + 1;
        }
        Ok(bytes)
    }

    /// The view of the Plugin holding the region, forwarded.
    ///
    /// A whole read is one frame, and the swap an `open` asked for
    /// happens on either side of it: what was asked before this read
    /// at its head, and what the frame itself asked once the Plugin
    /// that asked has returned, so a key that gives the region away is
    /// answered by the Plugin it was given to. One ask per read: a
    /// Plugin that asks in its own first frame is served at the next.
    ///
    /// A region no Plugin holds reads empty and the Client draws what
    /// it drew before.
    fn viewed(
        &self,
        ctx: &dyn Calls,
        at: u64,
        len: u32,
    ) -> Result<Vec<u8>, Error> {
        if at > 0 {
            return self.forwarded(ctx, at, len);
        }
        self.swapped(ctx)?;
        let bytes = self.forwarded(ctx, 0, len)?;
        if !self.pending() {
            return Ok(bytes);
        }
        self.swapped(ctx)?;
        self.forwarded(ctx, 0, len)
    }

    /// Whether the frame just read left an ask of its own.
    fn pending(&self) -> bool {
        let s = self.state.borrow();
        s.asked.is_some() || s.back
    }

    /// The read passed on to the Plugin holding the region.
    fn forwarded(
        &self,
        ctx: &dyn Calls,
        at: u64,
        len: u32,
    ) -> Result<Vec<u8>, Error> {
        let view =
            self.state.borrow().region.as_ref().map(|region| region.view);
        let Some(view) = view else { return Ok(Vec::new()) };
        ops::read(ctx, view, at, len)
    }

    /// What `open` or `back` left, acted on: one ask per step, and the
    /// last one stands.
    fn swapped(&self, ctx: &dyn Calls) -> Result<(), Error> {
        let (asked, back) = {
            let mut s = self.state.borrow_mut();
            (s.asked.take(), core::mem::replace(&mut s.back, false))
        };
        if back {
            return self.stepped_back(ctx);
        }
        let Some(ask) = asked else { return Ok(()) };
        self.shows(ctx, &ask, true)
    }

    /// `back`: what the region was opened with before the last swap,
    /// shown again. Nowhere to go is the Plugin that asked having been
    /// the first thing shown, and the Client leaves as an exit of 0
    /// would end it.
    fn stepped_back(&self, ctx: &dyn Calls) -> Result<(), Error> {
        let before = self.state.borrow_mut().before.take();
        let Some(ask) = before else {
            self.state.borrow_mut().exit = Some(0);
            return Ok(());
        };
        self.shows(ctx, &ask, false)
    }

    /// The region given to the Plugin `ask` asks for, and what stands
    /// there if it cannot be given: the ask the region stood on, shown
    /// again, and a line in the log, as an ask no Plugin serves has.
    ///
    /// A name is one Module's, so the old Plugin goes before the new
    /// one comes and a swap that fails past that point has no old
    /// region left to keep. A region no Plugin holds takes no key and
    /// answers no frame, so leaving one empty is the one thing a
    /// failure here may not do; a swap that failed before the old
    /// Plugin let go has left everything where it was.
    ///
    /// `step` is a swap forward, which `back` then has a step to take;
    /// the step back takes none of its own.
    fn shows(
        &self,
        ctx: &dyn Calls,
        ask: &open::Ask,
        step: bool,
    ) -> Result<(), Error> {
        let stood = {
            let s = self.state.borrow();
            (s.ask.clone(), s.before.clone())
        };
        let Err(error) = self.given(ctx, ask, step) else { return Ok(()) };
        if self.state.borrow().region.is_some() {
            return Err(error);
        }
        // The region comes back before anything else is done about it,
        // and the step `back` would have taken with it.
        let _ = self.given(ctx, &stood.0, false);
        self.state.borrow_mut().before = stood.1;
        warned(ctx, ask, error)?;
        if self.state.borrow().region.is_none() {
            return Err(error);
        }
        Ok(())
    }

    /// The swap itself: the old Plugin unloaded, which closes what it
    /// held, and the new one loaded under its own name with the ask
    /// already in `windows/0/ask`, where it reads it at `start`.
    ///
    /// Nothing half-loaded is left behind: a Plugin that will not
    /// start is not loaded, and one that serves no view goes back out
    /// with its name.
    fn given(
        &self,
        ctx: &dyn Calls,
        ask: &open::Ask,
        step: bool,
    ) -> Result<(), Error> {
        let Some((name, booted)) = self.served(ctx, ask)? else {
            return unserved(ctx, ask);
        };
        let host = self.host.upgrade().ok_or(Error::Gone)?;
        let old = self.state.borrow_mut().asking(ask.clone(), step);
        if let Some(old) = old {
            // The region is this Module's to give away, so the Plugin
            // holding it is unloaded here and by nothing else.
            host.shed(&old.name)?;
        }
        host.load(&name, booted.module, booted.drivers)?;
        let view = match opened(ctx, &name) {
            Ok(view) => view,
            Err(error) => {
                let _ = host.shed(&name);
                return Err(error);
            }
        };
        host.keeps(&name);
        self.state.borrow_mut().region = Some(Region { name, view });
        Ok(())
    }

    /// What the table gives for `ask`: the Plugin its name stands for,
    /// or the one the target's shape stands for.
    fn served(
        &self,
        ctx: &dyn Calls,
        ask: &open::Ask,
    ) -> Result<Option<(String, Booted)>, Error> {
        if ask.plugin.is_empty() {
            return Ok((self.table)(want_of(ctx, ask.target)?));
        }
        Ok((self.table)(Want::Named(&ask.plugin)))
    }

    /// The key codes of one write appended to the stream, stamped
    /// here: `when` is the Host's clock and `source` the rd of whoever
    /// wrote it, neither of them the writer's to say. A write is any
    /// number of codes, so a line typed from another Client is one op.
    fn typed(&self, buf: &[u8]) -> Result<usize, Error> {
        // Whole codes or none: half a key is no key, so a write that
        // is no run of them leaves the stream where it was.
        let (codes, rest) = buf.as_chunks::<KEY>();
        if !rest.is_empty() {
            return Err(Error::Invalid);
        }
        let host = self.host.upgrade().ok_or(Error::Gone)?;
        let when = host.now();
        let mut s = self.state.borrow_mut();
        let source = s.client;
        for code in codes {
            let key = u64::from_le_bytes(*code);
            s.keys.push(keyboard::Press { key, when, source });
        }
        Ok(buf.len())
    }

    /// One answer per handle: the read at 0 makes what the cell holds
    /// and keeps it for this handle, and every read past it is served
    /// from that. So a whole read is one frame however many chunks it
    /// takes and whatever was drawn between them, and one set of the
    /// wire's numbers however far they moved while it was read.
    fn snapped(
        &self,
        h: u64,
        at: u64,
        len: usize,
        made: impl FnOnce() -> Result<Vec<u8>, Error>,
    ) -> Result<Vec<u8>, Error> {
        if at == 0 {
            let whole = made()?;
            let mut s = self.state.borrow_mut();
            s.handles.get_mut(&h).ok_or(Error::Invalid)?.snap = whole;
        }
        let s = self.state.borrow();
        let slot = s.handles.get(&h).ok_or(Error::Invalid)?;
        Ok(part(&slot.snap, at, len))
    }

    /// The five numbers of the wire, made as the three of
    /// `/sys/info/mem` are and kept nowhere: reading twice is what a
    /// change is seen by.
    ///
    /// The borrow of the wire is taken and dropped here. A read of a
    /// Plugin's own stands at depth one with the wire free, and a
    /// served read runs from `Host::serve` after it has let the wire
    /// go, so both reach this and neither holds it twice.
    fn carried(&self) -> Result<Vec<u8>, Error> {
        let host = self.host.upgrade().ok_or(Error::Gone)?;
        let link = host.wire().link();
        let said = format!(
            "messages {}\nbytes_out {}\nbytes_in {}\nlast_us {}\nworst_us {}",
            link.messages,
            link.bytes_out,
            link.bytes_in,
            link.last_us,
            link.worst_us,
        );
        Ok(said.into_bytes())
    }

    /// The code the Client leaves with, in decimal; an empty write is
    /// none, so a Plugin that changed its mind can take it back.
    fn ending(&self, buf: &[u8]) -> Result<usize, Error> {
        let text = core::str::from_utf8(buf).map_err(|_| Error::Invalid)?;
        let text = text.trim();
        let given: Option<u64> = (!text.is_empty())
            .then(|| text.parse())
            .transpose()
            .map_err(|_| Error::Invalid)?;
        let code = given.map(|code| u8::try_from(code).unwrap_or(u8::MAX));
        self.state.borrow_mut().exit = code;
        Ok(buf.len())
    }
}

impl Module for Dev {
    /// The cells, in the order `ls` shows them, and the ask the Client
    /// was started with: the file it names resolved, and with it the
    /// name a region over nothing would write.
    fn start(&self, ctx: &dyn Calls, home: Rd) -> Result<(), Error> {
        let opened = [open::decl()];
        let read = (Flags::READ, &[][..]);
        let rw = (Flags::READ.union(Flags::WRITE), &[][..]);
        let dir = (Flags::READ.union(Flags::CONTAINER), &[][..]);
        self.buffers.start(ctx, home)?;
        self.add(ctx, home, "clipboard", rw, Held::Clipboard)?;
        self.add(ctx, home, "cwd", read, Held::Cwd)?;
        let display = self.add(ctx, home, "display", dir, Held::Dir)?;
        let screen = self.add(ctx, display, "0", dir, Held::Dir)?;
        self.add(ctx, screen, "text", rw, Held::Display(Part::Text))?;
        let info = self.add(ctx, screen, "info", dir, Held::Dir)?;
        self.add(ctx, info, "rows", rw, Held::Display(Part::Rows))?;
        self.add(ctx, info, "cols", rw, Held::Display(Part::Cols))?;
        self.add(ctx, home, "exit", rw, Held::Exit)?;
        let input = self.add(ctx, home, "input", dir, Held::Dir)?;
        self.add(ctx, input, "keyboard", rw, Held::Keyboard)?;
        self.add(ctx, home, "link", read, Held::Link)?;
        self.add(ctx, home, "open", (Flags::WRITE, &opened), Held::Open)?;
        let windows = self.add(ctx, home, "windows", dir, Held::Dir)?;
        let zero = self.add(ctx, windows, "0", dir, Held::Dir)?;
        self.add(ctx, zero, "ask", read, Held::Ask)?;
        self.add(ctx, zero, "size", rw, Held::Size)?;
        self.add(ctx, zero, "view", read, Held::View)?;

        let host = self.host.upgrade().ok_or(Error::Gone)?;
        let client = host.wire().cid();
        let name = {
            let mut s = self.state.borrow_mut();
            s.home = Some(home);
            s.client = client;
            core::mem::take(&mut s.ask.name)
        };
        let target = file_rd(ctx, &name)?;
        let mut s = self.state.borrow_mut();
        // A file that is there needs no making; one that is not is the
        // name a first write makes.
        s.ask.name = if target.is_some() { String::new() } else { name };
        s.ask.target = target.unwrap_or(NONE);
        s.asked = Some(s.ask.clone());
        Ok(())
    }
}

impl Driver for Dev {
    fn open(&self, _: &dyn Calls, rd: Rd, _: Rights) -> Reply<u64> {
        let mut s = self.state.borrow_mut();
        let h = s.next;
        s.next += 1;
        // A handle on the keyboard starts at the end of the stream.
        let at = s.keys.end();
        s.handles.insert(h, Slot { rd, at, snap: Vec::new() });
        Reply::ok(h)
    }

    fn read(
        &self,
        ctx: &dyn Calls,
        h: u64,
        at: u64,
        len: usize,
    ) -> Reply<Vec<u8>> {
        self.reading(ctx, h, at, len).into()
    }

    fn write(
        &self,
        ctx: &dyn Calls,
        h: u64,
        at: u64,
        buf: &[u8],
    ) -> Reply<usize> {
        self.writing(ctx, h, at, buf).into()
    }

    fn call(
        &self,
        ctx: &dyn Calls,
        h: u64,
        _: Hash,
        method: u16,
        input: &[u8],
    ) -> Reply<Vec<u8>> {
        let Ok((held, rd, _)) = self.slot(h) else {
            return Reply::err(Error::Invalid);
        };
        if held != Held::Open {
            return Reply::err(Error::Unsupported);
        }
        open::dispatch(self, ctx, rd, method, input).into()
    }

    fn close(&self, _: &dyn Calls, h: u64) -> Reply<()> {
        self.state.borrow_mut().handles.remove(&h);
        Reply::ok(())
    }
}

impl open::Api for Dev {
    fn open(&self, _: &dyn Calls, _: Rd, ask: open::Ask) -> Result<(), Error> {
        self.state.borrow_mut().asked = Some(ask);
        Ok(())
    }

    fn back(&self, _: &dyn Calls, _: Rd) -> Result<(), Error> {
        self.state.borrow_mut().back = true;
        Ok(())
    }
}

impl Dev {
    /// Read the whole:
    fn reading(
        &self,
        ctx: &dyn Calls,
        h: u64,
        at: u64,
        len: usize,
    ) -> Result<Vec<u8>, Error> {
        let (held, _, place) = self.slot(h)?;
        match held {
            Held::Keyboard => self.pressed(h, place, len),
            Held::View => {
                self.viewed(ctx, at, u32::try_from(len).unwrap_or(u32::MAX))
            }
            Held::Display(part) => {
                self.snapped(h, at, len, || Ok(self.state.borrow().shown(part)))
            }
            Held::Link => self.snapped(h, at, len, || self.carried()),
            // Each read asks the machine: the cell holds nothing.
            Held::Clipboard => Ok(part(&(self.clip.read)()?, at, len)),
            Held::Open | Held::Dir => Err(Error::Unsupported),
            kept => Ok(part(&self.kept(kept)?, at, len)),
        }
    }

    fn writing(
        &self,
        ctx: &dyn Calls,
        h: u64,
        at: u64,
        buf: &[u8],
    ) -> Result<usize, Error> {
        let (held, rd, _) = self.slot(h)?;
        match held {
            Held::Keyboard => self.typed(buf),
            Held::Exit => self.ending(buf),
            // The size the region is drawn for, which the tui writes
            // and a Plugin following the cell is told of, as it is
            // told of a frame.
            Held::Size => {
                self.state.borrow_mut().size = decode(buf)?;
                ctx.changed(rd, Kind::Modify)?;
                Ok(buf.len())
            }
            // The frame, or a fact of the display's own size, whole,
            // at 0 only. The announcement is what a remote follower
            // wakes on; with no handle watching, the Host keeps
            // nothing, so a frame nobody reads costs the write and no
            // more.
            Held::Display(part) if at == 0 => {
                self.state.borrow_mut().set(part, buf.to_vec());
                ctx.changed(rd, Kind::Modify)?;
                Ok(buf.len())
            }
            // The bytes whole, at 0 only: the machine takes no offset.
            Held::Clipboard if at == 0 => {
                (self.clip.write)(buf).map(|()| buf.len())
            }
            Held::Clipboard | Held::Display(_) => Err(Error::Invalid),
            _ => Err(Error::Unsupported),
        }
    }
}

/// The handle the region is read through: the `view` of the Plugin
/// loaded under `name`.
fn opened(ctx: &dyn Calls, name: &str) -> Result<Handle, Error> {
    let home = ops::lookup(ctx, SELF, name)?;
    ctx.open(ops::lookup(ctx, home, "view")?, Rights::READ)
}

/// An ask the table has no Plugin for: the region stays as it is and
/// the log says so, `open` carrying no answer back to the Plugin that
/// asked.
fn unserved(ctx: &dyn Calls, ask: &open::Ask) -> Result<(), Error> {
    warned(ctx, ask, Error::Unsupported)
}

/// An ask the region was not given to, and why, in the log: the one
/// place it is said, `open` carrying no answer back.
fn warned(ctx: &dyn Calls, ask: &open::Ask, error: Error) -> Result<(), Error> {
    let said = format!(
        "{WARN}open rd={} plugin={} {}",
        ask.target.0,
        ask.plugin,
        word(error)
    );
    let entry = cell::resolve(ctx, ROOT, "/sys/log")?;
    ops::put(ctx, entry, said.into_bytes()).map(|_| ())
}

/// What the table is asked for an rd: the Plugin of a Container, or the
/// Plugin of a cell, nothing on disk being a cell's shape too.
fn want_of(ctx: &dyn Calls, target: Rd) -> Result<Want<'static>, Error> {
    if target == NONE {
        return Ok(Want::Cell);
    }
    let flags = ops::info(ctx, target)?.flags;
    if flags.contains(Flags::CONTAINER) {
        return Ok(Want::Container);
    }
    Ok(Want::Cell)
}

/// The rd of `file`, a name under the root, or `None` when there is no
/// such file: what a region over nothing is opened for. The two ways
/// the tree says nothing is there are a name not on disk and a name
/// below a file.
fn file_rd(ctx: &dyn Calls, file: &str) -> Result<Option<Rd>, Error> {
    if file.is_empty() {
        return Ok(None);
    }
    match cell::resolve(ctx, ROOT, &format!("{MIRROR}/{file}")) {
        Err(Error::Gone | Error::Unsupported) => Ok(None),
        found => found.map(Some),
    }
}
