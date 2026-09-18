//! The tui on a Kernel with fs and text, beside a second Client, over
//! a stub Plugin.
//!
//! The stub is a Module the Client loaded: it registers the one cell a
//! Plugin serves, `view`, takes the keys it has not taken and answers
//! the View a test scripted. So each case judges only what the tui
//! owns: the keys it writes, the size it writes, the View laid out over
//! its own copy of the text that View names, the status row, the watch,
//! and the code it leaves with. What a key means is the Plugin's, and
//! the nvim corpus in plugins/vim judges that; the region's swap is
//! `dev`'s, and clients/core/tests/dev.rs judges that.

use std::cell::{OnceCell, RefCell};
use std::collections::VecDeque;
use std::rc::Rc;

use reovim_client::dev::{self, Clip, Table};
use reovim_client::{Client, ROOT};
use reovim_client_api::{
    BAR, BLOCK, BLUE, Frames, GREEN, Key, MATCH, NONE, RED, Region, SELECTION,
    open, window,
};
use reovim_client_tui::{Frame, Step, Tui};
use reovim_kernel_interface::{
    Booted, Calls, Driver, DriverId, Error, Flags, Handle, Module, Rd, Reply,
    Rights,
};
use reovim_rig::{Limits, Rig, buffer_over, shown};
use reovim_sdk::{cell, encode, ops};
use reovim_text_api::{history, publish, text};

/// What a scripted Plugin does with the next key.
#[derive(Clone, Debug)]
enum Act {
    /// That View to the region, over the Buffer it shows.
    Show(window::View),
    /// `text` in place of `len` bytes at `at`, on its Buffer.
    Edit(u64, u64, String),
    /// The head one revision back, then that View.
    Undo(window::View),
    /// The head published to its source.
    Save,
    /// The draft written to that file, which names a Buffer over
    /// nothing.
    SaveAs(String),
    /// That file made under the root, as the fs Module makes one.
    Make(String),
    /// A second Buffer over nothing holding that text: the Views it
    /// shows from then on name that one.
    Second(String),
    /// Leave with that code.
    Exit(u8),
}

/// What the stub was told and what it is to do next.
#[derive(Default)]
struct Script {
    /// What the region was opened with.
    ask: Option<open::Ask>,
    size: Option<window::Size>,
    /// Its own Buffer, and the handle it holds on it.
    buffer: Option<Rd>,
    h: Option<Handle>,
    /// The rd the Views it shows name: its own Buffer until a second
    /// one takes its place.
    shows: Option<Rd>,
    /// Every key it took, in order.
    keys: Vec<Key>,
    acts: VecDeque<Act>,
    /// The View it answers now.
    view: Option<window::View>,
}

impl Script {
    /// One key taken, and the act it was scripted for.
    fn took(&mut self, key: Key) -> Option<Act> {
        self.keys.push(key);
        self.acts.pop_front()
    }
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
    /// Its Module and its one Driver, for a table to give.
    fn booted(&self) -> Booted {
        let stub = Rc::new(self.clone());
        Booted { module: stub.clone(), drivers: vec![stub] }
    }

    /// What it does with the next key.
    fn will(&self, act: Act) {
        self.0.script.borrow_mut().acts.push_back(act);
    }

    /// The keys it took, in order.
    fn pressed(&self) -> Vec<Key> {
        self.0.script.borrow().keys.clone()
    }

    /// The sizes it was written, in order: the one it started with and
    /// every one after it.
    fn sizes(&self) -> Vec<(u64, u64)> {
        let size = self.0.script.borrow().size.clone();
        size.map(|size| (size.cols, size.rows)).into_iter().collect()
    }

    /// What the region was opened with.
    fn started(&self) -> open::Ask {
        self.0.script.borrow().ask.clone().unwrap()
    }

    /// The cells of `/self/dev` it holds.
    fn region(&self) -> &Region {
        self.0.region.get().unwrap()
    }

    /// That View over the Buffer the stub shows: a Plugin names the rd
    /// its View is in the bytes of, and the test says the rest.
    fn over(&self, view: &window::View) -> window::View {
        let mut view = view.clone();
        view.over = self.0.script.borrow().shows.unwrap_or(NONE);
        view
    }

