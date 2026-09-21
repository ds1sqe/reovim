//! `/self/dev` on a Kernel with fs and text, over a stub Plugin.
//!
//! The stub is a Module the Client loaded: it registers the one cell a
//! Plugin serves, `view`, reads what the region was opened with, takes
//! the keys it has not taken and answers the View a test wrote for it.
//! So each case judges only what `dev` owns: the keyboard as a stream,
//! the region's cells, the routing of the view, and the swap an `open`
//! asks for.
//!
//! The rig's own stub shows a Buffer and does no more; these cases
//! script what a Plugin asks for, so the stub here has a queue.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use reovim_client::dev::{self, Table, Want};
use reovim_client::{Client, Clip, Host, HostCalls, ROOT, SELF, bin, log};
use reovim_client_api::{Key, NONE, keyboard, open, window};
use reovim_client_region::{Region, Surface};
use reovim_host::clock;
use reovim_kernel_interface::{
    Booted, Calls, Driver, Error, Handle, Kind, Module, Rd, Reply, Rights,
};
use reovim_rig::bins::said;
use reovim_rig::{Rig, screen, shown};
use reovim_sdk::{cell, decode, encode, ops};

/// Bytes asked for per read of a cell of `dev`.
const CHUNK: u32 = 4096;

/// What a scripted Plugin does at its next frame.
#[derive(Clone, Debug)]
enum Act {
    /// That rd asked of the Client, in the Plugin the name says.
    Open(Rd, String),
    /// The region back to what it showed before the last ask.
    Back,
    /// Leave with that code.
    Exit(u8),
}

/// What the stub was told and what it is to do next.
#[derive(Default)]
struct Script {
    /// The `view` it registered.
    view: Option<Rd>,
    /// What the region was opened with, and where the Client stands.
    ask: Option<open::Ask>,
    dir: String,
    size: Option<window::Size>,
    /// Every key it took, in order, and how many frames it ran.
    keys: Vec<Key>,
    frames: u32,
    acts: VecDeque<Act>,
    /// What its View says, which a test reads back through `dev`.
    word: String,
}

/// What a stub holds: its script, and the `view` cell it serves.
#[derive(Default)]
struct Held {
    script: RefCell<Script>,
    /// The `view` cell it serves and the cells of `/self/dev`.
    surface: Surface,
}

/// A Plugin of scripted calls. The state is shared, so a test writes
/// the script and reads what was recorded while the Host holds the
/// Module itself.
#[derive(Clone, Default)]
struct Stub(Rc<Held>);

impl Stub {
    /// One saying `word`, and its Module and Driver for a table.
    fn booted(word: &str) -> (Self, Booted) {
        let stub = Self::default();
        stub.0.script.borrow_mut().word = String::from(word);
        let held = Rc::new(stub.clone());
        (stub, Booted { module: held.clone(), drivers: vec![held] })
    }

    /// What `start` was told.
    fn started(&self) -> (open::Ask, String) {
        let script = self.0.script.borrow();
        (script.ask.clone().unwrap(), script.dir.clone())
    }

    /// The keys it took, in order.
    fn pressed(&self) -> Vec<Key> {
        self.0.script.borrow().keys.clone()
    }

    /// How many frames it has run.
    fn frames(&self) -> u32 {
        self.0.script.borrow().frames
    }

    /// The `view` cell it registered.
    fn view(&self) -> Rd {
        self.0.script.borrow().view.unwrap()
    }

    /// The size it was told at `start`.
    fn size(&self) -> Option<window::Size> {
        self.0.script.borrow().size.clone()
    }

    /// What it does at its next frame.
    fn will(&self, act: Act) {
        self.0.script.borrow_mut().acts.push_back(act);
    }

    /// The cells of `/self/dev` it holds.
    fn region(&self) -> &Region {
        self.0.surface.region().unwrap()
    }

    /// One frame: every key it has not taken, then the act it was
    /// given, then the View it answers.
    fn framed(&self, ctx: &dyn Calls) -> Result<Vec<u8>, Error> {
        let keys = self.region().keys(ctx)?;
        let act = {
            let script = &mut *self.0.script.borrow_mut();
            script.keys.extend(keys.iter().filter_map(|p| Key::of(p.key)));
            script.frames += 1;
            script.acts.pop_front()
        };
        act.map_or(Ok(()), |act| self.act(ctx, &act))?;
        let word = self.0.script.borrow().word.clone();
        Ok(encode(&shown(NONE, &word)))
    }

