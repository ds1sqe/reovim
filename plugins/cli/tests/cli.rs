//! The cli buffer on a Kernel with fs and text, shown through the tui.
//!
//! The table gives the cli buffer to the name `cli` and a recording
//! stub to a cell, so a region that goes into the buffer and comes
//! back out of it is judged by what that stub was started over. What a
//! typed line does is clients/sh's and is judged there; what these
//! tests judge is the buffer: the prompt, the transcript, the line,
//! and the way out.

use std::cell::RefCell;
use std::rc::Rc;

use reovim_client::{
    BLOCK, Ctx, Event, Fault, Handle, Hash, Key, Plugin, Rd, Rights, Start,
    grid, open,
};
use reovim_client_tui::{Frame, Step, Table, Tui, Want};
use reovim_plugin_cli::Cli;
use reovim_rig::Rig;
use reovim_text_api::open as text_open;

/// The one editor a test drives, over whichever Plugin the table gives.
type Editor = Tui<Box<dyn Plugin>>;

/// What each start of the table's cell Plugin was over, in order.
type Opened = Rc<RefCell<Vec<Option<Rd>>>>;

/// A cell Plugin that records what it was started over, shows a Buffer
/// over it, and on any key asks for the cli buffer over the directory
/// that file is in, which is what `:cli` asks for.
struct Stub {
    seen: Opened,
    over: Option<Rd>,
    opening: Option<Handle>,
}

impl Plugin for Stub {
    fn start(&mut self, ctx: Ctx<'_>, at: Start) -> Result<(), Fault> {
        let Ctx { tree, caps } = ctx;
        self.seen.borrow_mut().push(at.over);
        self.over = at.over;
        self.opening = caps.open(open::hash());
        let opener = tree.resolve("/module/text/open")?;
        let opening = tree.open(opener, Rights::WRITE)?;
        let mut call = text_open::Call(tree, opening);
        let buffer = match at.over {
            Some(source) => call.open(source)?,
            None => call.new()?,
        };
        tree.close(opening)?;
        let Some(region) = caps.open(grid::hash()) else { return Ok(()) };
        let view = grid::View {
            over: buffer.0,
            top: 0,
            cursor: 0,
            shape: BLOCK,
            spans: Vec::new(),
            status: String::from("-- STUB --"),
        };
        grid::Call(caps, region).show(view)
    }

    fn input(&mut self, ctx: Ctx<'_>, _: Hash, _: &[u8]) -> Result<(), Fault> {
        let Ctx { tree, caps } = ctx;
        let (Some(over), Some(opening)) = (self.over, self.opening) else {
            return Ok(());
        };
        let dir = tree.parent(over)?;
        let ask = open::Ask { target: dir.0, plugin: String::from("cli") };
        open::Call(caps, opening).open(ask)
    }

    fn changed(&mut self, _: Ctx<'_>, _: &[Event]) -> Result<(), Fault> {
        Ok(())
    }
}

/// The app's table in the small: the cli buffer by name, the stub for
/// a cell.
fn table(seen: &Opened) -> Table<Box<dyn Plugin>> {
    let seen = seen.clone();
    Box::new(move |want| match want {
        Want::Named("cli") => Some(Box::new(Cli::default()) as Box<dyn Plugin>),
        Want::Cell => {
            let stub = Stub { seen: seen.clone(), over: None, opening: None };
            Some(Box::new(stub))
        }
        Want::Named(_) | Want::Container => None,
    })
}

/// A rig whose root holds `work`, which holds `a`, `b` and `sub`.
fn work(test: &str) -> Rig {
    let rig = Rig::new(test);
    std::fs::create_dir_all(rig.dir().join("work/sub")).unwrap();
    rig.file("work/b", b"second\n");
    rig.file("work/a", b"first\n");
    rig
}

/// The region on `work/a` in the stub, then the key that hands it to
/// the cli buffer over `work`.
fn buffer(rig: &Rig, seen: &Opened) -> Editor {
    let table = table(seen);
    let mut tui =
        Tui::open(rig.client(), "", Some("work/a"), None, table).unwrap();
    assert_eq!(tui.key(Key::Char('x')), Ok(Step::Changed), "the cli shows");
    tui
}

