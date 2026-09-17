//! The tui on a Kernel with fs and text, beside a second Client, over
//! a stub Plugin.
//!
//! The stub makes the calls a test scripts and records what the tui
//! told it, so each test judges only what the tui owns: the capability
//! cell, the keys it carries, the region's size and the resize, the
//! View laid out over its own copy of the text the View names, the
//! status row, the watch, the swap an `open` asks for, and the code it
//! leaves with. What a key means is the Plugin's, and the nvim corpus
//! in plugins/vim judges that.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use reovim_client::{
    BAR, BLOCK, Client, Ctx, Event, Fault, Handle, Hash, Key, Kind, MATCH,
    Plugin, Rd, Rights, SELECTION, Start, Tree, grid, keys, log, open, process,
};
use reovim_client_tui::{Frame, Step, Table, Tui, Want};
use reovim_rig::{Limits, Rig};
use reovim_text_api::{history, open as text_open, publish, text};

/// What the tui gave `start`, and the Buffer the stub opened for it:
/// that Buffer, the rd the region is on, the name of a file that is
/// not there yet, the directory and the line to begin at.
type Started = (Rd, Option<Rd>, Option<String>, String, Option<u64>);

/// What a scripted Plugin does with the next input.
#[derive(Clone, Debug)]
enum Act {
    /// That View to the region, over the Buffer it shows.
    Show(grid::View),
    /// `text` in place of `len` bytes at `at`, on its Buffer.
    Edit(u64, u64, String),
    /// The head one revision back, then that View.
    Undo(grid::View),
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
    /// What `start` was given, and the Buffer it opened there.
    start: Option<Started>,
    size: Option<grid::Size>,
    /// Its own Buffer, and the handle it holds on it.
    buffer: Option<Rd>,
    h: Option<Handle>,
    /// The rd the Views it shows name: its own Buffer until a second
    /// one takes its place.
    shows: Option<Rd>,
    region: Option<Handle>,
    opening: Option<Handle>,
    ending: Option<Handle>,
    /// Every input, by the api it came from, and every event.
    inputs: Vec<(Hash, Vec<u8>)>,
    events: Vec<(Rd, Kind)>,
    acts: VecDeque<Act>,
}

/// A Plugin of scripted calls. The handle is shared, so a test writes
/// the script and reads what was recorded while the tui holds the
/// Plugin itself.
#[derive(Clone, Default)]
struct Stub(Rc<RefCell<Script>>);

impl Stub {
    /// What it does with the next input.
    fn will(&self, act: Act) {
        self.0.borrow_mut().acts.push_back(act);
    }

    /// The keys it was given, in order.
    fn pressed(&self) -> Vec<Key> {
        self.0
            .borrow()
            .inputs
            .iter()
            .filter(|(from, _)| *from == keys::hash())
            .map(|(_, bytes)| {
                let press: keys::Press = reovim_sdk::decode(bytes).unwrap();
                Key::of(press.key).unwrap()
            })
            .collect()
    }

    /// The sizes it was resized to, in order.
    fn resized(&self) -> Vec<(u64, u64)> {
        self.0
            .borrow()
            .inputs
            .iter()
            .filter(|(from, _)| *from == grid::hash())
            .map(|(_, bytes)| {
                let to: grid::Resized = reovim_sdk::decode(bytes).unwrap();
                (to.cols, to.rows)
            })
            .collect()
    }

    fn events(&self) -> Vec<(Rd, Kind)> {
        self.0.borrow().events.clone()
    }

    /// What `start` was told and what it did there.
    fn started(&self) -> Started {
        self.0.borrow().start.clone().unwrap()
    }

    /// That View over the Buffer the stub shows: a Plugin names the rd
    /// its View is in the bytes of, and the test says the rest.
    fn over(&self, view: &grid::View) -> grid::View {
        let mut view = view.clone();
        view.over = self.0.borrow().shows.map_or(0, |rd| rd.0);
        view
    }