    /// The one act, performed.
    fn act(&self, ctx: &dyn Calls, act: &Act) -> Result<(), Error> {
        let region = self.region();
        match act {
            Act::Open(target, plugin) => region.opens(ctx, *target, plugin),
            Act::Back => region.backs(ctx),
            Act::Exit(code) => region.ends(ctx, *code),
        }
    }
}

impl Module for Stub {
    fn start(&self, ctx: &dyn Calls, home: Rd) -> Result<(), Error> {
        let view = self.0.surface.start(ctx, home)?;
        let region = self.region();
        let (ask, dir) = (region.ask(ctx)?, region.cwd(ctx)?);
        let size = region.size(ctx)?;
        let script = &mut *self.0.script.borrow_mut();
        script.view = Some(view);
        (script.ask, script.dir) = (Some(ask), dir);
        script.size = Some(size);
        Ok(())
    }
}

impl Driver for Stub {
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
        self.0.surface.read(at, len, || self.framed(ctx))
    }
}

/// A Plugin that will not start: `start` is the one fallible step of a
/// load, and a swap to this is a swap that fails.
struct Wont;

impl Module for Wont {
    fn start(&self, _: &dyn Calls, _: Rd) -> Result<(), Error> {
        Err(Error::Invalid)
    }
}

impl Driver for Wont {}

/// It, as the table answers for the name `bad`.
fn wont() -> Booted {
    let wont = Rc::new(Wont);
    Booted { module: wont.clone(), drivers: vec![wont] }
}

/// The stubs a table gave, in the order it was asked for them.
type Kept = Rc<RefCell<Vec<Stub>>>;

/// The table these cases give: a fresh stub saying `word`, for a cell
/// under the name `stub` and for the name `cli` under `cli`; the name
/// `bad` stands for a Plugin that will not start. Every other Want has
/// no Plugin, which is what an unserved ask is.
fn stubs(word: &str) -> (Table, Kept) {
    let kept = Kept::default();
    let (word, keeps) = (String::from(word), kept.clone());
    let table: Table = Box::new(move |want| {
        let name = match want {
            Want::Cell => "stub",
            Want::Named("cli") => "cli",
            Want::Named("bad") => return Some((String::from("bad"), wont())),
            _ => return None,
        };
        let (stub, booted) = Stub::booted(&word);
        keeps.borrow_mut().push(stub);
        Some((String::from(name), booted))
    });
    (table, kept)
}

/// A table with no Plugin at all: what a case that judges the cells
/// alone gives, the region staying empty.
fn none() -> Table {
    Box::new(|_| None)
}

/// A Client with `dev` loaded over a rig: what everything here asks,
/// and where `/self/dev` stands.
struct Dev {
    host: Rc<Host>,
    rd: Rd,
}

impl Dev {
    /// `dev` over `rig` with `table`, the region opened over `file` in
    /// `dir`, at `line`, and no machine behind the clipboard.
    fn boot(
        rig: &Rig,
        table: Table,
        dir: &str,
        file: Option<&str>,
        line: Option<u64>,
    ) -> Self {
        let host = rig.host();
        let rd =
            dev::boot(&host, table, Clip::none(), dir, file, line, "").unwrap();
        Self { host, rd }
    }

    /// `dev` over `rig` with `clip` for its machine and no Plugin at
    /// all: what the two cases of the clipboard cell ask.
    fn clipped(rig: &Rig, clip: Clip) -> Self {
        let host = rig.host();
        let rd = dev::boot(&host, none(), clip, "", None, None, "").unwrap();
        Self { host, rd }
    }

    /// The one door, taken for the statement that makes an op.
    fn calls(&self) -> HostCalls<'_> {
        self.host.own()
    }

    /// The cell `path` names under `/self/dev`.
    fn at(&self, path: &str) -> Rd {
        cell::resolve(&self.calls(), self.rd, path).unwrap()
    }

    /// One whole read of the region's view, which is one frame of the
    /// Plugin holding it.
    fn frame(&self) -> window::View {
        let view = self.at("windows/0/view");
        decode(&ops::bytes(&self.calls(), view).unwrap()).unwrap()
    }

    /// What the cell `rd` holds.
    fn bytes(&self, rd: Rd) -> Vec<u8> {
        ops::bytes(&self.calls(), rd).unwrap()
    }

    /// Those bytes into the cell `rd`.
    fn put(&self, rd: Rd, bytes: Vec<u8>) {
        ops::put(&self.calls(), rd, bytes).unwrap();
    }

    /// What the exit cell holds; empty until a code is written.
    fn exit(&self) -> Vec<u8> {
        self.bytes(self.at("exit"))
    }

    /// That key written to the keyboard, as the tui writes one.
    fn typed(&self, key: Key) {
        self.put(self.at("input/keyboard"), key.code().to_le_bytes().to_vec());
    }

    /// The names under `rd`, as `ls` shows them.
    fn names(&self, rd: Rd) -> Vec<String> {
        let own = self.calls();
        let ctx: &dyn Calls = &own;
        let mut names: Vec<String> = ops::children(ctx, rd)
            .unwrap()
            .into_iter()
            .filter_map(|child| ops::info(ctx, child).ok()?.name)
            .collect();
        names.sort();
        names
    }
}

