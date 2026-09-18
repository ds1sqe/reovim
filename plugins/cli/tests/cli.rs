//! The cli buffer on a Kernel with fs and text, shown through the tui.
//!
//! The table gives the cli buffer to the name `cli` and the rig's
//! recording stub to a cell, so a region that goes into the buffer and
//! comes back out of it is judged by what that stub was started over.
//! What a typed line does is clients/sh's and is judged there; what
//! these tests judge is the buffer: the prompt, the transcript, the
//! line, and the way out.

use std::rc::Rc;

use reovim_client::dev::{self, Table, Want};
use reovim_client_api::{BLUE, Key, Region};
use reovim_client_tui::{Frame, Step, Tui};
use reovim_kernel_interface::{Calls, Error, Rd};
use reovim_rig::{Rig, Seen, Stub};
use reovim_sdk::ops;

/// What the stub does on any key: the cli buffer asked for over the
/// directory the file it was started over is in, which is what `:cli`
/// asks for.
fn asks(
    ctx: &dyn Calls,
    region: &Region,
    over: Option<Rd>,
) -> Result<(), Error> {
    let Some(over) = over else { return Ok(()) };
    region.opens(ctx, ops::parent(ctx, over)?, "cli")
}

/// The app's table in the small: the cli buffer by name, the stub for
/// a cell. On any key the stub asks for the cli buffer over the
/// directory the file it was started over is in, which is what `:cli`
/// asks for.
fn table(seen: &Seen) -> Table {
    let seen = seen.clone();
    Box::new(move |want| match want {
        Want::Named("cli") => {
            Some((String::from("cli"), reovim_plugin_cli::boot()))
        }
        Want::Cell => {
            let stub = Stub::typing(&seen, "-- STUB --", Box::new(asks));
            Some((String::from("stub"), stub))
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

/// The tui over `rig` with `table`, opened over the name `file`.
fn editor(rig: &Rig, table: Table, file: Option<&str>) -> Tui {
    let host = Rc::new(rig.host());
    let dev = dev::boot(&host, table, "", file, None).unwrap();
    Tui::open(host, dev).unwrap()
}

/// The region on `work/a` in the stub, then the key that hands it to
/// the cli buffer over `work`.
fn buffer(rig: &Rig, seen: &Seen) -> Tui {
    let mut tui = editor(rig, table(seen), Some("work/a"));
    assert_eq!(tui.key(Key::Char('x')), Ok(Step::Changed), "the cli shows");
    tui
}

/// The frame at the size the app always asks for.
fn frame(tui: &mut Tui) -> Frame {
    tui.frame(80, 24).unwrap()
}

/// Those keys, in order, and what the last one did.
fn keys(tui: &mut Tui, keys: &[Key]) -> Step {
    let mut step = Step::Same;
    for key in keys {
        step = tui.key(*key).unwrap();
    }
    step
}

/// That line typed and submitted.
fn line(tui: &mut Tui, text: &str) -> Step {
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
    let seen = Seen::default();
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
    let seen = Seen::default();
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
    let seen = Seen::default();
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
    let seen = Seen::default();
    let mut tui = buffer(&rig, &seen);
    line(&mut tui, "ls /x");
    let at = prompt("work");
    let said = "ls /x: invalid";
    assert_eq!(frame(&mut tui).lines, [format!("{at}ls /x"), said.into(), at]);
}

#[test]
fn a_following_line_goes_on_printing_with_no_key_pressed() {
    let rig = work("follow");
    let seen = Seen::default();
    let mut tui = buffer(&rig, &seen);
    // `-f` follows, and the run comes back open: the buffer keeps it
    // and every frame asks it for what its Driver has since.
    assert_eq!(line(&mut tui, "cat -f a"), Step::Changed);
    let at = prompt("work");
    let shown = [format!("{at}cat -f a"), "first".into(), at.clone()];
    assert_eq!(frame(&mut tui).lines, shown, "what the cell held");
    let mut other = rig.client();
    let cat = other.resolve("/bin/cat").unwrap();
    assert_eq!(other.children(cat).unwrap().len(), 1, "the cell it follows");

    let a = other.resolve("/module/fs/root/work/a").unwrap();
    other.write_all(a, b"first\nsecond\n".to_vec()).unwrap();
    assert_eq!(tui.tick(), Ok(Step::Changed), "no key was pressed");
    let shown = [
        format!("{at}cat -f a"),
        "first".into(),
        "first".into(),
        "second".into(),
        at,
    ];
    assert_eq!(frame(&mut tui).lines, shown, "what the target holds now");
}

#[test]
fn a_listing_leaves_no_escape_in_the_text_and_a_span_per_name() {
    let rig = work("color");
    let seen = Seen::default();
    let mut tui = buffer(&rig, &seen);
    line(&mut tui, "ls");
    let listed = frame(&mut tui);
    assert_eq!(listed.lines[1], "a  b  sub", "the text the Buffer holds");
    // `sub` is the one Container; `a` and `b` offer no api and were
    // written plain.
    let at = u16::try_from("a  b  ".len()).unwrap();
    assert_eq!(listed.styled, [(1, at, at + 3, BLUE)], "where it stood");

    // Every name of the tree's own root is a Container: each is a run
    // of its own on the one row.
    line(&mut tui, "ls /");
    let root = frame(&mut tui);
    assert_eq!(root.lines[3], "bin  module  session  sys  var");
    let names = ["bin", "module", "session", "sys", "var"];
    let mut spans = vec![(1, at, at + 3, BLUE)];
    let mut from = 0u16;
    for name in names {
        let wide = u16::try_from(name.len()).unwrap();
        spans.push((3, from, from + wide, BLUE));
        from += wide + 2;
    }
    assert_eq!(root.styled, spans, "five runs on the one row");
}

#[test]
fn tab_completes_a_name_from_where_the_line_stands() {
    let rig = work("tab");
    let seen = Seen::default();
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
    let seen = Seen::default();
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
    let seen = Seen::default();
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
    let table: Table =
        Box::new(|_| Some((String::from("cli"), reovim_plugin_cli::boot())));
    let mut tui = editor(&rig, table, Some("work"));
    assert_eq!(frame(&mut tui).lines, [prompt("work")]);
    assert_eq!(keys(&mut tui, &[Key::Ctrl('d')]), Step::Quit(0));
}

#[test]
fn a_region_over_a_cell_leaves_the_line_at_the_root() {
    let rig = work("cell");
    // `cd` is the shell's own and a file is no Container: the shell
    // refuses it, nothing is said, and the line stands at the root.
    let table: Table =
        Box::new(|_| Some((String::from("cli"), reovim_plugin_cli::boot())));
    let mut tui = editor(&rig, table, Some("work/a"));
    assert_eq!(frame(&mut tui).lines, ["/ > "]);
    line(&mut tui, "ls");
    let shown = ["/ > ls", "bin  module  session  sys  var", "/ > "];
    assert_eq!(frame(&mut tui).lines, shown, "the tree's own root");
}

#[test]
fn a_transcript_longer_than_the_region_keeps_the_prompt_in_it() {
    let rig = work("scroll");
    let seen = Seen::default();
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