    /// The one act, performed.
    fn act(&self, ctx: Ctx<'_>, act: &Act) -> Result<(), Fault> {
        let Ctx { tree, caps } = ctx;
        let (h, region) = {
            let script = self.0.borrow();
            (script.h.unwrap(), script.region.unwrap())
        };
        match act {
            Act::Show(view) => grid::Call(caps, region).show(self.over(view)),
            Act::Edit(at, len, text) => {
                let edit =
                    text::Edit { at: *at, len: *len, text: text.clone() };
                text::Call(tree, h).replace(edit)
            }
            Act::Undo(view) => {
                history::Call(tree, h).undo()?;
                grid::Call(caps, region).show(self.over(view))
            }
            Act::Save => publish::Call(tree, h).save(),
            Act::SaveAs(name) => {
                let path = format!("/module/fs/root/{name}");
                let target = tree.resolve(&path)?;
                publish::Call(tree, h).save_as(target)
            }
            Act::Make(name) => {
                let root = tree.resolve("/module/fs/root")?;
                let input = reovim_sdk::encode(name);
                tree.call_by_name(root, "create", input).map(|_| ())
            }
            Act::Second(text) => self.second(tree, text),
            Act::Open(target, plugin) => {
                let opening = self.0.borrow().opening.unwrap();
                let ask =
                    open::Ask { target: target.0, plugin: plugin.clone() };
                open::Call(caps, opening).open(ask)
            }
            Act::Back => {
                let opening = self.0.borrow().opening.unwrap();
                open::Call(caps, opening).back()
            }
            Act::Exit(code) => {
                let ending = self.0.borrow().ending.unwrap();
                process::Call(caps, ending).exit(u64::from(*code))
            }
        }
    }

    /// A Buffer of its own over nothing, holding `text`: what a Plugin
    /// drawing something that is not the Client's file makes.
    fn second(&self, tree: &mut Tree, text: &str) -> Result<(), Fault> {
        let buffer = buffer_over(tree, None)?;
        let h = tree.open(buffer, Rights::READ.union(Rights::WRITE))?;
        let edit = text::Edit { at: 0, len: 0, text: String::from(text) };
        text::Call(tree, h).replace(edit)?;
        self.0.borrow_mut().shows = Some(buffer);
        Ok(())
    }
}

impl Plugin for Stub {
    fn start(&mut self, ctx: Ctx<'_>, at: Start) -> Result<(), Fault> {
        let Ctx { tree, caps } = ctx;
        let buffer = buffer_over(tree, at.over)?;
        let rights = Rights::READ.union(Rights::WRITE);
        let h = tree.open(buffer, rights)?;
        let region = caps.open(grid::hash());
        let (opening, ending) =
            (caps.open(open::hash()), caps.open(process::hash()));
        let size = match region {
            Some(region) => Some(grid::Call(caps, region).size()?),
            None => None,
        };
        let script = &mut *self.0.borrow_mut();
        script.start = Some((buffer, at.over, at.name, at.dir, at.line));
        (script.buffer, script.shows) = (Some(buffer), Some(buffer));
        (script.h, script.region, script.size) = (Some(h), region, size);
        (script.opening, script.ending) = (opening, ending);
        Ok(())
    }

    fn input(
        &mut self,
        ctx: Ctx<'_>,
        from: Hash,
        bytes: &[u8],
    ) -> Result<(), Fault> {
        let act = {
            let script = &mut *self.0.borrow_mut();
            script.inputs.push((from, bytes.to_vec()));
            script.acts.pop_front()
        };
        act.map_or(Ok(()), |act| self.act(ctx, &act))
    }

    fn changed(&mut self, _: Ctx<'_>, events: &[Event]) -> Result<(), Fault> {
        let mut script = self.0.borrow_mut();
        script.events.extend(events.iter().map(|e| (e.rd, e.kind)));
        Ok(())
    }
}

/// A Buffer over `source`, or over nothing, as every Plugin that shows
/// text opens its own.
fn buffer_over(tree: &mut Tree, source: Option<Rd>) -> Result<Rd, Fault> {
    let opener = tree.resolve("/module/text/open")?;
    let opening = tree.open(opener, Rights::WRITE)?;
    let mut call = text_open::Call(tree, opening);
    let buffer = match source {
        Some(source) => call.open(source),
        None => call.new(),
    };
    tree.close(opening)?;
    buffer
}

/// A View over the whole text, with a block cursor at that byte. The
/// rd it is in the bytes of is the stub's to fill in.
fn view(cursor: u64, status: &str) -> grid::View {
    grid::View {
        over: 0,
        top: 0,
        cursor,
        shape: BLOCK,
        spans: Vec::new(),
        status: status.into(),
    }
}