/// One press from `h`, or nothing where the stream has none for it.
fn took(ctx: &dyn Calls, h: Handle) -> Result<Option<keyboard::Press>, Error> {
    match ops::read(ctx, h, 0, CHUNK) {
        Ok(bytes) => decode(&bytes).map(Some),
        Err(Error::Busy) => Ok(None),
        Err(error) => Err(error),
    }
}

/// The rds the `handle` lines of `/sys/core` name: every handle the
/// Kernel holds, whoever holds it.
fn handled(client: &mut Client) -> Vec<u64> {
    let core = client.resolve("/sys/core").unwrap();
    let text = String::from_utf8(client.read_all(core).unwrap()).unwrap();
    text.lines()
        .filter_map(|line| line.strip_prefix("handle "))
        .filter_map(|line| {
            line.split(' ').nth(2)?.strip_prefix("rd=")?.parse().ok()
        })
        .collect()
}

/// How many handles are open on `rd`, as the Kernel's core says.
fn open_on(client: &mut Client, rd: Rd) -> usize {
    handled(client).iter().filter(|on| **on == rd.0).count()
}

#[test]
fn a_key_is_one_write_and_a_stamped_press_each_reader_takes_for_itself() {
    let rig = Rig::new("dev-keys");
    let it = Dev::boot(&rig, none(), "", None, None);
    let own = it.calls();
    let ctx: &dyn Calls = &own;
    let keyboard = it.at("input/keyboard");
    let client = it.host.wire().cid();

    // A handle opened before the keys reads all three, each stamped
    // here: the clock the Host was made with, and this Client's rd.
    let first = ctx.open(keyboard, Rights::READ).unwrap();
    let before = clock::now();
    for key in [Key::Char('a'), Key::Ctrl('r'), Key::Esc] {
        it.typed(key);
    }
    let taken: Vec<keyboard::Press> =
        core::iter::from_fn(|| took(ctx, first).unwrap()).collect();
    let keys: Vec<Key> =
        taken.iter().map(|p| Key::of(p.key).unwrap()).collect();
    assert_eq!(keys, [Key::Char('a'), Key::Ctrl('r'), Key::Esc]);
    assert!(taken.iter().all(|p| p.source == client), "this Client wrote it");
    assert!(taken.iter().all(|p| p.when >= before), "the Host's clock");
    assert_eq!(took(ctx, first).unwrap(), None, "the stream is not over");

    // A second reader of the same stream takes none from the first,
    // and one opened after a key does not see it.
    let second = ctx.open(keyboard, Rights::READ).unwrap();
    assert_eq!(took(ctx, second).unwrap(), None, "it opened at the end");
    it.typed(Key::Tab);
    let third = ctx.open(keyboard, Rights::READ).unwrap();
    for h in [first, second] {
        let press = took(ctx, h).unwrap().unwrap();
        assert_eq!(Key::of(press.key), Some(Key::Tab));
    }
    assert_eq!(took(ctx, third).unwrap(), None, "opened after that key");
    ops::shut(ctx, &[first, second, third]);
}