    /// One frame: the keys it has not taken, each with the act it was
    /// given for it, and the View it shows now.
    fn framed(&self, ctx: &dyn Calls) -> Result<Vec<u8>, Error> {
        let keys = self.region().keys(ctx)?;
        for key in keys {
            let act = self.0.script.borrow_mut().took(key);
            act.map_or(Ok(()), |act| self.act(ctx, &act))?;
        }
        let held = self.region().size(ctx)?;
        let mut script = self.0.script.borrow_mut();
        script.size = Some(held);
        Ok(script.view.as_ref().map(encode).unwrap_or_default())
    }

    /// The one act, performed.
    fn act(&self, mut ctx: &dyn Calls, act: &Act) -> Result<(), Error> {
        let h = self.0.script.borrow().h.unwrap();
        match act {
            Act::Show(view) => {
                let shown = self.over(view);
                self.0.script.borrow_mut().view = Some(shown);
                Ok(())
            }
            Act::Edit(at, len, text) => {
                let edit =
                    text::Edit { at: *at, len: *len, text: text.clone() };
                text::Call(&mut ctx, h).replace(edit)
            }
            Act::Undo(view) => {
                history::Call(&mut ctx, h).undo()?;
                let shown = self.over(view);
                self.0.script.borrow_mut().view = Some(shown);
                Ok(())
            }
            Act::Save => publish::Call(&mut ctx, h).save(),
            Act::SaveAs(name) => {
                let path = format!("/module/fs/root/{name}");
                let target = cell::resolve(ctx, ROOT, &path)?;
                publish::Call(&mut ctx, h).save_as(target)
            }
            Act::Make(name) => {
                let root = cell::resolve(ctx, ROOT, "/module/fs/root")?;
                let input = encode(name);
                ops::call_by_name(ctx, root, "create", input).map(|_| ())
            }
            Act::Second(text) => self.second(ctx, text),
            Act::Exit(code) => self.region().ends(ctx, *code),
        }
    }

    /// A Buffer of its own over nothing, holding `text`: what a Plugin
    /// drawing something that is not the Client's file makes.
    fn second(&self, mut ctx: &dyn Calls, text: &str) -> Result<(), Error> {
        let buffer = buffer_over(ctx, None)?;
        let h = ctx.open(buffer, Rights::READ.union(Rights::WRITE))?;
        let edit = text::Edit { at: 0, len: 0, text: String::from(text) };
        text::Call(&mut ctx, h).replace(edit)?;
        self.0.script.borrow_mut().shows = Some(buffer);
        Ok(())
    }
}

