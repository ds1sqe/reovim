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

use std::cell::{OnceCell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

use reovim_client::dev::{self, Table, Want};
use reovim_client::{Client, Host, HostCalls, SELF, log};
use reovim_client_api::{Frames, Key, NONE, Region, keyboard, open, window};
use reovim_host::clock;
use reovim_kernel_interface::{
    Booted, Calls, Driver, DriverId, Error, Flags, Handle, Module, Rd, Reply,
    Rights,
};
use reovim_rig::{Rig, shown};
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
    /// The cells of `/self/dev`, found once at `start`.
    region: OnceCell<Region>,
    view: Frames,
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
        self.0.region.get().unwrap()
    }

    /// One frame: every key it has not taken, then the act it was
    /// given, then the View it answers.
    fn framed(&self, ctx: &dyn Calls) -> Result<Vec<u8>, Error> {
        let keys = self.region().keys(ctx)?;
        let act = {
            let script = &mut *self.0.script.borrow_mut();
            script.keys.extend(keys);
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
        let view = ctx.register(DriverId(0), home, Some("view"))?;
        ctx.report(view, Flags::READ, &[])?;
        let region = Region::of(ctx)?;
        let (ask, dir) = (region.ask(ctx)?, region.cwd(ctx)?);
        let size = region.size(ctx)?;
        let _ = self.0.region.set(region);
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
        self.0.view.read(at, len, || self.framed(ctx))
    }
}

/// The stubs a table gave, in the order it was asked for them.
type Kept = Rc<RefCell<Vec<Stub>>>;

/// The table these cases give: a fresh stub saying `word`, for a cell
/// under the name `stub` and for the name `cli` under `cli`. Every
/// other Want has no Plugin, which is what an unserved ask is.
fn stubs(word: &str) -> (Table, Kept) {
    let kept = Kept::default();
    let (word, keeps) = (String::from(word), kept.clone());
    let table: Table = Box::new(move |want| {
        let name = match want {
            Want::Cell => "stub",
            Want::Named("cli") => "cli",
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
    /// `dir`, at `line`.
    fn boot(
        rig: &Rig,
        table: Table,
        dir: &str,
        file: Option<&str>,
        line: Option<u64>,
    ) -> Self {
        let host = Rc::new(rig.host());
        let rd = dev::boot(&host, table, dir, file, line).unwrap();
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

    assert_eq!(it.names(it.rd), ["cwd", "exit", "input", "open", "windows"]);
    assert_eq!(it.names(it.at("windows/0")), ["ask", "size", "view"]);
    let size = it.at("windows/0/size");
    let at = ops::where_is(&it.calls(), size).unwrap();
    assert_eq!(at, "/self/dev/windows/0/size");
    // Nothing is loaded under `/self` but the Client's own two until
    // the first read of the view gives the region away.
    assert_eq!(it.names(SELF), ["bin", "dev"]);

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