#[test]
fn the_cells_of_dev_are_where_the_tree_says_they_are() {
    let rig = Rig::new("dev-cells");
    rig.file("note", b"first\n");
    let it = Dev::boot(&rig, none(), "", Some("note"), None);

    assert_eq!(
        it.names(it.rd),
        [
            "buffers",
            "clipboard",
            "cwd",
            "display",
            "exit",
            "input",
            "link",
            "open",
            "windows"
        ]
    );
    assert_eq!(it.names(it.at("windows/0")), ["ask", "size", "view"]);
    assert_eq!(it.names(it.at("display")), ["0"]);
    assert_eq!(it.names(it.at("display/0")), ["info", "text"]);
    assert_eq!(it.names(it.at("display/0/info")), ["cols", "rows"]);
    let size = it.at("windows/0/size");
    let at = ops::where_is(&it.calls(), size).unwrap();
    assert_eq!(at, "/self/dev/windows/0/size");
    // Nothing is loaded under `/self` but the Client's own two until
    // the first read of the view gives the region away.
    assert_eq!(it.names(SELF), ["bin", "dev"]);

    // A display holds only the facts it has: nothing has drawn on this
    // Host, so its own size and its frame all read empty, unlike
    // `windows/0/size`, whose `FIRST` is the region's documented
    // default.
    let rows = it.at("display/0/info/rows");
    let cols = it.at("display/0/info/cols");
    let text = it.at("display/0/text");
    assert_eq!(it.bytes(rows), b"");
    assert_eq!(it.bytes(cols), b"");
    assert_eq!(it.bytes(text), b"");
    it.put(rows, b"12\n".to_vec());
    it.put(cols, b"40\n".to_vec());
    assert_eq!(it.bytes(rows), b"12\n");
    assert_eq!(it.bytes(cols), b"40\n");

    // The region is taken for 80 by 23 until something writes a size.
    let held: window::Size = decode(&it.bytes(size)).unwrap();
    assert_eq!(held, window::Size { cols: 80, rows: 23 });
    let wrote = window::Size { cols: 40, rows: 9 };
    it.put(size, encode(&wrote));
    assert_eq!(decode::<window::Size>(&it.bytes(size)), Ok(wrote));

    // The exit code is empty until one is written, and in decimal after.
    assert_eq!(it.exit(), b"");
    it.put(it.at("exit"), b"3".to_vec());
    assert_eq!(it.exit(), b"3");
}

#[test]
fn the_link_cell_says_what_the_wire_has_carried_and_nothing_else() {
    let rig = Rig::new("dev-link");
    let it = Dev::boot(&rig, none(), "", None, None);
    let link = it.at("link");
    let said = |bytes: Vec<u8>| String::from_utf8(bytes).unwrap();
    let messages = |said: &str| -> u64 {
        let first = said.lines().next().unwrap();
        first.strip_prefix("messages ").unwrap().parse().unwrap()
    };

    // Five labelled numbers in one order, as `/sys/info/mem` is three,
    // and they are what this Client's own side of the wire holds.
    let counted = it.host.wire().link();
    let first = said(it.bytes(link));
    let shown = format!(
        "messages {}\nbytes_out {}\nbytes_in {}\nlast_us {}\nworst_us {}",
        counted.messages,
        counted.bytes_out,
        counted.bytes_in,
        counted.last_us,
        counted.worst_us,
    );
    assert_eq!(first, shown);
    assert!(counted.messages > 0, "the Connect is a message");
    assert!(counted.worst_us >= counted.last_us, "{counted:?}");

    // Made at the read and kept nowhere: the read of it is an op of
    // the Client's own and moves nothing, and one that reaches the
    // wire is what a second read sees.
    assert_eq!(said(it.bytes(link)), first, "a read of it was a message");
    cell::resolve(&it.calls(), ROOT, "/module/text").unwrap();
    let again = said(it.bytes(link));
    assert!(messages(&again) > messages(&first), "{again}");

    // One answer a whole read, however many chunks it takes: the tail
    // is served from what the read at 0 made. Here the numbers grow a
    // digit between the two chunks, which a cell made afresh at each
    // one would answer with a stray digit past its own end.
    let own = it.calls();
    let ctx: &dyn Calls = &own;
    let h = ctx.open(link, Rights::READ).unwrap();
    let whole = ops::read(ctx, h, 0, CHUNK).unwrap();
    let digits = |n: u64| format!("{n}").len();
    let was = digits(it.host.wire().link().messages);
    while digits(it.host.wire().link().messages) == was {
        cell::resolve(ctx, ROOT, "/module/text").unwrap();
    }
    let at = whole.len() as u64;
    let tail = ops::read(ctx, h, at, CHUNK).unwrap();
    assert!(tail.is_empty(), "the read was torn: {:?}", said(tail));
    ops::shut(ctx, &[h]);
}

#[test]
fn the_clipboard_cell_reads_and_writes_the_machine_whole() {
    let rig = Rig::new("dev-clipboard");
    let it = Dev::clipped(&rig, rig.clipping());
    let own = it.calls();
    let ctx: &dyn Calls = &own;
    let rights = Rights::READ.union(Rights::WRITE);
    let h = ctx.open(it.at("clipboard"), rights).unwrap();

    // Each read asks the machine, from the offset it was given; the
    // cell holds nothing of its own.
    rig.clip(b"one");
    assert_eq!(ops::read(ctx, h, 0, CHUNK), Ok(b"one".to_vec()));
    assert_eq!(ops::read(ctx, h, 1, 1), Ok(b"n".to_vec()));
    assert_eq!(ops::read(ctx, h, 3, CHUNK), Ok(Vec::new()), "past the end");

    // A write is the bytes whole, and only at 0.
    assert_eq!(ops::write(ctx, h, 0, b"two\n".to_vec()), Ok(4));
    assert_eq!(rig.clipboard(), b"two\n");
    assert_eq!(
        ops::write(ctx, h, 1, b"two\n".to_vec()),
        Err(Error::Invalid),
        "whole or not at all"
    );
    ops::shut(ctx, &[h]);
}