/// The frame at the size the app always asks for.
fn frame(tui: &mut Editor) -> Frame {
    tui.frame(80, 24).unwrap()
}

/// Those keys, in order, and what the last one did.
fn keys(tui: &mut Editor, keys: &[Key]) -> Step {
    let mut step = Step::Same;
    for key in keys {
        step = tui.key(*key).unwrap();
    }
    step
}

/// That line typed and submitted.
fn line(tui: &mut Editor, text: &str) -> Step {
    let mut typed: Vec<Key> = text.chars().map(Key::Char).collect();
    typed.push(Key::Enter);
    keys(tui, &typed)
}

/// The prompt the line stands after, which is where the Shell stands:
/// the mirror of the directory `under` names, as the shell writes it.
fn prompt(under: &str) -> String {
    format!("/module/fs/root/{under} > ")
}

#[test]
fn the_first_frame_is_the_prompt_where_the_region_was_opened() {
    let rig = work("prompt");
    let seen = Opened::default();
    let mut tui = buffer(&rig, &seen);
    let frame = frame(&mut tui);
    // The Shell stands where the region was opened, and the path is
    // the one the shell itself finds from the rd.
    assert_eq!(frame.lines, [prompt("work")]);
    let column = prompt("work").chars().count();
    assert_eq!(frame.cursor, (0, u16::try_from(column).unwrap()));
    // A Buffer of the buffer's own, one revision, and nothing of its
    // own in the status row.
    assert_eq!(frame.status, format!("work  1:{}  r2[+]   ", column + 1));
}

#[test]
fn a_line_is_run_and_answered_above_the_next_prompt() {
    let rig = work("ls");
    let seen = Opened::default();
    let mut tui = buffer(&rig, &seen);
    assert_eq!(line(&mut tui, "ls"), Step::Changed);
    let at = prompt("work");
    let shown = [format!("{at}ls"), "a  b  sub".into(), at.clone()];
    assert_eq!(frame(&mut tui).lines, shown, "the names above a new prompt");

    // `pwd` is where the line stands, which is the mirror of the
    // directory the region was opened on.
    line(&mut tui, "pwd");
    let said = at.trim_end_matches(" > ").to_string();
    let shown = [
        format!("{at}ls"),
        "a  b  sub".into(),
        format!("{at}pwd"),
        said,
        at.clone(),
    ];
    assert_eq!(frame(&mut tui).lines, shown);

    // An empty line is a new prompt and nothing else.
    line(&mut tui, "");
    let lines = frame(&mut tui).lines;
    assert_eq!(&lines[4..], [at.clone(), at], "the empty line, then one more");
}

#[test]
fn cd_moves_the_prompt_and_what_a_name_is_read_from() {
    let rig = work("cd");
    let seen = Opened::default();
    let mut tui = buffer(&rig, &seen);
    line(&mut tui, "cd sub");
    let (was, now) = (prompt("work"), prompt("work/sub"));
    let shown = [format!("{was}cd sub"), now.clone()];
    assert_eq!(frame(&mut tui).lines, shown, "the line stands there now");
    line(&mut tui, "ls");
    let shown = [format!("{was}cd sub"), format!("{now}ls"), now];
    assert_eq!(frame(&mut tui).lines, shown, "an empty directory");
}

#[test]
fn a_line_that_does_not_run_prints_the_miss_the_shell_prints() {
    let rig = work("miss");
    let seen = Opened::default();
    let mut tui = buffer(&rig, &seen);
    line(&mut tui, "ls /x");
    let at = prompt("work");
    let said = "ls /x: invalid";
    assert_eq!(frame(&mut tui).lines, [format!("{at}ls /x"), said.into(), at]);
}

#[test]
fn a_following_line_prints_what_its_cell_holds_and_closes_it() {
    let rig = work("follow");
    let seen = Opened::default();
    let mut tui = buffer(&rig, &seen);
    // `-f` follows: the buffer reads the cell once, a Plugin having no
    // tick to read it again, and closes it, which ends the follow.
    assert_eq!(line(&mut tui, "cat -f a"), Step::Changed);
    let at = prompt("work");
    let shown = [format!("{at}cat -f a"), "first".into(), at];
    assert_eq!(frame(&mut tui).lines, shown, "what the cell held");
    let mut other = rig.client();
    let cat = other.resolve("/bin/cat").unwrap();
    assert_eq!(other.children(cat), Ok(Vec::new()), "no cell was left");
}

