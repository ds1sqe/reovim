//! The tui on a Kernel with fs and text, beside a second Client, over
//! a stub Plugin.
//!
//! The stub makes the calls a test scripts and records what the tui
//! told it, so each test judges only what the tui owns: the capability
//! cell, the keys it carries, the region's size and the resize, the
//! View laid out over its own copy of the text, the status row, the
//! watch, and the code it leaves with. What a key means is the
//! Plugin's, and the nvim corpus in plugins/vim judges that.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use reovim_client::{
    Client, Ctx, Event, Fault, Handle, Hash, Key, Kind, MATCH, Plugin, Rd,
    Rights, SELECTION, Start, grid, keys, process,
};
use reovim_client_tui::{Frame, Step, Tui};
use reovim_rig::{Limits, Rig};
use reovim_text_api::{history, publish, text};

/// What a scripted Plugin does with the next input.
#[derive(Clone, Debug)]
enum Act {
    /// That View to the region.
    Show(grid::View),
    /// `text` in place of `len` bytes at `at`, on the Buffer.
    Edit(u64, u64, String),
    /// The head one revision back, then that View.
    Undo(grid::View),
    /// The head published to its source.
    Save,
    /// The draft written to that file, which names a Buffer over
    /// nothing.
    SaveAs(String),
    /// Leave with that code.
    Exit(u8),
}

/// What the stub was told and what it is to do next.
#[derive(Default)]
struct Script {
    /// What `start` was given, and what it found there.
    start: Option<(Rd, Option<Rd>, String, u64)>,
    size: Option<grid::Size>,
    h: Option<Handle>,
    region: Option<Handle>,
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

    /// The one act, performed.
    fn act(&self, ctx: Ctx<'_>, act: &Act) -> Result<(), Fault> {
        let Ctx { tree, caps } = ctx;
        let (h, region) = {
            let script = self.0.borrow();
            (script.h.unwrap(), script.region.unwrap())
        };
        match act {
            Act::Show(view) => grid::Call(caps, region).show(view.clone()),
            Act::Edit(at, len, text) => {
                let edit =
                    text::Edit { at: *at, len: *len, text: text.clone() };
                text::Call(tree, h).replace(edit)
            }
            Act::Undo(view) => {
                history::Call(tree, h).undo()?;
                grid::Call(caps, region).show(view.clone())
            }
            Act::Save => publish::Call(tree, h).save(),
            Act::SaveAs(name) => {
                let path = format!("/module/fs/root/{name}");
                let target = tree.resolve(&path)?;
                publish::Call(tree, h).save_as(target)
            }
            Act::Exit(code) => {
                let ending = self.0.borrow().ending.unwrap();
                process::Call(caps, ending).exit(u64::from(*code))
            }
        }
    }
}