#[test]
fn a_client_given_no_machine_answers_the_clipboard_unsupported() {
    let rig = Rig::new("dev-clipboard-none");
    let it = Dev::clipped(&rig, Clip::none());
    let own = it.calls();
    let ctx: &dyn Calls = &own;
    let rights = Rights::READ.union(Rights::WRITE);
    let h = ctx.open(it.at("clipboard"), rights).unwrap();

    // What a host with no tool on PATH answers, so a Plugin reaches
    // its own sentence by the one road.
    assert_eq!(ops::read(ctx, h, 0, CHUNK), Err(Error::Unsupported));
    assert_eq!(ops::write(ctx, h, 0, b"x".to_vec()), Err(Error::Unsupported));
    ops::shut(ctx, &[h]);
}

#[test]
fn every_key_reaches_the_plugin_once_in_order() {
    let rig = Rig::new("dev-presses");
    rig.file("note", b"first\n");
    let (table, kept) = stubs("");
    let it = Dev::boot(&rig, table, "", Some("note"), None);

    it.frame();
    let stub = kept.borrow()[0].clone();
    assert_eq!(stub.pressed(), [], "nothing typed yet");
    let keys = [Key::Char('한'), Key::Ctrl('r'), Key::Esc, Key::Tab];
    for key in keys {
        it.typed(key);
    }
    it.frame();
    assert_eq!(stub.pressed(), keys, "every key, once, in order");
    it.frame();
    assert_eq!(stub.pressed(), keys, "and none of them again");
}

#[test]
fn a_view_is_the_whole_read_of_the_region() {
    let rig = Rig::new("dev-view");
    rig.file("note", b"first\n");
    let (table, kept) = stubs("-- STUB --");
    let it = Dev::boot(&rig, table, "", Some("note"), None);

    let view = it.frame();
    let stub = kept.borrow()[0].clone();
    assert_eq!(view.status, "-- STUB --", "the Plugin's own word");
    assert_eq!(stub.frames(), 1, "one whole read is one frame");
    // And the region's Plugin is a Module of the Client's, under the
    // name the table gave it.
    assert_eq!(it.names(SELF), ["bin", "dev", "stub"]);
    let at = ops::where_is(&it.calls(), stub.view()).unwrap();
    assert_eq!(at, "/self/stub/view");
}

#[test]
fn the_region_is_opened_over_the_file_the_client_was_started_with() {
    let rig = Rig::new("dev-start");
    rig.file("note", b"one\ntwo\nthree\n");
    let (table, kept) = stubs("");
    let it = Dev::boot(&rig, table, "d", Some("note"), Some(9));
    it.frame();

    let mut other = rig.client();
    let note = other.resolve("/module/fs/root/note").unwrap();
    let stub = kept.borrow()[0].clone();
    let (ask, dir) = stub.started();
    assert_eq!(ask.target, note, "the file the region is on");
    assert_eq!(ask.name, "", "a file that is there needs no making");
    assert_eq!(dir, "d", "where the Client stands, from cwd");
    // Only whoever has the text can find the byte, so the line is
    // handed over as it was given: plugins/vim judges where it lands.
    assert_eq!(ask.line, 9);
    assert_eq!(stub.size(), Some(window::Size { cols: 80, rows: 23 }));
}

#[test]
fn a_missing_file_is_a_region_over_nothing_that_keeps_its_name() {
    let rig = Rig::new("dev-missing");
    let (table, kept) = stubs("");
    let it = Dev::boot(&rig, table, "", Some("none"), None);
    it.frame();

    let (ask, _) = kept.borrow()[0].started();
    assert_eq!(ask.target, NONE, "there is no such file");
    assert_eq!(ask.name, "none", "the name a first write makes");
}

