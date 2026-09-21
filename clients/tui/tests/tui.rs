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

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use reovim_client::dev::{self, Clip, Table};
use reovim_client::{Client, Host, ROOT};
use reovim_client_api::{
    BAR, BLOCK, BLUE, GREEN, Key, MATCH, NONE, RED, SELECTION, display, looks,
    open, window,
};
use reovim_client_region::{Region, Surface};
use reovim_client_tui::{Frame, Step, Tui};
use reovim_kernel_interface::{
    Booted, Calls, Decl, Driver, DriverId, Error, Flags, Handle, Hash, Module,
    Rd, Reply, Rights,
};
use reovim_rig::{Limits, Rig, buffer_over, shown};
use reovim_sdk::{cell, decode, encode, log, ops, sgr};
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
    /// The `view` cell it serves and the cells of `/self/dev`.
    surface: Surface,
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

    /// Its own Buffer, which the Views it shows name until a second
    /// one takes its place.
    fn buffer(&self) -> Rd {
        self.0.script.borrow().buffer.unwrap()
    }

    /// The cells of `/self/dev` it holds.
    fn region(&self) -> &Region {
        self.0.surface.region().unwrap()
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
        for key in keys.iter().filter_map(|press| Key::of(press.key)) {
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
        self.0.surface.start(ctx, home)?;
        let ask = self.region().ask(ctx)?;
        let source = (ask.target != NONE).then_some(ask.target);
        let buffer = buffer_over(ctx, source)?;
        let h = ctx.open(buffer, Rights::READ.union(Rights::WRITE))?;
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
        self.0.surface.read(at, len, || self.framed(ctx))
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
/// asked to show, opened over `file`: the tui, the stub and the Host
/// they stand on, which is what reads the cells the tui writes.
fn hosted(rig: &Rig, file: Option<&str>) -> (Tui, Stub, Rc<Host>) {
    let stub = Stub::default();
    let one = stub.clone();
    let table: Table =
        Box::new(move |_| Some((String::from("stub"), one.booted())));
    let host = rig.host();
    let dev =
        dev::boot(&host, table, Clip::none(), "", file, None, "").unwrap();
    (Tui::open(host.clone(), dev).unwrap(), stub, host)
}

/// As [`hosted`], for the cases that need no Host of their own.
fn over(rig: &Rig, file: Option<&str>) -> (Tui, Stub) {
    let (tui, stub, _) = hosted(rig, file);
    (tui, stub)
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
    // The cursor on `b`, the second line's name selected, `x` a match.
    let mut shown = view(2, "-- VISUAL LINE --");
    shown.spans = vec![span(4, 10, SELECTION), span(10, 11, MATCH)];
    assert_eq!(press(&mut tui, &stub, Act::Show(shown)), Ok(Step::Changed));
    let frame = frame(&mut tui);
    assert_eq!(frame.lines, ["a       b", "한글x", "last"], "tab to its stop");
    assert_eq!(frame.cursor, (0, 8), "the cell after the tab");
    let runs = [
        display::Run { from: 0, to: 6, style: sgr::look(SELECTION) },
        display::Run { from: 6, to: 7, style: sgr::look(MATCH) },
    ];
    assert_eq!(frame.runs[1], runs, "a wide char is two bytes, one look each");

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
fn a_span_of_any_other_style_reaches_the_frame_with_its_look() {
    let rig = Rig::new("styled");
    rig.file("note", "bin  ls\n한글x\n".as_bytes());
    let (mut tui, stub) = over(&rig, Some("note"));
    // The two names of the first row and a wide one of the second, in
    // looks the tui does not draw itself.
    let mut shown = view(0, "");
    shown.spans = vec![span(0, 3, BLUE), span(5, 7, GREEN), span(8, 14, RED)];
    assert_eq!(press(&mut tui, &stub, Act::Show(shown)), Ok(Step::Changed));
    let frame = frame(&mut tui);
    let row0 = [
        display::Run { from: 0, to: 3, style: sgr::look(BLUE) },
        display::Run { from: 5, to: 7, style: sgr::look(GREEN) },
    ];
    assert_eq!(frame.runs[0], row0, "each run of a row, in bytes");
    let row1 = [display::Run { from: 0, to: 6, style: sgr::look(RED) }];
    assert_eq!(frame.runs[1], row1, "a wide char is two of its own bytes");
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

#[test]
fn the_frame_the_tui_answers_is_what_display_holds() {
    let rig = Rig::new("display");
    rig.file("note", "한글x\n".as_bytes());
    let (mut tui, stub, host) = hosted(&rig, Some("note"));
    let ctx = host.own();
    let at = cell::resolve(&ctx, ROOT, "/self/dev/display/0/text").unwrap();
    let held = || -> display::Screen {
        decode(&ops::bytes(&ctx, at).unwrap()).unwrap()
    };

    // Nothing is drawn until the app asks for a frame.
    assert_eq!(ops::bytes(&ctx, at), Ok(Vec::new()));
    press(&mut tui, &stub, Act::Show(view(0, "-- INSERT --"))).unwrap();
    let frame = tui.frame(80, 24).unwrap();
    let screen = held();

    // What the terminal was asked for, and the frame it was answered:
    // one `Line` a row the Plugin showed, its text the frame's own,
    // the status apart.
    assert_eq!(screen.rows, 24);
    let info = |name: &str| {
        let path = format!("/self/dev/display/0/info/{name}");
        cell::resolve(&ctx, ROOT, &path).unwrap()
    };
    let fact = |at: Rd| ops::bytes(&ctx, at).unwrap();
    assert_eq!(fact(info("rows")), b"24\n", "the whole terminal");
    assert_eq!(fact(info("cols")), b"80\n");
    let texts: Vec<String> =
        screen.lines.iter().map(|line| line.text.clone()).collect();
    assert_eq!(texts, frame.lines, "the rows the Plugin showed");
    assert_eq!(screen.status, frame.status);

    // The cursor is the frame's own, turned from cells to bytes.
    assert_eq!((screen.row, screen.col), (0, 0));
    assert_eq!((screen.shape, frame.shape), (BLOCK, BLOCK));

    // A frame nobody asked for leaves the display as it was; the one
    // asked for redraws it.
    press(&mut tui, &stub, Act::Show(view(6, ""))).unwrap();
    assert_eq!(held(), screen, "a frame nobody asked for");
    tui.frame(80, 24).unwrap();
    assert_ne!(held(), screen);
    assert_eq!(held().col, 6, "the x, two wide chars along, in bytes");

    // A resize follows in the facts too.
    tui.frame(40, 10).unwrap();
    assert_eq!(fact(info("rows")), b"10\n");
    assert_eq!(fact(info("cols")), b"40\n");
}

#[test]
fn a_selection_and_a_match_reach_the_display_s_runs() {
    let rig = Rig::new("display-runs");
    rig.file("note", "bin  ls\n한글x\n".as_bytes());
    let (mut tui, stub, host) = hosted(&rig, Some("note"));
    let ctx = host.own();
    let at = cell::resolve(&ctx, ROOT, "/self/dev/display/0/text").unwrap();
    let held = || -> display::Screen {
        decode(&ops::bytes(&ctx, at).unwrap()).unwrap()
    };

    // A selection through the first row's newline and a match on the
    // second row's `x`: neither has anywhere to stand in the record
    // `08.0` replaces, `cells: [u64; 65536]` holding one scalar a cell
    // and no style at all, so this failed before it with nowhere to
    // put a run; here each lands on the right bytes of its own line.
    let mut shown = view(0, "");
    shown.spans = vec![span(0, 8, SELECTION), span(14, 15, MATCH)];
    press(&mut tui, &stub, Act::Show(shown)).unwrap();
    tui.frame(80, 24).unwrap();
    let screen = held();

    assert_eq!(screen.lines[0].text, "bin  ls");
    assert_eq!(
        screen.lines[0].runs,
        [display::Run { from: 0, to: 8, style: sgr::look(SELECTION) }],
        "through the line's own newline, one byte past its text"
    );
    assert_eq!(screen.lines[1].text, "한글x");
    assert_eq!(
        screen.lines[1].runs,
        [display::Run { from: 6, to: 7, style: sgr::look(MATCH) }],
        "the match on `x`, in the bytes of its own line"
    );
}

/// `text` shown with `spans` over it, row 0's runs once drawn: what a
/// later span left over an earlier one, in the bytes of its own line.
fn overlap(
    name: &str,
    text: &str,
    spans: Vec<window::Span>,
) -> Vec<display::Run> {
    let rig = Rig::new(name);
    rig.file("note", format!("{text}\n").as_bytes());
    let (mut tui, stub, host) = hosted(&rig, Some("note"));
    let ctx = host.own();
    let at = cell::resolve(&ctx, ROOT, "/self/dev/display/0/text").unwrap();
    let mut shown = view(0, "");
    shown.spans = spans;
    press(&mut tui, &stub, Act::Show(shown)).unwrap();
    tui.frame(80, 24).unwrap();
    let screen: display::Screen =
        decode(&ops::bytes(&ctx, at).unwrap()).unwrap();
    assert_eq!(screen.lines[0].text, text);
    screen.lines[0].runs.clone()
}

#[test]
fn a_selection_beginning_inside_a_match_is_whole() {
    // A match on `bin` and a selection that begins inside it and runs
    // on: the selection is on top of the whole of what it covers, the
    // match standing only where the selection does not reach.
    let spans = vec![span(0, 3, MATCH), span(1, 7, SELECTION)];
    let runs = overlap("overlap-begins", "bin  ls", spans);
    let expected = [
        display::Run { from: 0, to: 1, style: sgr::look(MATCH) },
        display::Run { from: 1, to: 7, style: sgr::look(SELECTION) },
    ];
    assert_eq!(runs, expected, "the selection stands whole");
}

#[test]
fn a_selection_over_a_match_inside_it_is_one_run() {
    // Vim gives its matches before its selection: a match entirely
    // inside a later selection must vanish under it whole, not clip
    // the selection down to where the match happens to start.
    let spans = vec![span(3, 5, MATCH), span(0, 10, SELECTION)];
    let runs = overlap("overlap-inside", "0123456789", spans);
    let expected =
        [display::Run { from: 0, to: 10, style: sgr::look(SELECTION) }];
    assert_eq!(runs, expected, "one run, the selection whole");
}

#[test]
fn a_match_partly_under_a_selection_is_its_head_then_the_selection() {
    let spans = vec![span(0, 6, MATCH), span(4, 10, SELECTION)];
    let runs = overlap("overlap-partial", "0123456789", spans);
    let expected = [
        display::Run { from: 0, to: 4, style: sgr::look(MATCH) },
        display::Run { from: 4, to: 10, style: sgr::look(SELECTION) },
    ];
    assert_eq!(runs, expected, "the match's head, then the selection");
}

fn retiring_drafts(name: &str) -> (Rig, Tui, Stub, Rc<Host>, Rd, Rd) {
    let rig = Rig::new(name);
    let (mut tui, stub, host) = hosted(&rig, None);
    let ctx = host.own();
    let old = reovim_client_region::drafted(&ctx, None).unwrap();
    let new = reovim_client_region::drafted(&ctx, None).unwrap();
    ops::put(&ctx, old, b"old".to_vec()).unwrap();
    ops::put(&ctx, new, b"new".to_vec()).unwrap();
    assert_eq!(ops::bytes(&ctx, old).unwrap(), b"old");
    stub.0.script.borrow_mut().shows = Some(old);
    assert_eq!(
        press(&mut tui, &stub, Act::Show(view(0, ""))),
        Ok(Step::Changed)
    );
    assert_eq!(tui.over(), Some(old));
    assert_eq!(frame(&mut tui).lines, ["old"]);
    let list = cell::resolve(&ctx, ROOT, "/self/dev/buffers").unwrap();
    ops::call_by_name(&ctx, list, "close", encode(&old)).unwrap();
    assert_eq!(ops::info(&ctx, old), Err(Error::Invalid));
    assert!(!ops::children(&ctx, list).unwrap().contains(&old));
    (rig, tui, stub, host, old, new)
}

#[test]
fn a_new_view_replaces_a_closed_laid_draft() {
    let (_rig, mut tui, stub, _host, _old, new) =
        retiring_drafts("tui-retired-view");
    stub.0.script.borrow_mut().shows = Some(new);
    assert_eq!(
        press(&mut tui, &stub, Act::Show(view(0, "new"))),
        Ok(Step::Changed)
    );
    assert_eq!(tui.over(), Some(new));
    assert_eq!(frame(&mut tui).lines, ["new"]);
}

#[test]
fn gone_on_the_current_view_still_fails() {
    let (_rig, mut tui, _stub, _host, _old, _new) =
        retiring_drafts("tui-current-gone");
    assert_eq!(tui.tick(), Err(Error::Gone));
}

#[test]
fn a_none_view_does_not_hide_current_gone() {
    let (_rig, mut tui, stub, _host, _old, _new) =
        retiring_drafts("tui-none-gone");
    stub.0.script.borrow_mut().shows = Some(NONE);
    assert_eq!(
        press(&mut tui, &stub, Act::Show(view(0, ""))),
        Err(Error::Gone)
    );
}

/// What a test hands the looks stub and what it recorded: the looks it
/// answers, the byte range each `runs` was asked for, and the errors it
/// answers before any of them.
#[derive(Default)]
struct Served {
    looks: Vec<looks::Look>,
    asked: Vec<(u64, u64)>,
    refuses: VecDeque<Error>,
}

/// A second Plugin over the Buffer the region's own shows, serving
/// `looks` for it under `/self/<plugin>/<n>/looks` and nothing else:
/// what a Plugin with a decoration to draw registers.
#[derive(Clone)]
struct Looks {
    /// The name the rd it serves looks over goes by, which is the name
    /// it registers its own child under.
    over: String,
    /// What it reports at that node: the `looks` declaration, or none,
    /// which makes it a node named `looks` that is no `looks` at all.
    decls: Vec<Decl>,
    served: Rc<RefCell<Served>>,
}

impl Looks {
    /// It serves those looks from now on.
    fn serves(&self, served: Vec<looks::Look>) {
        self.served.borrow_mut().looks = served;
    }

    /// The next `runs` fails with that error, and no other.
    fn refuses(&self, error: Error) {
        self.served.borrow_mut().refuses.push_back(error);
    }

    /// Every byte range it was asked for, in order.
    fn asked(&self) -> Vec<(u64, u64)> {
        self.served.borrow().asked.clone()
    }
}

impl Module for Looks {
    fn start(&self, ctx: &dyn Calls, home: Rd) -> Result<(), Error> {
        let flags = Flags::READ.union(Flags::CONTAINER);
        let over = ctx.register(DriverId(0), home, Some(&self.over))?;
        ctx.report(over, flags, &[])?;
        let node = ctx.register(DriverId(0), over, Some("looks"))?;
        ctx.report(node, Flags::READ, &self.decls)
    }
}

impl Driver for Looks {
    fn open(&self, _: &dyn Calls, rd: Rd, _: Rights) -> Reply<u64> {
        Reply::ok(rd.0)
    }

    fn call(
        &self,
        ctx: &dyn Calls,
        h: u64,
        api: Hash,
        number: u16,
        input: &[u8],
    ) -> Reply<Vec<u8>> {
        if api != looks::hash() {
            return Reply::err(Error::Unsupported);
        }
        looks::dispatch(self, ctx, Rd(h), number, input).into()
    }
}

impl looks::Api for Looks {
    fn runs(
        &self,
        _: &dyn Calls,
        _: Rd,
        args: looks::Range,
    ) -> Result<Vec<looks::Look>, Error> {
        let mut served = self.served.borrow_mut();
        served.asked.push((args.from, args.to));
        match served.refuses.pop_front() {
            Some(error) => Err(error),
            None => Ok(served.looks.clone()),
        }
    }
}

/// A Plugin over `over` loaded under `name`, serving looks for that rd
/// by the name the rd goes by, as a Plugin over a Buffer serves them
/// for the draft it follows: `decls` is what it reports at that node.
fn loading(host: &Rc<Host>, name: &str, over: Rd, decls: Vec<Decl>) -> Looks {
    let over = ops::info(&host.own(), over).unwrap().name.unwrap();
    let it = Rc::new(Looks { over, decls, served: Rc::default() });
    let drivers: Vec<Rc<dyn Driver>> = vec![it.clone()];
    host.load(name, it.clone(), drivers).unwrap();
    it.as_ref().clone()
}

/// One that declares the api, under `looks-stub`.
fn looking(host: &Rc<Host>, over: Rd) -> Looks {
    loading(host, "looks-stub", over, vec![looks::decl()])
}

/// The log keeps a `debug` entry from now on, which is the level the
/// tui says a looks Driver's trouble at.
fn debugged(host: &Rc<Host>) {
    let ctx = host.own();
    let rd = cell::resolve(&ctx, ROOT, "/sys/loglevel").unwrap();
    ops::put(&ctx, rd, b"debug".to_vec()).unwrap();
}

/// What `/sys/log` holds of the tui's own word for a looks Driver.
fn grumbles(host: &Rc<Host>) -> Vec<(u8, String)> {
    let ctx = host.own();
    let rd = cell::resolve(&ctx, ROOT, "/sys/log").unwrap();
    log::parse(&ops::bytes(&ctx, rd).unwrap())
        .iter()
        .filter(|entry| entry.text().starts_with("looks rd="))
        .map(|entry| (entry.level, entry.text()))
        .collect()
}

/// One look a Plugin serves over those bytes.
const fn looked(from: u64, to: u64, style: u64, priority: u64) -> looks::Look {
    looks::Look { from, to, style, priority }
}

/// The run a meaning draws as, in the bytes of the row it is on.
fn drawn(from: u64, to: u64, meaning: u64) -> display::Run {
    display::Run { from, to, style: sgr::look(meaning) }
}

#[test]
fn a_plugins_look_reaches_the_frame() {
    let rig = Rig::new("looks");
    rig.file("note", b"a bold word\n");
    let (mut tui, stub, host) = hosted(&rig, Some("note"));
    let looks = looking(&host, stub.buffer());
    looks.serves(vec![looked(2, 6, sgr::STRONG, 0)]);
    assert_eq!(
        press(&mut tui, &stub, Act::Show(view(0, ""))),
        Ok(Step::Changed)
    );
    let frame = frame(&mut tui);
    assert_eq!(frame.lines, ["a bold word"], "the text is nobody's to move");
    assert_eq!(frame.runs[0], [drawn(2, 6, sgr::STRONG)]);
}

#[test]
fn two_looks_compose_by_priority() {
    let rig = Rig::new("compose");
    rig.file("note", b"abcdef\n");
    let (mut tui, stub, host) = hosted(&rig, Some("note"));
    let looks = looking(&host, stub.buffer());
    // The higher priority stands second in the answer and first on the
    // overlap: what is painted is the order of `priority`.
    looks.serves(vec![
        looked(2, 4, sgr::STRONG, 1),
        looked(0, 6, sgr::EMPHASIS, 0),
    ]);
    press(&mut tui, &stub, Act::Show(view(0, ""))).unwrap();
    let runs = [
        drawn(0, 2, sgr::EMPHASIS),
        drawn(2, 4, sgr::STRONG),
        drawn(4, 6, sgr::EMPHASIS),
    ];
    assert_eq!(frame(&mut tui).runs[0], runs);
}

#[test]
fn the_regions_own_spans_stay_on_top() {
    let rig = Rig::new("on-top");
    rig.file("note", b"abcdef\n");
    let (mut tui, stub, host) = hosted(&rig, Some("note"));
    let looks = looking(&host, stub.buffer());
    looks.serves(vec![looked(0, 6, sgr::STRONG, 9)]);
    let mut shown = view(0, "-- VISUAL --");
    shown.spans = vec![span(2, 4, SELECTION)];
    press(&mut tui, &stub, Act::Show(shown)).unwrap();
    let runs = [
        drawn(0, 2, sgr::STRONG),
        drawn(2, 4, SELECTION),
        drawn(4, 6, sgr::STRONG),
    ];
    assert_eq!(frame(&mut tui).runs[0], runs, "however high the priority");
}

#[test]
fn a_hidden_cell_is_not_drawn_off_the_cursors_row() {
    let rig = Rig::new("hidden");
    rig.file("note", b"**bold**\nplain\n");
    let (mut tui, stub, host) = hosted(&rig, Some("note"));
    let looks = looking(&host, stub.buffer());
    looks.serves(vec![
        looked(0, 2, sgr::MARKER, 0),
        looked(6, 8, sgr::MARKER, 0),
        looked(2, 6, sgr::STRONG, 1),
    ]);
    // The cursor on the second row: the first closes up over its
    // markers, and the look that is left is in the bytes drawn.
    press(&mut tui, &stub, Act::Show(view(9, ""))).unwrap();
    let off = frame(&mut tui);
    assert_eq!(off.lines, ["bold", "plain"]);
    assert_eq!(off.runs[0], [drawn(0, 4, sgr::STRONG)]);
    assert_eq!(off.cursor, (1, 0), "column 0 of the row it is on");

    // The cursor on that row shows every hidden cell of it, as nvim's
    // `concealcursor` default does; a marker then draws in nothing.
    press(&mut tui, &stub, Act::Show(view(2, ""))).unwrap();
    let on = frame(&mut tui);
    assert_eq!(on.lines, ["**bold**", "plain"]);
    assert_eq!(on.runs[0], [drawn(2, 6, sgr::STRONG)]);
    assert_eq!(on.cursor, (0, 2), "over the `b` it stands on");
}

#[test]
fn a_driver_that_comes_after_lay_on_is_read() {
    let rig = Rig::new("after");
    rig.file("note", b"abcdef\n");
    let (mut tui, stub, host) = hosted(&rig, Some("note"));
    press(&mut tui, &stub, Act::Show(view(0, ""))).unwrap();
    assert_eq!(frame(&mut tui).runs[0], Vec::new(), "no Plugin serves one");

    let looks = looking(&host, tui.over().unwrap());
    looks.serves(vec![looked(0, 6, sgr::STRONG, 0)]);
    assert_eq!(tui.tick(), Ok(Step::Changed), "a home came under /self");
    assert_eq!(frame(&mut tui).runs[0], [drawn(0, 6, sgr::STRONG)]);
}

#[test]
fn a_failing_driver_costs_one_frame() {
    let rig = Rig::new("failing");
    rig.file("note", b"abcdef\n");
    let (mut tui, stub, host) = hosted(&rig, Some("note"));
    let looks = looking(&host, stub.buffer());
    looks.serves(vec![looked(0, 6, sgr::STRONG, 0)]);
    looks.refuses(Error::Busy);
    debugged(&host);
    press(&mut tui, &stub, Act::Show(view(0, ""))).unwrap();
    assert_eq!(frame(&mut tui).runs[0], Vec::new(), "the frame it refused");
    let said = grumbles(&host);
    assert_eq!(said.len(), 1, "said once, at debug: {said:?}");
    assert_eq!(said[0].0, 7, "a decoration is nobody's emergency");
    let drew = [drawn(0, 6, sgr::STRONG)];
    assert_eq!(frame(&mut tui).runs[0], drew, "and the one after it");
    assert_eq!(grumbles(&host).len(), 1, "and nothing more to say");
}

#[test]
fn a_node_named_looks_that_declares_none_is_passed_over() {
    let rig = Rig::new("undeclared");
    rig.file("note", b"abcdef\n");
    let (mut tui, stub, host) = hosted(&rig, Some("note"));
    // A Plugin whose `looks` node declares nothing is a name and not
    // an api: the tui tells one by the declaration the Kernel checked.
    debugged(&host);
    let plain = loading(&host, "plain-stub", stub.buffer(), Vec::new());
    plain.serves(vec![looked(0, 6, sgr::EMPHASIS, 0)]);
    let looks = looking(&host, stub.buffer());
    looks.serves(vec![looked(0, 6, sgr::STRONG, 0)]);
    press(&mut tui, &stub, Act::Show(view(0, ""))).unwrap();
    assert_eq!(frame(&mut tui).runs[0], [drawn(0, 6, sgr::STRONG)]);
    assert_eq!(plain.asked(), Vec::new(), "it was never asked");
    // Nor opened and asked in vain: a node the walk passes over costs
    // no handle, no call the Kernel refuses and no word in the log.
    assert_eq!(grumbles(&host), Vec::new(), "and nothing was said");
}

#[test]
fn the_viewport_alone_is_asked() {
    let rig = Rig::new("viewport");
    rig.file("note", b"one\ntwo\nthree\nfour\n");
    let (mut tui, stub, host) = hosted(&rig, Some("note"));
    let looks = looking(&host, stub.buffer());
    press(&mut tui, &stub, Act::Show(view(0, ""))).unwrap();
    // Three rows of terminal are two of text: the bytes of those two,
    // and of no line below them.
    tui.frame(80, 3).unwrap();
    assert_eq!(looks.asked(), [(0, 7)]);
}