impl Module for Stub {
    fn start(&self, ctx: &dyn Calls, home: Rd) -> Result<(), Error> {
        let view = ctx.register(DriverId(0), home, Some("view"))?;
        ctx.report(view, Flags::READ, &[])?;
        let region = Region::of(ctx)?;
        let ask = region.ask(ctx)?;
        let source = (ask.target != NONE).then_some(ask.target);
        let buffer = buffer_over(ctx, source)?;
        let h = ctx.open(buffer, Rights::READ.union(Rights::WRITE))?;
        let _ = self.0.region.set(region);
        let script = &mut *self.0.script.borrow_mut();
        script.ask = Some(ask);
        (script.buffer, script.shows) = (Some(buffer), Some(buffer));
        script.h = Some(h);
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

/// A View over the whole text, with a block cursor at that byte. The
/// rd it is in the bytes of is the stub's to fill in.
fn view(cursor: u64, status: &str) -> window::View {
    window::View { cursor, ..shown(NONE, status) }
}

/// A styled span of bytes.
const fn span(from: u64, to: u64, style: u64) -> window::Span {
    window::Span { from, to, style }
}

/// The tui over `rig` with one stub Plugin for whatever the region is
/// asked to show, opened over `file`: the tui and the stub.
fn over(rig: &Rig, file: Option<&str>) -> (Tui, Stub) {
    let stub = Stub::default();
    let one = stub.clone();
    let table: Table =
        Box::new(move |_| Some((String::from("stub"), one.booted())));
    let host = rig.host();
    let dev = dev::boot(&host, table, Clip::none(), "", file, None).unwrap();
    (Tui::open(host, dev).unwrap(), stub)
}

/// One key, `act` what the stub does for it. The key itself is the
/// stub's to ignore: what a key means is the Plugin's.
fn press(tui: &mut Tui, stub: &Stub, act: Act) -> Result<Step, Error> {
    stub.will(act);
    tui.key(Key::Char('.'))
}

/// The frame at the size the app always asks for.
fn frame(tui: &mut Tui) -> Frame {
    tui.frame(80, 24).unwrap()
}

fn buffer_of(client: &mut Client, tui: &Tui) -> Rd {
    let buffer = tui.over().unwrap();
    let name = client.name(buffer).unwrap();
    assert_eq!(client.resolve(&format!("/module/text/{name}")), Ok(buffer));
    buffer
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
fn the_region_the_tui_opened_is_the_file_it_was_given() {
    let rig = Rig::new("opened");
    rig.file("note", b"first\n");
    let (tui, stub) = over(&rig, Some("note"));
    let mut other = rig.client();
    // The tui was the first to connect: session 1, its one client. It
    // writes nothing into its client rd; what this Client serves is
    // `ls /self/dev`, which clients/core judges.
    let clients = other.resolve("/session/1/client").unwrap();
    assert_eq!(other.children(clients).unwrap().len(), 1);

    let ask = stub.started();
    let note = other.resolve("/module/fs/root/note").unwrap();
    assert_eq!(ask.target, note, "the file the region is on");
    assert_eq!(ask.name, "", "a file that is there needs no making");
    assert_eq!(ask.line, 0);
    assert_eq!(stub.sizes(), [(80, 23)], "the size it started at");
    assert_eq!(tui.over(), None, "nothing laid out until a View names it");
}

#[test]
fn a_size_the_tui_has_not_written_is_written_before_the_frame() {
    let rig = Rig::new("resize");
    rig.file("note", b"first\n");
    let (mut tui, stub) = over(&rig, Some("note"));
    assert_eq!(
        press(&mut tui, &stub, Act::Show(view(0, ""))),
        Ok(Step::Changed)
    );
    assert_eq!(stub.sizes(), [(80, 23)], "the size it started at");
    frame(&mut tui);
    assert_eq!(stub.sizes(), [(80, 23)], "and the app asked for that one");
    tui.frame(40, 10).unwrap();
    tui.frame(40, 10).unwrap();
    assert_eq!(stub.sizes(), [(40, 9)], "the status row is not its");
}

#[test]
fn a_view_is_laid_out_over_the_tui_s_own_text() {
    let rig = Rig::new("layout");
    rig.file("note", "a\tb\n한글x\nlast\n".as_bytes());
    let (mut tui, stub) = over(&rig, Some("note"));
    // The cursor on `b`, the second line selected, `x` a match.
    let mut shown = view(2, "-- VISUAL LINE --");
    shown.spans = vec![span(4, 12, SELECTION), span(10, 11, MATCH)];
    assert_eq!(press(&mut tui, &stub, Act::Show(shown)), Ok(Step::Changed));
    let frame = frame(&mut tui);
    assert_eq!(frame.lines, ["a       b", "한글x", "last"], "tab to its stop");
    assert_eq!(frame.cursor, (0, 8), "the cell after the tab");
    assert_eq!(frame.selection, [(1, 0, 6)], "through its newline");
    assert_eq!(frame.marks, [(1, 4, 5)], "a wide char is two cells");

    // The top line is the line the top byte is on; a narrow region
    // leaves the rest out.
    let mut shown = view(12, "");
    shown.top = 5;
    assert_eq!(press(&mut tui, &stub, Act::Show(shown)), Ok(Step::Changed));
    let frame = tui.frame(4, 3).unwrap();
    assert_eq!(frame.lines, ["한글", "last"], "a wide char is kept whole");
    assert_eq!(frame.cursor, (1, 0));
}

#[test]
fn a_span_of_any_other_style_reaches_the_frame_with_its_number() {
    let rig = Rig::new("styled");
    rig.file("note", "bin  ls\n한글x\n".as_bytes());
    let (mut tui, stub) = over(&rig, Some("note"));
    // The two names of the first row and a wide one of the second, in
    // styles the tui draws neither of itself.
    let mut shown = view(0, "");
    shown.spans = vec![span(0, 3, BLUE), span(5, 7, GREEN), span(8, 14, RED)];
    assert_eq!(press(&mut tui, &stub, Act::Show(shown)), Ok(Step::Changed));
    let frame = frame(&mut tui);
    let styled = [(0, 0, 3, BLUE), (0, 5, 7, GREEN), (1, 0, 4, RED)];
    assert_eq!(frame.styled, styled, "each run of a row, in cells");
    assert_eq!((frame.selection, frame.marks), (Vec::new(), Vec::new()));
}

#[test]
fn a_view_over_another_buffer_is_laid_out_over_that_one() {
    let rig = Rig::new("over");
    rig.file("note", b"first\n");
    let (mut tui, stub) = over(&rig, Some("note"));
    assert_eq!(
        press(&mut tui, &stub, Act::Show(view(0, ""))),
        Ok(Step::Changed)
    );
    let own = tui.over().unwrap();
    assert_eq!(frame(&mut tui).lines, ["first"]);

    // A Buffer of the Plugin's own, over nothing: the View names it,
    // and the tui lays out over that instead.
    let second = Act::Second("listing\nhere\n".into());
    assert_eq!(press(&mut tui, &stub, second), Ok(Step::Same), "nothing shown");
    let shown = Act::Show(view(8, "-- LIST --"));
    assert_eq!(press(&mut tui, &stub, shown), Ok(Step::Changed));
    let laid = tui.over().unwrap();
    assert_ne!(laid, own, "another rd than the one it started over");
    let frame = frame(&mut tui);
    assert_eq!(frame.lines, ["listing", "here"], "that Buffer's text");
    // A Buffer over nothing adds no name: the region is the rd it was
    // opened on, and the revisions are the laid Buffer's.
    assert_eq!(frame.status, "note  2:1  r2[+]   -- LIST --");

    // And what was laid before is let go.
    let mut other = rig.client();
    assert_eq!(open_on(&mut other, own), 1, "only the Plugin's own handle");
}

#[test]
fn the_cursor_s_shape_says_which_cell_of_a_tab_it_takes() {
    let rig = Rig::new("shape");
    rig.file("note", b"\tx\n");
    let (mut tui, stub) = over(&rig, Some("note"));
    let shown = view(0, "");
    assert_eq!(press(&mut tui, &stub, Act::Show(shown)), Ok(Step::Changed));
    let normal = frame(&mut tui);
    assert_eq!(normal.cursor, (0, 7), "a block is on the tab's last cell");
    assert_eq!(normal.shape, BLOCK, "and the app draws it as one");

    let mut shown = view(0, "-- INSERT --");
    shown.shape = BAR;
    assert_eq!(press(&mut tui, &stub, Act::Show(shown)), Ok(Step::Changed));
    let inserting = frame(&mut tui);
    assert_eq!(inserting.cursor, (0, 0), "a bar stands before the tab");
    assert_eq!(inserting.shape, BAR);
}

#[test]
fn the_status_row_is_the_tui_s_own() {
    let rig = Rig::new("status");
    rig.file("note", "한글\nx\n".as_bytes());
    let (mut tui, stub) = over(&rig, Some("note"));
    // Nothing is laid out before the first View, so there are no
    // revisions to name; the region is the file it was opened on.
    assert_eq!(frame(&mut tui).status, "note  1:1  r0   ", "before a show");
    let shown = Act::Show(view(8, "-- INSERT --"));
    assert_eq!(press(&mut tui, &stub, shown), Ok(Step::Changed));
    let status = frame(&mut tui).status;
    assert_eq!(status, "note  2:2  r1   -- INSERT --", "the cursor in chars");
    let edit = Act::Edit(0, 0, "y".into());
    assert_eq!(press(&mut tui, &stub, edit), Ok(Step::Changed));
    assert_eq!(
        press(&mut tui, &stub, Act::Show(view(9, ""))),
        Ok(Step::Changed)
    );
    assert_eq!(frame(&mut tui).status, "note  2:2  r2[+]   ", "unsaved");
    assert_eq!(press(&mut tui, &stub, Act::Save), Ok(Step::Changed));
    assert_eq!(frame(&mut tui).status, "note  2:2  r2   ", "written");
    assert_eq!(rig.disk("note"), "y한글\nx\n".as_bytes());
}

#[test]
fn a_plugin_that_never_shows_draws_an_empty_frame() {
    let rig = Rig::new("quiet");
    rig.file("note", b"first\n");
    let (mut tui, _) = over(&rig, Some("note"));
    let frame = frame(&mut tui);
    assert_eq!(frame.lines, Vec::<String>::new(), "nothing to draw");
    assert_eq!(frame.cursor, (0, 0));
    assert_eq!(frame.status, "note  1:1  r0   ", "the status row is the tui's");
    assert_eq!(tui.key(Key::Char('x')), Ok(Step::Same));
}

#[test]
fn the_exit_code_is_the_plugin_s() {
    let rig = Rig::new("exit");
    rig.file("note", b"first\n");
    let (mut tui, stub) = over(&rig, Some("note"));
    assert_eq!(
        press(&mut tui, &stub, Act::Show(view(0, ""))),
        Ok(Step::Changed)
    );
    assert_eq!(press(&mut tui, &stub, Act::Exit(3)), Ok(Step::Quit(3)));
    assert_eq!(tui.tick(), Ok(Step::Quit(3)), "and it stays gone");
}

#[test]
fn keys_change_the_draft_and_save_reaches_disk() {
    let rig = Rig::new("keys");
    rig.file("note", b"first\n");
    let (mut tui, stub) = over(&rig, Some("note"));
    assert_eq!(
        press(&mut tui, &stub, Act::Show(view(0, ""))),
        Ok(Step::Changed)
    );
    let mut other = rig.client();
    let buffer = buffer_of(&mut other, &tui);

    let edit = Act::Edit(0, 0, "x".into());
    assert_eq!(press(&mut tui, &stub, edit), Ok(Step::Changed));
    assert_eq!(other.read_all(buffer).unwrap(), b"xfirst\n");
    assert_eq!(rig.disk("note"), b"first\n", "disk waits for save");
    assert_eq!(
        press(&mut tui, &stub, Act::Show(view(1, ""))),
        Ok(Step::Changed)
    );
    assert_eq!(frame(&mut tui).lines, ["xfirst"], "the tui read it again");
    assert_eq!(tui.tick(), Ok(Step::Same), "its own edits change nothing");
    assert_eq!(press(&mut tui, &stub, Act::Save), Ok(Step::Changed));
    assert_eq!(rig.disk("note"), b"xfirst\n");
    assert!(frame(&mut tui).status.starts_with("note  1:2  r2   "));
}

#[test]
fn undo_and_redo_walk_the_revisions() {
    let rig = Rig::new("undo");
    rig.file("note", b"first\n");
    let (mut tui, stub) = over(&rig, Some("note"));
    let edit = Act::Edit(0, 0, "x".into());
    assert_eq!(press(&mut tui, &stub, edit), Ok(Step::Same));
    assert_eq!(
        press(&mut tui, &stub, Act::Show(view(0, ""))),
        Ok(Step::Changed)
    );
    assert_eq!(frame(&mut tui).lines, ["xfirst"]);
    let undo = Act::Undo(view(0, ""));
    assert_eq!(press(&mut tui, &stub, undo), Ok(Step::Changed));
    let frame = frame(&mut tui);
    assert_eq!(frame.lines, ["first"], "the earlier text, read again");
    assert_eq!(frame.status, "note  1:1  r1   ", "and the revision with it");
}

#[test]
fn a_write_from_another_client_reaches_the_tui_s_own_copy() {
    let rig = Rig::new("modify");
    rig.file("note", b"first\n");
    let (mut tui, stub) = over(&rig, Some("note"));
    assert_eq!(
        press(&mut tui, &stub, Act::Show(view(0, ""))),
        Ok(Step::Changed)
    );
    let mut other = rig.client();
    let buffer = buffer_of(&mut other, &tui);

    assert_eq!(tui.tick(), Ok(Step::Same));
    other.write_all(buffer, b"other\nlines".to_vec()).unwrap();
    assert_eq!(tui.tick(), Ok(Step::Changed), "the tui read it again");
    assert_eq!(tui.tick(), Ok(Step::Same));
    let frame = frame(&mut tui);
    assert_eq!(frame.lines, ["other", "lines"]);
    assert!(frame.status.contains("[+]"), "{}", frame.status);
}

#[test]
fn a_tui_behind_a_full_ring_reads_everything_again() {
    let rig =
        Rig::with("overflow", Limits { watch_ring: 2, ..Limits::default() });
    rig.file("note", b"first\n");
    let (mut tui, stub) = over(&rig, Some("note"));
    assert_eq!(
        press(&mut tui, &stub, Act::Show(view(0, ""))),
        Ok(Step::Changed)
    );
    let mut other = rig.client();
    let buffer = buffer_of(&mut other, &tui);
    for text in [b"one".as_slice(), b"two", b"three"] {
        other.write_all(buffer, text.to_vec()).unwrap();
    }
    assert_eq!(tui.tick(), Ok(Step::Changed));
    assert_eq!(frame(&mut tui).lines, ["three"]);
    assert!(frame(&mut tui).status.contains("r4[+]"));
}

#[test]
fn a_missing_file_is_a_region_over_nothing_that_keeps_its_name() {
    let rig = Rig::new("missing");
    let (mut tui, stub) = over(&rig, Some("none"));
    // Over nothing, but named: the Plugin is handed the name to make.
    let ask = stub.started();
    assert_eq!((ask.target, ask.name), (NONE, "none".to_string()));
    assert_eq!(frame(&mut tui).status, "none  1:1  r0   ");
    assert_eq!(
        press(&mut tui, &stub, Act::Show(view(0, ""))),
        Ok(Step::Changed)
    );
    assert_eq!(frame(&mut tui).lines, [""], "one empty line");
    assert!(!rig.dir().join("none").exists(), "nothing on disk yet");

    let edit = Act::Edit(0, 0, "x\n".into());
    assert_eq!(press(&mut tui, &stub, edit), Ok(Step::Changed));
    assert!(!rig.dir().join("none").exists(), "nor after an edit");

    // The Plugin makes the file and its Buffer goes on as that file's:
    // the tui follows the source it took.
    assert_eq!(
        press(&mut tui, &stub, Act::Make("none".into())),
        Ok(Step::Same)
    );
    let named = Act::SaveAs("none".into());
    assert_eq!(press(&mut tui, &stub, named), Ok(Step::Changed));
    assert_eq!(rig.disk("none"), b"x\n");
    assert_eq!(frame(&mut tui).status, "none  1:1  r2   ");
}

#[test]
fn the_tui_names_the_laid_rd_when_it_takes_a_source() {
    let rig = Rig::new("unnamed");
    let (mut tui, stub) = over(&rig, None);
    assert_eq!(frame(&mut tui).status, "[No Name]  1:1  r0   ");
    let ask = stub.started();
    assert_eq!((ask.target, ask.name), (NONE, String::new()), "no rd, no name");
    assert_eq!(
        press(&mut tui, &stub, Act::Show(view(0, ""))),
        Ok(Step::Changed)
    );
    let edit = Act::Edit(0, 0, "hi\n".into());
    assert_eq!(press(&mut tui, &stub, edit), Ok(Step::Changed));
    assert_eq!(frame(&mut tui).status, "[No Name]  1:1  r2[+]   ");
    // `:w NAME` is the Plugin's, and the Buffer it saves goes on as
    // that file's: the tui follows it by name.
    assert_eq!(
        press(&mut tui, &stub, Act::Make("note".into())),
        Ok(Step::Same)
    );
    let named = Act::SaveAs("note".into());
    assert_eq!(press(&mut tui, &stub, named), Ok(Step::Changed));
    assert_eq!(rig.disk("note"), b"hi\n");
    assert_eq!(frame(&mut tui).status, "note  1:1  r2   ");
}

#[test]
fn every_key_the_tui_writes_reaches_the_plugin_once_in_order() {
    let rig = Rig::new("presses");
    rig.file("note", b"first\n");
    let (mut tui, stub) = over(&rig, Some("note"));
    let keys = [Key::Char('한'), Key::Ctrl('r'), Key::Esc, Key::Tab];
    for (n, key) in keys.iter().enumerate() {
        stub.will(Act::Show(view(n as u64, "")));
        assert_eq!(tui.key(*key), Ok(Step::Changed), "a show is a draw");
    }
    assert_eq!(stub.pressed(), keys, "every key, and nothing else");
    assert_eq!(tui.key(Key::Char('x')), Ok(Step::Same), "no show, no draw");
}