#[test]
fn open_swaps_the_region_to_the_plugin_the_table_gives_the_shape() {
    let rig = Rig::new("dev-open-cell");
    rig.file("note", b"first\n");
    rig.file("other", b"second\n");
    let (table, kept) = stubs("");
    let it = Dev::boot(&rig, table, "", Some("note"), None);
    it.frame();
    let mut other = rig.client();
    let target = other.resolve("/module/fs/root/other").unwrap();
    let held = it.calls().open(it.at("input/keyboard"), Rights::READ).unwrap();

    // The ask is acted on once the Plugin that made it has returned,
    // inside the read it was made in: the View that read answers is
    // the new Plugin's.
    let first = kept.borrow()[0].clone();
    first.will(Act::Open(target, String::new()));
    it.frame();
    let second = kept.borrow()[1].clone();
    assert_eq!(second.started().0.target, target, "started over the rd asked");
    assert_eq!(first.frames(), 2, "and the old one runs no more");
    it.frame();
    assert_eq!(first.frames(), 2);

    // What the old Plugin held went with it, and so did its name.
    assert_eq!(it.names(SELF), ["bin", "dev", "stub"]);
    assert_eq!(open_on(&mut other, target), 0, "nothing of the old is left");
    ops::shut(&it.calls(), &[held]);
}

#[test]
fn open_by_name_swaps_to_the_plugin_the_table_knows() {
    let rig = Rig::new("dev-open-named");
    rig.file("note", b"first\n");
    rig.file("other", b"second\n");
    let (table, kept) = stubs("");
    let it = Dev::boot(&rig, table, "", Some("note"), None);
    it.frame();
    let mut other = rig.client();
    let target = other.resolve("/module/fs/root/other").unwrap();

    kept.borrow()[0].will(Act::Open(target, String::from("cli")));
    it.frame();
    let named = kept.borrow()[1].clone();
    assert_eq!(named.started().0.target, target, "the name stands for it");
    assert_eq!(it.names(SELF), ["bin", "cli", "dev"]);
}

#[test]
fn back_gives_the_region_to_the_plugin_of_the_step_before_it() {
    let rig = Rig::new("dev-back");
    rig.file("note", b"first\n");
    rig.file("other", b"second\n");
    let (table, kept) = stubs("");
    let it = Dev::boot(&rig, table, "", Some("note"), None);
    it.frame();
    let mut other = rig.client();
    let note = other.resolve("/module/fs/root/note").unwrap();
    let target = other.resolve("/module/fs/root/other").unwrap();

    kept.borrow()[0].will(Act::Open(target, String::from("cli")));
    it.frame();
    assert_eq!(kept.borrow()[1].started().0.target, target, "it went there");

    // The step back is the file the region was over, in a Plugin the
    // table makes afresh by its shape.
    kept.borrow()[1].will(Act::Back);
    it.frame();
    assert_eq!(kept.borrow()[2].started().0.target, note, "and back again");
    assert_eq!(it.names(SELF), ["bin", "dev", "stub"]);

    // One step, and no stack: the step is spent, and the Client ends.
    kept.borrow()[2].will(Act::Back);
    it.frame();
    assert_eq!(it.exit(), b"0");
}

#[test]
fn back_with_no_step_leaves_with_nothing_lost() {
    let rig = Rig::new("dev-back-first");
    rig.file("note", b"first\n");
    let (table, kept) = stubs("");
    let it = Dev::boot(&rig, table, "", Some("note"), None);
    it.frame();

    // The first Plugin was the first thing shown, so there is nowhere
    // to go: the Client ends, as an exit of 0 would end it.
    assert_eq!(it.exit(), b"", "not yet");
    kept.borrow()[0].will(Act::Back);
    it.frame();
    assert_eq!(it.exit(), b"0");
}

#[test]
fn back_to_a_region_over_nothing_keeps_the_name_it_was_opened_with() {
    let rig = Rig::new("dev-back-unnamed");
    rig.file("other", b"second\n");
    let (table, kept) = stubs("");
    let it = Dev::boot(&rig, table, "", Some("note"), None);
    it.frame();
    let mut other = rig.client();
    let there = other.resolve("/module/fs/root/other").unwrap();

    kept.borrow()[0].will(Act::Open(there, String::from("cli")));
    it.frame();
    kept.borrow()[1].will(Act::Back);
    it.frame();
    let (ask, _) = kept.borrow()[2].started();
    assert_eq!(ask.target, NONE, "there was no such file, and still is not");
    assert_eq!(ask.name, "note", "the name a write makes");
}