#[test]
fn tab_completes_a_name_from_where_the_line_stands() {
    let rig = work("tab");
    let seen = Opened::default();
    let mut tui = buffer(&rig, &seen);
    keys(&mut tui, &[Key::Char('c'), Key::Char('d'), Key::Char(' ')]);
    keys(&mut tui, &[Key::Char('s'), Key::Tab]);
    let at = prompt("work");
    // A Container keeps the `/` and takes no space, so the next Tab
    // would go on under it.
    assert_eq!(frame(&mut tui).lines, [format!("{at}cd sub/")]);
    keys(&mut tui, &[Key::Enter]);
    assert_eq!(frame(&mut tui).lines[1], prompt("work/sub"));
}

#[test]
fn ctrl_l_empties_what_was_run_and_keeps_the_line() {
    let rig = work("clear");
    let seen = Opened::default();
    let mut tui = buffer(&rig, &seen);
    line(&mut tui, "ls");
    keys(&mut tui, &[Key::Char('p'), Key::Char('w')]);
    assert_eq!(keys(&mut tui, &[Key::Ctrl('l')]), Step::Changed);
    let at = prompt("work");
    assert_eq!(frame(&mut tui).lines, [format!("{at}pw")], "the line stands");
}

#[test]
fn ctrl_d_gives_the_region_back_to_what_it_came_from() {
    let rig = work("back");
    let seen = Opened::default();
    let mut tui = buffer(&rig, &seen);
    let mut other = rig.client();
    let a = other.resolve("/module/fs/root/work/a").unwrap();
    assert_eq!(*seen.borrow(), [Some(a)], "the stub, once");
    let step = keys(&mut tui, &[Key::Ctrl('d')]);
    assert_eq!(step, Step::Changed, "the file is shown again");
    assert_eq!(*seen.borrow(), [Some(a), Some(a)], "over the same rd");
    let frame = frame(&mut tui);
    assert_eq!(frame.lines, ["first"], "what that file holds");
    assert_eq!(frame.status, "a  1:1  r1   -- STUB --");
}

#[test]
fn a_cli_buffer_that_was_the_first_thing_shown_leaves_with_nothing() {
    let rig = work("first");
    // A table that gives the cli buffer the region the editor opened:
    // there is no step back, so the way out is the way out.
    let table: Table<Box<dyn Plugin>> =
        Box::new(|_| Some(Box::new(Cli::default()) as Box<dyn Plugin>));
    let mut tui =
        Tui::open(rig.client(), "", Some("work"), None, table).unwrap();
    assert_eq!(frame(&mut tui).lines, [prompt("work")]);
    assert_eq!(keys(&mut tui, &[Key::Ctrl('d')]), Step::Quit(0));
}

#[test]
fn a_region_over_a_cell_leaves_the_line_at_the_root() {
    let rig = work("cell");
    // `cd` is the shell's own and a file is no Container: the shell
    // refuses it, nothing is said, and the line stands at the root.
    let table: Table<Box<dyn Plugin>> =
        Box::new(|_| Some(Box::new(Cli::default()) as Box<dyn Plugin>));
    let mut tui =
        Tui::open(rig.client(), "", Some("work/a"), None, table).unwrap();
    assert_eq!(frame(&mut tui).lines, ["/ > "]);
    line(&mut tui, "ls");
    let shown = ["/ > ls", "bin  module  session  sys  var", "/ > "];
    assert_eq!(frame(&mut tui).lines, shown, "the tree's own root");
}

#[test]
fn a_transcript_longer_than_the_region_keeps_the_prompt_in_it() {
    let rig = work("scroll");
    let seen = Opened::default();
    let mut tui = buffer(&rig, &seen);
    // Two lines each: the line as it was typed and what it printed.
    for _ in 0..30 {
        line(&mut tui, "pwd");
    }
    let frame = frame(&mut tui);
    assert_eq!(frame.lines.len(), 23, "the rows the region has");
    assert_eq!(frame.lines[22], prompt("work"), "the prompt is the last");
    assert_eq!(frame.cursor.0, 22, "and the cursor is on it");
}