impl Plugin for Stub {
    fn start(&mut self, ctx: Ctx<'_>, at: Start) -> Result<(), Fault> {
        let Ctx { tree, caps } = ctx;
        let rights = Rights::READ.union(Rights::WRITE);
        let h = tree.open(at.buffer, rights)?;
        let region = caps.open(grid::hash());
        let ending = caps.open(process::hash());
        let size = match region {
            Some(region) => Some(grid::Call(caps, region).size()?),
            None => None,
        };
        let script = &mut *self.0.borrow_mut();
        script.start = Some((at.buffer, at.source, at.dir, at.at));
        (script.h, script.region, script.size) = (Some(h), region, size);
        script.ending = ending;
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

/// A View over the whole text, with the cursor at that byte.
fn view(cursor: u64, status: &str) -> grid::View {
    grid::View { top: 0, cursor, spans: Vec::new(), status: status.into() }
}

/// A styled span of bytes.
const fn span(from: u64, to: u64, style: u64) -> grid::Span {
    grid::Span { from, to, style }
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
    let buffer = tui.buffer();
    let name = client.name(buffer).unwrap();
    assert_eq!(client.resolve(&format!("/module/text/{name}")), Ok(buffer));
    buffer
}

#[test]
fn the_capability_cell_names_what_the_tui_serves() {
    let rig = Rig::new("capability");
    rig.file("note", b"first\n");
    let tui = Tui::open(rig.client(), "", Some("note"), None, Stub::default())
        .unwrap();
    let mut other = rig.client();
    // The tui was the first to connect: session 1, its one client.
    let clients = other.resolve("/session/1/client").unwrap();
    let first = other.children(clients).unwrap()[0];
    let capability = other.lookup(first, "capability").unwrap();
    let cell = other.read_all(capability).unwrap();
    assert_eq!(cell, b"keys\ngrid\nprocess\n");
    // And each of those names one the Plugin opened.
    let script = tui.plugin().0.borrow();
    assert!(script.region.is_some() && script.ending.is_some());
    assert_eq!(script.size, Some(grid::Size { cols: 80, rows: 23 }));
    let (buffer, source, dir, at) = script.start.clone().unwrap();
    assert_eq!((buffer, dir, at), (tui.buffer(), String::new(), 0));
    assert!(source.is_some(), "the file the Buffer is over");
}

#[test]
fn keys_reach_the_plugin_as_presses_and_a_resize_before_the_frame() {
    let rig = Rig::new("presses");
    rig.file("note", b"first\n");
    let mut tui =
        Tui::open(rig.client(), "", Some("note"), None, Stub::default())
            .unwrap();
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
        Tui::open(rig.client(), "", Some("note"), None, Stub::default())
            .unwrap();
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
fn the_status_row_is_the_tui_s_own() {
    let rig = Rig::new("status");
    rig.file("note", "한글\nx\n".as_bytes());
    let mut tui =
        Tui::open(rig.client(), "", Some("note"), None, Stub::default())
            .unwrap();
    assert_eq!(frame(&mut tui).status, "note  1:1  r1   ", "before a show");
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
        Tui::open(rig.client(), "", Some("note"), None, Stub::default())
            .unwrap();
    let frame = frame(&mut tui);
    assert_eq!(frame.lines, Vec::<String>::new(), "nothing to draw");
    assert_eq!(frame.cursor, (0, 0));
    assert_eq!(frame.status, "note  1:1  r1   ", "the status row is the tui's");
    assert_eq!(tui.key(Key::Char('x')), Ok(Step::Same));
}

#[test]
fn the_exit_code_is_the_plugin_s() {
    let rig = Rig::new("exit");
    rig.file("note", b"first\n");
    let mut tui =
        Tui::open(rig.client(), "", Some("note"), None, Stub::default())
            .unwrap();
    assert_eq!(press(&mut tui, Act::Show(view(0, ""))), Ok(Step::Changed));
    assert_eq!(press(&mut tui, Act::Exit(3)), Ok(Step::Quit(3)));
    assert_eq!(tui.tick(), Ok(Step::Quit(3)), "and it stays gone");
}

#[test]
fn keys_change_the_draft_and_save_reaches_disk() {
    let rig = Rig::new("keys");
    rig.file("note", b"first\n");
    let mut tui =
        Tui::open(rig.client(), "", Some("note"), None, Stub::default())
            .unwrap();
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
        Tui::open(rig.client(), "", Some("note"), None, Stub::default())
            .unwrap();
    assert_eq!(press(&mut tui, Act::Edit(0, 0, "x".into())), Ok(Step::Changed));
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
        Tui::open(rig.client(), "", Some("note"), None, Stub::default())
            .unwrap();
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
        Tui::open(rig.client(), "", Some("note"), None, Stub::default())
            .unwrap();
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
fn a_missing_file_opens_empty_and_appears_on_save() {
    let rig = Rig::new("missing");
    let mut tui =
        Tui::open(rig.client(), "", Some("none"), None, Stub::default())
            .unwrap();
    assert_eq!(press(&mut tui, Act::Show(view(0, ""))), Ok(Step::Changed));
    assert_eq!(frame(&mut tui).lines, [""], "one empty line");
    assert_eq!(
        press(&mut tui, Act::Edit(0, 0, "x\n".into())),
        Ok(Step::Changed)
    );
    assert!(!rig.dir().join("none").exists(), "nothing on disk yet");
    assert_eq!(press(&mut tui, Act::Save), Ok(Step::Changed));
    assert_eq!(rig.disk("none"), b"x\n");
}

#[test]
fn the_tui_names_its_buffer_when_that_buffer_takes_a_source() {
    let rig = Rig::new("unnamed");
    let mut tui =
        Tui::open(rig.client(), "", None, None, Stub::default()).unwrap();
    assert_eq!(frame(&mut tui).status, "[No Name]  1:1  r1   ");
    assert_eq!(tui.plugin().0.borrow().start.clone().unwrap().1, None);
    assert_eq!(
        press(&mut tui, Act::Edit(0, 0, "hi\n".into())),
        Ok(Step::Changed)
    );
    assert_eq!(frame(&mut tui).status, "[No Name]  1:1  r2[+]   ");
    // `:w NAME` is the Plugin's, and the Buffer it saves goes on as
    // that file's: the tui follows it by name.
    let named = Act::SaveAs("note".into());
    assert_eq!(press(&mut tui, named), Ok(Step::Changed));
    assert_eq!(rig.disk("note"), b"hi\n");
    assert_eq!(frame(&mut tui).status, "note  1:1  r2   ");
}

#[test]
fn the_plugin_is_one_of_several_a_client_could_hold() {
    let plugins: Vec<Box<dyn Plugin>> = vec![Box::new(Stub::default())];
    assert_eq!(plugins.len(), 1, "`Plugin` is dyn-compatible");
}

#[test]
fn the_line_of_the_command_line_is_where_the_plugin_begins() {
    let rig = Rig::new("plus");
    rig.file("note", b"one\ntwo\nthree\n");
    let open = |line| {
        let tui =
            Tui::open(rig.client(), "d", Some("note"), line, Stub::default());
        let start = tui.unwrap().plugin().0.borrow().start.clone().unwrap();
        (start.2, start.3)
    };
    assert_eq!(open(None), ("d".into(), 0), "the dir, and the first byte");
    assert_eq!(open(Some(1)), ("d".into(), 0));
    assert_eq!(open(Some(3)), ("d".into(), 8));
    assert_eq!(open(Some(9)), ("d".into(), 8), "clamped to the last line");
}