#[test]
fn an_open_the_table_has_no_plugin_for_leaves_the_region_and_warns() {
    let rig = Rig::new("dev-open-none");
    rig.file("note", b"first\n");
    let (table, kept) = stubs("-- STUB --");
    let it = Dev::boot(&rig, table, "", Some("note"), None);
    it.frame();
    let mut other = rig.client();
    let note = other.resolve("/module/fs/root/note").unwrap();
    let root = other.resolve("/module/fs/root").unwrap();

    // A name the table lacks, then a shape it has no Plugin for.
    let stub = kept.borrow()[0].clone();
    stub.will(Act::Open(note, String::from("explorer")));
    it.frame();
    stub.will(Act::Open(root, String::new()));
    let held = it.frame();
    assert_eq!(held.status, "-- STUB --", "the region is where it was");

    // Nothing carries the answer back to the Plugin, so the log has it.
    let said: Vec<(String, String)> = log::tail(&mut other, 0)
        .unwrap()
        .iter()
        .filter(|entry| entry.text().starts_with("open rd="))
        .map(|entry| (entry.level_name().into(), entry.text()))
        .collect();
    assert_eq!(
        said,
        [
            (
                "warn".into(),
                format!("open rd={} plugin=explorer unsupported", note.0)
            ),
            ("warn".into(), format!("open rd={} plugin= unsupported", root.0)),
        ]
    );
}

#[test]
fn the_exit_code_a_plugin_wrote_is_what_the_cell_holds() {
    let rig = Rig::new("dev-exit");
    rig.file("note", b"first\n");
    let (table, kept) = stubs("");
    let it = Dev::boot(&rig, table, "", Some("note"), None);
    it.frame();

    kept.borrow()[0].will(Act::Exit(3));
    it.frame();
    assert_eq!(it.exit(), b"3");
    it.frame();
    assert_eq!(it.exit(), b"3", "it stays");
}

#[test]
fn a_whole_read_of_display_is_one_frame_per_handle() {
    let rig = Rig::new("dev-display");
    let it = Dev::boot(&rig, none(), "", None, None);
    let own = it.calls();
    let ctx: &dyn Calls = &own;
    let display = it.at("display/0/text");

    // The tui writes it whole, at 0 only, and a read gives it back.
    let first = encode(&screen(2, &["one", "two"]));
    it.put(display, first.clone());
    assert_eq!(it.bytes(display), first);
    let w = ctx.open(display, Rights::WRITE).unwrap();
    let off = ops::write(ctx, w, 1, first.clone());
    assert_eq!(off, Err(Error::Invalid), "a frame is whole or not at all");
    ops::shut(ctx, &[w]);

    // Two handles read it at once and each sees one frame: the read at
    // 0 is the snapshot, and the write between the chunks of the
    // second is not in what it is still reading.
    let reader = || ctx.open(display, Rights::READ).unwrap();
    let (a, b) = (reader(), reader());
    let head = ops::read(ctx, a, 0, 8).unwrap();
    assert_eq!(ops::read(ctx, b, 0, CHUNK), Ok(first.clone()));
    let second = encode(&screen(2, &["two", "one"]));
    it.put(display, second.clone());
    let mut whole = head;
    whole.extend(ops::read(ctx, a, 8, CHUNK).unwrap());
    assert_eq!(whole, first, "the frame a began on");
    let past = ops::read(ctx, a, first.len() as u64, CHUNK);
    assert_eq!(past, Ok(Vec::new()), "past the bytes ends the read");

    // And the read after it is the frame that stands now.
    assert_eq!(ops::read(ctx, a, 0, CHUNK), Ok(second));
    ops::shut(ctx, &[a, b]);
}

#[test]
fn a_swap_to_a_plugin_that_will_not_start_leaves_the_region_shown() {
    let rig = Rig::new("dev-open-refused");
    rig.file("note", b"first\n");
    let (table, kept) = stubs("-- STUB --");
    let it = Dev::boot(&rig, table, "", Some("note"), None);
    it.frame();
    let mut other = rig.client();
    let note = other.resolve("/module/fs/root/note").unwrap();

    // The Plugin the ask names will not start, so the region cannot be
    // given away: it stands on the ask it stood on, in a Plugin of
    // that ask's own, and the frame is a frame like any other.
    kept.borrow()[0].will(Act::Open(note, String::from("bad")));
    let held = it.frame();
    assert_eq!(held.status, "-- STUB --", "the region is where it was");
    let ask: open::Ask = decode(&it.bytes(it.at("windows/0/ask"))).unwrap();
    assert_eq!(ask.target, note, "over the file it was over");

    // Nothing of the Plugin that would not start is left, so its name
    // is free and the Client is not wedged: what the table serves is
    // served, and the ask that failed fails the same way as before.
    assert_eq!(it.names(SELF), ["bin", "dev", "stub"]);
    kept.borrow()[1].will(Act::Open(note, String::from("bad")));
    it.frame();
    assert_eq!(it.names(SELF), ["bin", "dev", "stub"], "and again");
    kept.borrow()[2].will(Act::Open(note, String::from("cli")));
    it.frame();
    assert_eq!(it.names(SELF), ["bin", "cli", "dev"], "the region moves");

    // Nothing carries the answer back to the Plugin that asked, so the
    // log has it, as an ask no Plugin serves does.
    let said: Vec<(String, String)> = log::tail(&mut other, 0)
        .unwrap()
        .iter()
        .filter(|entry| entry.text().starts_with("open rd="))
        .map(|entry| (entry.level_name().into(), entry.text()))
        .collect();
    let line = format!("open rd={} plugin=bad invalid", note.0);
    assert_eq!(said, [("warn".into(), line.clone()), ("warn".into(), line)]);
}