/// A styled span of bytes.
const fn span(from: u64, to: u64, style: u64) -> grid::Span {
    grid::Span { from, to, style }
}

/// The table a test gives when one Plugin is all it needs: a stub for
/// whatever the region is asked to show.
fn one() -> Table<Stub> {
    Box::new(|_| Some(Stub::default()))
}

/// A table with a stub for the Wants `yes` says yes to, and no Plugin
/// for the rest.
fn only(yes: fn(Want<'_>) -> bool) -> Table<Stub> {
    Box::new(move |want| yes(want).then(Stub::default))
}

/// One key, `act` what the stub does for it. The key itself is the
/// stub's to ignore: what a key means is the Plugin's.
fn press(tui: &mut Tui<Stub>, act: Act) -> Result<Step, Fault> {
    tui.plugin().will(act);
    tui.key(Key::Char('.'))
}

/// The frame at the size the app always asks for.
fn frame(tui: &mut Tui<Stub>) -> Frame {
    tui.frame(80, 24).unwrap()
}

fn buffer_of(client: &mut Client, tui: &Tui<Stub>) -> Rd {
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
fn the_capability_cell_names_what_the_tui_serves() {
    let rig = Rig::new("capability");
    rig.file("note", b"first\n");
    let tui = Tui::open(rig.client(), "", Some("note"), None, one()).unwrap();
    let mut other = rig.client();
    // The tui was the first to connect: session 1, its one client.
    let clients = other.resolve("/session/1/client").unwrap();
    let first = other.children(clients).unwrap()[0];
    let capability = other.lookup(first, "capability").unwrap();
    let cell = other.read_all(capability).unwrap();
    assert_eq!(cell, b"keys\ngrid\nopen\nprocess\n");
    // And each of those names one the Plugin opened.
    {
        let script = tui.plugin().0.borrow();
        assert!(script.region.is_some() && script.ending.is_some());
        assert!(script.opening.is_some(), "the region is a region");
        assert_eq!(script.size, Some(grid::Size { cols: 80, rows: 23 }));
    }
    let (buffer, over, name, dir, line) = tui.plugin().started();
    assert_eq!((dir, line), (String::new(), None));
    let note = other.resolve("/module/fs/root/note").unwrap();
    assert_eq!(over, Some(note), "the file the region is on");
    assert_eq!(name, None, "a file that is there needs no making");
    let home = other.resolve("/module/text").unwrap();
    assert_eq!(other.parent(buffer), Ok(home), "a Buffer of the Plugin's");
    assert_eq!(tui.over(), None, "nothing laid out until a View names it");
}

#[test]
fn keys_reach_the_plugin_as_presses_and_a_resize_before_the_frame() {
    let rig = Rig::new("presses");
    rig.file("note", b"first\n");
    let mut tui =
        Tui::open(rig.client(), "", Some("note"), None, one()).unwrap();
    for key in [Key::Char('한'), Key::Ctrl('r'), Key::Esc, Key::Tab] {
        tui.plugin().will(Act::Show(view(0, "")));
        assert_eq!(tui.key(key), Ok(Step::Changed), "a show is a draw");
    }
    let keys = [Key::Char('한'), Key::Ctrl('r'), Key::Esc, Key::Tab];
    assert_eq!(tui.plugin().pressed(), keys, "every key, and nothing else");
    assert_eq!(tui.key(Key::Char('x')), Ok(Step::Same), "no show, no draw");

    assert_eq!(tui.plugin().resized(), [], "the first size is not a resize");
    frame(&mut tui);
    assert_eq!(tui.plugin().resized(), [], "nor is the size it started at");
    tui.frame(40, 10).unwrap();
    tui.frame(40, 10).unwrap();
    assert_eq!(tui.plugin().resized(), [(40, 9)], "the status row is not its");
}

#[test]
fn a_view_is_laid_out_over_the_tui_s_own_text() {
    let rig = Rig::new("layout");
    rig.file("note", "a\tb\n한글x\nlast\n".as_bytes());
    let mut tui =
        Tui::open(rig.client(), "", Some("note"), None, one()).unwrap();
    // The cursor on `b`, the second line selected, `x` a match.
    let mut shown = view(2, "-- VISUAL LINE --");
    shown.spans = vec![span(4, 12, SELECTION), span(10, 11, MATCH)];
    assert_eq!(press(&mut tui, Act::Show(shown)), Ok(Step::Changed));
    let frame = frame(&mut tui);
    assert_eq!(frame.lines, ["a       b", "한글x", "last"], "tab to its stop");
    assert_eq!(frame.cursor, (0, 8), "the cell after the tab");
    assert_eq!(frame.selection, [(1, 0, 6)], "through its newline");
    assert_eq!(frame.marks, [(1, 4, 5)], "a wide char is two cells");

    // The top line is the line the top byte is on; a narrow region
    // leaves the rest out.
    let mut shown = view(12, "");
    shown.top = 5;
    assert_eq!(press(&mut tui, Act::Show(shown)), Ok(Step::Changed));
    let frame = tui.frame(4, 3).unwrap();
    assert_eq!(frame.lines, ["한글", "last"], "a wide char is kept whole");
    assert_eq!(frame.cursor, (1, 0));
}

#[test]
fn a_view_over_another_buffer_is_laid_out_over_that_one() {
    let rig = Rig::new("over");
    rig.file("note", b"first\n");
    let mut tui =
        Tui::open(rig.client(), "", Some("note"), None, one()).unwrap();
    assert_eq!(press(&mut tui, Act::Show(view(0, ""))), Ok(Step::Changed));
    let own = tui.over().unwrap();
    assert_eq!(frame(&mut tui).lines, ["first"]);

    // A Buffer of the Plugin's own, over nothing: the View names it,
    // and the tui lays out over that instead.
    let second = Act::Second("listing\nhere\n".into());
    assert_eq!(press(&mut tui, second), Ok(Step::Same), "nothing shown yet");
    let shown = Act::Show(view(8, "-- LIST --"));
    assert_eq!(press(&mut tui, shown), Ok(Step::Changed));
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
    let mut tui =
        Tui::open(rig.client(), "", Some("note"), None, one()).unwrap();
    let shown = view(0, "");
    assert_eq!(press(&mut tui, Act::Show(shown)), Ok(Step::Changed));
    let normal = frame(&mut tui);
    assert_eq!(normal.cursor, (0, 7), "a block is on the tab's last cell");
    assert_eq!(normal.shape, BLOCK, "and the app draws it as one");

    let mut shown = view(0, "-- INSERT --");
    shown.shape = BAR;
    assert_eq!(press(&mut tui, Act::Show(shown)), Ok(Step::Changed));
    let inserting = frame(&mut tui);
    assert_eq!(inserting.cursor, (0, 0), "a bar stands before the tab");
    assert_eq!(inserting.shape, BAR);
}

#[test]
fn the_status_row_is_the_tui_s_own() {
    let rig = Rig::new("status");
    rig.file("note", "한글\nx\n".as_bytes());
    let mut tui =
        Tui::open(rig.client(), "", Some("note"), None, one()).unwrap();
    // Nothing is laid out before the first View, so there are no
    // revisions to name; the region is the file it was opened on.
    assert_eq!(frame(&mut tui).status, "note  1:1  r0   ", "before a show");
    let shown = Act::Show(view(8, "-- INSERT --"));
    assert_eq!(press(&mut tui, shown), Ok(Step::Changed));
    let status = frame(&mut tui).status;
    assert_eq!(status, "note  2:2  r1   -- INSERT --", "the cursor in chars");
    assert_eq!(press(&mut tui, Act::Edit(0, 0, "y".into())), Ok(Step::Changed));
    assert_eq!(press(&mut tui, Act::Show(view(9, ""))), Ok(Step::Changed));
    assert_eq!(frame(&mut tui).status, "note  2:2  r2[+]   ", "unsaved");
    assert_eq!(press(&mut tui, Act::Save), Ok(Step::Changed));
    assert_eq!(frame(&mut tui).status, "note  2:2  r2   ", "written");
    assert_eq!(rig.disk("note"), "y한글\nx\n".as_bytes());
}

#[test]
fn a_plugin_that_never_shows_draws_an_empty_frame() {
    let rig = Rig::new("quiet");
    rig.file("note", b"first\n");
    let mut tui =
        Tui::open(rig.client(), "", Some("note"), None, one()).unwrap();
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
    let mut tui =
        Tui::open(rig.client(), "", Some("note"), None, one()).unwrap();
    assert_eq!(press(&mut tui, Act::Show(view(0, ""))), Ok(Step::Changed));
    assert_eq!(press(&mut tui, Act::Exit(3)), Ok(Step::Quit(3)));
    assert_eq!(tui.tick(), Ok(Step::Quit(3)), "and it stays gone");
}

#[test]
fn keys_change_the_draft_and_save_reaches_disk() {
    let rig = Rig::new("keys");
    rig.file("note", b"first\n");
    let mut tui =
        Tui::open(rig.client(), "", Some("note"), None, one()).unwrap();
    assert_eq!(press(&mut tui, Act::Show(view(0, ""))), Ok(Step::Changed));
    let mut other = rig.client();
    let buffer = buffer_of(&mut other, &tui);

    assert_eq!(press(&mut tui, Act::Edit(0, 0, "x".into())), Ok(Step::Changed));
    assert_eq!(other.read_all(buffer).unwrap(), b"xfirst\n");
    assert_eq!(rig.disk("note"), b"first\n", "disk waits for save");
    assert_eq!(press(&mut tui, Act::Show(view(1, ""))), Ok(Step::Changed));
    assert_eq!(frame(&mut tui).lines, ["xfirst"], "the tui read it again");
    assert_eq!(tui.tick(), Ok(Step::Same), "its own edits change nothing");
    assert_eq!(press(&mut tui, Act::Save), Ok(Step::Changed));
    assert_eq!(rig.disk("note"), b"xfirst\n");
    assert!(frame(&mut tui).status.starts_with("note  1:2  r2   "));
}

#[test]
fn undo_and_redo_walk_the_revisions() {
    let rig = Rig::new("undo");
    rig.file("note", b"first\n");
    let mut tui =
        Tui::open(rig.client(), "", Some("note"), None, one()).unwrap();
    assert_eq!(press(&mut tui, Act::Edit(0, 0, "x".into())), Ok(Step::Same));
    assert_eq!(press(&mut tui, Act::Show(view(0, ""))), Ok(Step::Changed));
    assert_eq!(frame(&mut tui).lines, ["xfirst"]);
    assert_eq!(press(&mut tui, Act::Undo(view(0, ""))), Ok(Step::Changed));
    let frame = frame(&mut tui);
    assert_eq!(frame.lines, ["first"], "the earlier text, read again");
    assert_eq!(frame.status, "note  1:1  r1   ", "and the revision with it");
}

#[test]
fn a_write_from_another_client_shows_after_tick() {
    let rig = Rig::new("modify");
    rig.file("note", b"first\n");
    let mut tui =
        Tui::open(rig.client(), "", Some("note"), None, one()).unwrap();
    assert_eq!(press(&mut tui, Act::Show(view(0, ""))), Ok(Step::Changed));
    let mut other = rig.client();
    let buffer = buffer_of(&mut other, &tui);

    assert_eq!(tui.tick(), Ok(Step::Same));
    assert_eq!(tui.plugin().events(), [], "nothing happened");
    other.write_all(buffer, b"other\nlines".to_vec()).unwrap();
    assert_eq!(tui.tick(), Ok(Step::Changed));
    assert_eq!(tui.tick(), Ok(Step::Same));
    assert_eq!(tui.plugin().events(), [(buffer, Kind::Modify)], "the Plugin's");
    assert_eq!(press(&mut tui, Act::Show(view(0, ""))), Ok(Step::Changed));
    let frame = frame(&mut tui);
    assert_eq!(frame.lines, ["other", "lines"]);
    assert!(frame.status.contains("[+]"), "{}", frame.status);
}

#[test]
fn a_tui_behind_a_full_ring_reads_everything_again() {
    let rig =
        Rig::with("overflow", Limits { watch_ring: 2, ..Limits::default() });
    rig.file("note", b"first\n");
    let mut tui =
        Tui::open(rig.client(), "", Some("note"), None, one()).unwrap();
    assert_eq!(press(&mut tui, Act::Show(view(0, ""))), Ok(Step::Changed));
    let mut other = rig.client();
    let buffer = buffer_of(&mut other, &tui);
    for text in [b"one".as_slice(), b"two", b"three"] {
        other.write_all(buffer, text.to_vec()).unwrap();
    }
    assert_eq!(tui.tick(), Ok(Step::Changed));
    assert_eq!(press(&mut tui, Act::Show(view(0, ""))), Ok(Step::Changed));
    assert_eq!(frame(&mut tui).lines, ["three"]);
    assert!(frame(&mut tui).status.contains("r4[+]"));
}

#[test]
fn a_missing_file_is_a_region_over_nothing_that_keeps_its_name() {
    let rig = Rig::new("missing");
    let mut tui =
        Tui::open(rig.client(), "", Some("none"), None, one()).unwrap();
    // Over nothing, but named: the Plugin is handed the name to make.
    let (_, over, name, _, _) = tui.plugin().started();
    assert_eq!((over, name), (None, Some("none".to_string())));
    assert_eq!(frame(&mut tui).status, "none  1:1  r0   ");
    assert_eq!(press(&mut tui, Act::Show(view(0, ""))), Ok(Step::Changed));
    assert_eq!(frame(&mut tui).lines, [""], "one empty line");
    assert!(!rig.dir().join("none").exists(), "nothing on disk yet");

    assert_eq!(
        press(&mut tui, Act::Edit(0, 0, "x\n".into())),
        Ok(Step::Changed)
    );
    assert!(!rig.dir().join("none").exists(), "nor after an edit");

    // The Plugin makes the file and its Buffer goes on as that file's:
    // the tui follows the source it took.
    assert_eq!(press(&mut tui, Act::Make("none".into())), Ok(Step::Same));
    assert_eq!(press(&mut tui, Act::SaveAs("none".into())), Ok(Step::Changed));
    assert_eq!(rig.disk("none"), b"x\n");
    assert_eq!(frame(&mut tui).status, "none  1:1  r2   ");
}

#[test]
fn the_tui_names_the_laid_rd_when_it_takes_a_source() {
    let rig = Rig::new("unnamed");
    let mut tui = Tui::open(rig.client(), "", None, None, one()).unwrap();
    assert_eq!(frame(&mut tui).status, "[No Name]  1:1  r0   ");
    let (_, over, name, _, _) = tui.plugin().started();
    assert_eq!((over, name), (None, None), "no rd, no name");
    assert_eq!(press(&mut tui, Act::Show(view(0, ""))), Ok(Step::Changed));
    assert_eq!(
        press(&mut tui, Act::Edit(0, 0, "hi\n".into())),
        Ok(Step::Changed)
    );
    assert_eq!(frame(&mut tui).status, "[No Name]  1:1  r2[+]   ");
    // `:w NAME` is the Plugin's, and the Buffer it saves goes on as
    // that file's: the tui follows it by name.
    assert_eq!(press(&mut tui, Act::Make("note".into())), Ok(Step::Same));
    let named = Act::SaveAs("note".into());
    assert_eq!(press(&mut tui, named), Ok(Step::Changed));
    assert_eq!(rig.disk("note"), b"hi\n");
    assert_eq!(frame(&mut tui).status, "note  1:1  r2   ");
}

#[test]
fn open_swaps_the_region_to_the_plugin_the_table_gives_the_shape() {
    let rig = Rig::new("open-cell");
    rig.file("note", b"first\n");
    rig.file("other", b"second\n");
    let cells = only(|want| matches!(want, Want::Cell));
    let mut tui =
        Tui::open(rig.client(), "", Some("note"), None, cells).unwrap();
    assert_eq!(press(&mut tui, Act::Show(view(0, ""))), Ok(Step::Changed));
    let mut other = rig.client();
    let first = tui.over().unwrap();
    assert_eq!(open_on(&mut other, first), 2, "the Plugin's and the tui's");
    let target = other.resolve("/module/fs/root/other").unwrap();

    // The ask is acted on once the call that made it has returned.
    let ask = Act::Open(target, String::new());
    assert_eq!(press(&mut tui, ask), Ok(Step::Same), "the new one is quiet");
    let (_, over, name, dir, line) = tui.plugin().started();
    assert_eq!(over, Some(target), "started over the rd asked for");
    assert_eq!((name, dir, line), (None, String::new(), None));
    assert_eq!(open_on(&mut other, first), 1, "the old Plugin's is closed");

    // Its first View is the next frame, and what the tui laid before
    // goes with it.
    assert_eq!(press(&mut tui, Act::Show(view(0, ""))), Ok(Step::Changed));
    assert_ne!(tui.over(), Some(first));
    assert_eq!(open_on(&mut other, first), 0, "nothing is left on it");
    let frame = frame(&mut tui);
    assert_eq!(frame.lines, ["second"], "the file it was asked to show");
    assert_eq!(frame.status, "other  1:1  r1   ");
}

#[test]
fn open_by_name_swaps_to_the_plugin_the_table_knows() {
    let rig = Rig::new("open-named");
    rig.file("note", b"first\n");
    rig.file("other", b"second\n");
    let known = only(|want| matches!(want, Want::Cell | Want::Named("stub")));
    let mut tui =
        Tui::open(rig.client(), "", Some("note"), None, known).unwrap();
    let mut other = rig.client();
    let target = other.resolve("/module/fs/root/other").unwrap();
    let ask = Act::Open(target, "stub".into());
    assert_eq!(press(&mut tui, ask), Ok(Step::Same));
    let (_, over, _, _, _) = tui.plugin().started();
    assert_eq!(over, Some(target), "the Plugin the name stands for");
}

#[test]
fn back_gives_the_region_to_the_plugin_of_the_step_before_it() {
    let rig = Rig::new("back");
    rig.file("note", b"first\n");
    rig.file("other", b"second\n");
    let known = only(|want| matches!(want, Want::Cell | Want::Named("cli")));
    let mut tui =
        Tui::open(rig.client(), "", Some("note"), None, known).unwrap();
    let mut other = rig.client();
    let note = other.resolve("/module/fs/root/note").unwrap();
    let target = other.resolve("/module/fs/root/other").unwrap();
    let ask = Act::Open(target, "cli".into());
    assert_eq!(press(&mut tui, ask), Ok(Step::Same));
    let (_, over, _, _, _) = tui.plugin().started();
    assert_eq!(over, Some(target), "the region went there");

    // The step back is the file the region was over, in a Plugin the
    // table makes afresh by its shape.
    assert_eq!(press(&mut tui, Act::Back), Ok(Step::Same));
    let (_, over, name, dir, line) = tui.plugin().started();
    assert_eq!(over, Some(note), "started over the rd it was over");
    assert_eq!((name, dir, line), (None, String::new(), None));
    assert_eq!(press(&mut tui, Act::Show(view(0, ""))), Ok(Step::Changed));
    assert_eq!(frame(&mut tui).lines, ["first"], "the file is back");

    // One step, and no stack: the step is spent.
    assert_eq!(press(&mut tui, Act::Back), Ok(Step::Quit(0)));
}

#[test]
fn back_with_no_step_leaves_with_nothing_lost() {
    let rig = Rig::new("back-first");
    rig.file("note", b"first\n");
    let mut tui =
        Tui::open(rig.client(), "", Some("note"), None, one()).unwrap();
    // The first Plugin was the first thing shown, so there is nowhere
    // to go: the Client ends, as an exit of 0 would end it.
    assert_eq!(press(&mut tui, Act::Back), Ok(Step::Quit(0)));
    assert_eq!(tui.key(Key::Char('x')), Ok(Step::Quit(0)), "and stays ended");
}

#[test]
fn back_to_a_region_over_nothing_keeps_the_name_it_was_opened_with() {
    let rig = Rig::new("back-unnamed");
    rig.file("other", b"second\n");
    let known = only(|want| matches!(want, Want::Cell | Want::Named("cli")));
    let mut tui =
        Tui::open(rig.client(), "", Some("note"), None, known).unwrap();
    assert_eq!(press(&mut tui, Act::Show(view(0, ""))), Ok(Step::Changed));
    let mut other = rig.client();
    let there = other.resolve("/module/fs/root/other").unwrap();
    let ask = Act::Open(there, "cli".into());
    assert_eq!(press(&mut tui, ask), Ok(Step::Same));
    assert_eq!(press(&mut tui, Act::Back), Ok(Step::Same));
    let (_, over, name, _, _) = tui.plugin().started();
    assert_eq!(over, None, "there was no such file, and still is not");
    assert_eq!(name, Some(String::from("note")), "the name a write makes");
    assert_eq!(press(&mut tui, Act::Show(view(0, ""))), Ok(Step::Changed));
    assert_eq!(frame(&mut tui).status, "note  1:1  r1   ");
}

#[test]
fn an_open_the_table_has_no_plugin_for_leaves_the_region_and_warns() {
    let rig = Rig::new("open-none");
    rig.file("note", b"first\n");
    let cells = only(|want| matches!(want, Want::Cell));
    let mut tui =
        Tui::open(rig.client(), "", Some("note"), None, cells).unwrap();
    assert_eq!(press(&mut tui, Act::Show(view(0, ""))), Ok(Step::Changed));
    let laid = tui.over().unwrap();
    let mut other = rig.client();
    let note = other.resolve("/module/fs/root/note").unwrap();
    let root = other.resolve("/module/fs/root").unwrap();

    // A name the table lacks, then a shape it has no Plugin for.
    let unknown = Act::Open(note, "explorer".into());
    assert_eq!(press(&mut tui, unknown), Ok(Step::Same));
    assert_eq!(press(&mut tui, Act::Open(root, String::new())), Ok(Step::Same));
    assert_eq!(tui.over(), Some(laid), "the region is where it was");
    assert_eq!(frame(&mut tui).lines, ["first"]);

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
fn the_plugin_is_one_of_several_a_client_could_hold() {
    let plugins: Vec<Box<dyn Plugin>> = vec![Box::new(Stub::default())];
    assert_eq!(plugins.len(), 1, "`Plugin` is dyn-compatible");
}

#[test]
fn a_boxed_plugin_is_the_plugin_an_app_s_table_gives() {
    let rig = Rig::new("boxed");
    rig.file("note", b"first\n");
    let table: Table<Box<dyn Plugin>> =
        Box::new(|_| Some(Box::new(Stub::default()) as Box<dyn Plugin>));
    let mut tui =
        Tui::open(rig.client(), "", Some("note"), None, table).unwrap();
    assert_eq!(tui.key(Key::Char('x')), Ok(Step::Same), "a Box is a Plugin");
    assert_eq!(tui.tick(), Ok(Step::Same));
}

#[test]
fn the_line_of_the_command_line_is_handed_to_the_plugin() {
    let rig = Rig::new("plus");
    rig.file("note", b"one\ntwo\nthree\n");
    let open = |line| {
        let tui = Tui::open(rig.client(), "d", Some("note"), line, one());
        let (_, _, _, dir, line) = tui.unwrap().plugin().started();
        (dir, line)
    };
    assert_eq!(open(None), ("d".into(), None), "the dir, and no line");
    assert_eq!(open(Some(1)), ("d".into(), Some(1)));
    // Only whoever has the text can find the byte, so the line is
    // handed over as it was given: plugins/vim judges where it lands.
    assert_eq!(open(Some(9)), ("d".into(), Some(9)));
}

#[test]
fn the_first_plugin_is_asked_for_by_the_target_s_shape() {
    let rig = Rig::new("first-shape");
    rig.file("note", b"first\n");
    std::fs::create_dir(rig.dir().join("sub")).unwrap();
    let wants: Rc<RefCell<Vec<String>>> = Rc::default();
    // A table with no Plugin at all, which says what it was asked
    // for: `reovim DIR` is a region over a Container, and the tui
    // knows no Plugin for either shape.
    let asked = |file| {
        let seen = wants.clone();
        let table: Table<Stub> = Box::new(move |want| {
            seen.borrow_mut().push(format!("{want:?}"));
            None
        });
        Tui::open(rig.client(), "", file, None, table).err()
    };
    let refused =
        |shape: &str| Some(Fault::Usage(format!("no Plugin for {shape}")));
    assert_eq!(asked(Some("sub")), refused("a Container"));
    assert_eq!(asked(Some("note")), refused("a cell"));
    assert_eq!(asked(None), refused("a cell"), "nothing is a cell");
    assert_eq!(*wants.borrow(), ["Container", "Cell", "Cell"]);
}