#[test]
fn the_names_the_client_is_nothing_without_are_not_unloaded() {
    let rig = Rig::new("dev-unload");
    rig.file("note", b"first\n");
    let (table, _) = stubs("");
    let it = Dev::boot(&rig, table, "", Some("note"), None);
    it.frame();
    let own = it.calls();
    let ctx: &dyn Calls = &own;
    let unload = cell::resolve(ctx, SELF, "bin/unload").unwrap();
    let taking = |name: &str| {
        let args = bin::unload::Args { local: true, name: String::from(name) };
        said(ctx, unload, &args)
    };

    // The devices the Client speaks through, the row the Host serves
    // itself, and the Plugin holding the region, which is `/self/dev`'s
    // to give away and nobody else's to take.
    assert_eq!(taking("dev"), Err(Error::Denied), "its own devices");
    assert_eq!(taking("bin"), Err(Error::Denied), "the Host's own row");
    assert_eq!(taking("stub"), Err(Error::Denied), "the region's Plugin");
    assert_eq!(it.names(SELF), ["bin", "dev", "stub"], "and none of it went");
    assert_eq!(it.frame().status, "", "the region reads as it read");

    // What this Client loaded of its own userland still goes.
    let coreutils = reovim_module_coreutils::boot("");
    it.host.load("coreutils", coreutils.module, coreutils.drivers).unwrap();
    assert_eq!(taking("coreutils").as_deref(), Ok(""));
    assert_eq!(it.names(SELF), ["bin", "dev", "stub"]);
}

#[test]
fn a_write_of_the_size_is_a_change_a_watcher_of_it_hears() {
    let rig = Rig::new("dev-size-changed");
    let it = Dev::boot(&rig, none(), "", None, None);
    let own = it.calls();
    let ctx: &dyn Calls = &own;
    let size = it.at("windows/0/size");
    let h = ops::watch(ctx, size).unwrap();
    let (_, cursor) = ops::since(ctx, 0).unwrap();

    // The tui writes the size the region is drawn for, so a Plugin
    // following the cell is told, as one following `display` is.
    it.put(size, encode(&window::Size { cols: 40, rows: 9 }));
    let (events, _) = ops::since(ctx, cursor).unwrap();
    let seen: Vec<(Rd, Kind)> = events.iter().map(|e| (e.rd, e.kind)).collect();
    assert_eq!(seen, [(size, Kind::Modify)]);
    ops::shut(ctx, &[h]);
}

#[test]
fn a_write_of_the_keyboard_is_as_many_keys_as_it_holds() {
    let rig = Rig::new("dev-typed");
    let it = Dev::boot(&rig, none(), "", None, None);
    let own = it.calls();
    let ctx: &dyn Calls = &own;
    let keyboard = it.at("input/keyboard");
    let h = ctx.open(keyboard, Rights::READ).unwrap();

    // Three codes in one op are three presses, in the order written.
    let typed = [Key::Char('i'), Key::Char('x'), Key::Esc];
    let mut codes = Vec::new();
    for key in typed {
        codes.extend(key.code().to_le_bytes());
    }
    assert_eq!(ops::put(ctx, keyboard, codes), Ok(24));
    let taken: Vec<Key> = core::iter::from_fn(|| took(ctx, h).unwrap())
        .filter_map(|press| Key::of(press.key))
        .collect();
    assert_eq!(taken, typed);

    // Whole codes or none: nine bytes are no run of them and the
    // stream is left where it was.
    assert_eq!(
        ops::put(ctx, keyboard, vec![0; 9]),
        Err(Error::Invalid),
        "half a key is no key"
    );
    assert_eq!(took(ctx, h).unwrap(), None, "nothing was pushed");
    ops::shut(ctx, &[h]);
}
