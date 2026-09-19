//! `capture` and `keys` over the rig: a Client with `/self/dev` of its
//! own, its screen read as a person reads a terminal and its keyboard
//! typed into from outside.
//!
//! The two names are loaded at that Client, so what they resolve is the
//! Client's own `/self/dev`; the same two at the server read a mirrored
//! rd, which clients/core/tests/wire.rs says.

use std::rc::Rc;

use reovim_client::{Calls, Clip, Error, Host, ROOT, Rd, Rights, Table, dev};
use reovim_client_api::{BAR, Key, display, keyboard};
use reovim_module_sysutils::bins::{capture, keys};
use reovim_rig::bins::{ran, said};
use reovim_rig::{Rig, screen};
use reovim_sdk::{cell, decode, encode, look, ops};

/// A Client with no Plugin at all: the cells of `/self/dev` are what
/// these cases judge, and a region is nobody's.
fn none() -> Table {
    Box::new(|_| None)
}

/// A Client with `/self/dev` booted and sysutils loaded of its own, as
/// the app's Client loads it.
fn editor(rig: &Rig) -> Rc<Host> {
    let host = rig.host();
    dev::boot(&host, none(), Clip::none(), "", None, None).unwrap();
    let booted = reovim_module_sysutils::boot("");
    host.load("sysutils", booted.module, booted.drivers).unwrap();
    host
}

/// `capture [-f] TARGET`, as a shell fills the record: from the root.
fn args(f: bool, target: &str) -> capture::Args {
    capture::Args { f, cwd: ROOT, target: target.to_string() }
}

/// The rd of `path`, which must be there.
fn at(ctx: &dyn Calls, path: &str) -> Rd {
    cell::resolve(ctx, ROOT, path).unwrap()
}

/// Where the screen these cases write stands.
const DISPLAY: &str = "/self/dev/display/0/text";

/// `screen` written into that Client's `display`.
fn drew(ctx: &dyn Calls, screen: &display::Screen) {
    ops::put(ctx, at(ctx, DISPLAY), encode(screen)).unwrap();
}

#[test]
fn a_run_prints_the_rows_of_a_screen_and_the_cursor_last() {
    let rig = Rig::new("capture");
    let host = editor(&rig);
    let ctx = host.own();
    let run = at(&ctx, "/self/bin/capture");
    let printed = || said(&ctx, run, &args(false, DISPLAY));

    // One line a row, and the cursor on a line of its own.
    drew(&ctx, &screen(3, &["hi", "", "there"]));
    assert_eq!(printed().as_deref(), Ok("hi\n\nthere\ncursor 0 0 block\n"));

    // The cursor is where it stands and the shape it is drawn in: the
    // byte the record holds turned to the cell its own row's text
    // gives it, a wide char two of them.
    let bar = display::Screen {
        row: 2,
        col: 3,
        shape: BAR,
        ..screen(4, &["", "", "한a", ""])
    };
    drew(&ctx, &bar);
    assert_eq!(printed().as_deref(), Ok("\n\n한a\n\ncursor 2 2 bar\n"));
}

/// `text` a `Line`, its whole span one run of `style`.
fn styled(text: &str, style: u64) -> display::Line {
    let to = u64::try_from(text.len()).unwrap_or(u64::MAX);
    let run = display::Run { from: 0, to, style };
    display::Line { text: text.to_string(), runs: vec![run] }
}

/// A `Screen` of `lines`, plain apart from their own runs.
const fn drawn(rows: u64, lines: Vec<display::Line>) -> display::Screen {
    display::Screen {
        rows,
        row: 0,
        col: 0,
        shape: BAR,
        lines,
        status: String::new(),
    }
}

#[test]
fn a_run_of_each_kind_writes_its_own_sgr() {
    let rig = Rig::new("capture-looks");
    let host = editor(&rig);
    let ctx = host.own();
    let run = at(&ctx, "/self/bin/capture");
    let printed = || said(&ctx, run, &args(false, DISPLAY));

    // No table and no theme: `capture` asks `look::painted` for the
    // bits and nothing else, a palette 0-7, 8-15, past 15, an rgb fg
    // and bg, and bold with inverse together.
    let screen = drawn(
        8,
        vec![
            styled("bin", look::fg(look::palette(4))),
            styled("etc", look::bg(look::palette(12))),
            styled("var", look::fg(look::palette(208))),
            styled("usr", look::fg(look::rgb(1, 2, 3))),
            styled("dev", look::bg(look::rgb(4, 5, 6))),
            styled("opt", look::BOLD | look::INVERSE),
        ],
    );
    drew(&ctx, &screen);
    let out = printed().unwrap();
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "\x1b[34mbin\x1b[0m");
    assert_eq!(lines[1], "\x1b[104metc\x1b[0m");
    assert_eq!(lines[2], "\x1b[38;5;208mvar\x1b[0m");
    assert_eq!(lines[3], "\x1b[38;2;1;2;3musr\x1b[0m");
    assert_eq!(lines[4], "\x1b[48;2;4;5;6mdev\x1b[0m");
    assert_eq!(lines[5], "\x1b[1;7mopt\x1b[0m");
}

#[test]
fn a_look_no_theme_names_still_writes_its_sgr() {
    let rig = Rig::new("capture-unknown-look");
    let host = editor(&rig);
    let ctx = host.own();
    let run = at(&ctx, "/self/bin/capture");
    let printed = || said(&ctx, run, &args(false, DISPLAY));

    // `italic` and `strike` name no row of the theme at all, and
    // `capture` writes their bits regardless: a reader shows what its
    // own colours allow, and no table stands between it and them.
    let unnamed = look::ITALIC | look::STRIKE;
    let screen = drawn(2, vec![styled("hi", unnamed)]);
    drew(&ctx, &screen);
    let out = printed().unwrap();
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "\x1b[3;9mhi\x1b[0m", "no table, no theme, just bits");
}

#[test]
fn a_run_past_the_text_is_clamped_to_one_space() {
    let rig = Rig::new("capture-clamped-run");
    let host = editor(&rig);
    let ctx = host.own();
    let run = at(&ctx, "/self/bin/capture");
    let printed = || said(&ctx, run, &args(false, DISPLAY));

    // `to` one past `text`'s own length is the newline a selection
    // runs into: `painted` clamps it to one space, as the app draws
    // it, and never panics on a byte past the text.
    let past = display::Run { from: 0, to: 3, style: look::INVERSE };
    let line = display::Line { text: "hi".to_string(), runs: vec![past] };
    let screen = drawn(2, vec![line]);
    drew(&ctx, &screen);
    let out = printed().unwrap();
    let lines: Vec<&str> = out.lines().collect();
    let shown = "\x1b[7mhi \x1b[0m";
    assert_eq!(lines[0], shown, "the byte past the text is a space");
}

#[test]
fn a_run_that_goes_backwards_prints_the_line_plain() {
    let rig = Rig::new("capture-backwards-run");
    let host = editor(&rig);
    let ctx = host.own();
    let run = at(&ctx, "/self/bin/capture");
    let printed = || said(&ctx, run, &args(false, DISPLAY));

    // A run whose `to` comes before its own `from` is another
    // Client's mistake, or worse, and not a range to slice on: the
    // line prints plain, the run `look::painted` cannot make sense of
    // skipped whole, and `capture` never panics on it.
    let backwards = display::Run { from: 8, to: 2, style: look::INVERSE };
    let line = display::Line {
        text: "hello world".to_string(),
        runs: vec![backwards],
    };
    let screen = drawn(2, vec![line]);
    drew(&ctx, &screen);
    let out = printed().unwrap();
    let lines: Vec<&str> = out.lines().collect();
    assert_eq!(lines[0], "hello world", "the backwards run is skipped");
}

#[test]
fn bytes_that_do_not_decode_as_a_screen_are_refused() {
    let rig = Rig::new("capture-invalid");
    let host = editor(&rig);
    let ctx = host.own();
    ops::put(&ctx, at(&ctx, DISPLAY), b"not a screen".to_vec()).unwrap();

    // What a cell means is the Client's, and a reader given bytes that
    // are no `Screen` says so rather than drawing whatever it holds.
    let run = at(&ctx, "/self/bin/capture");
    let refused = said(&ctx, run, &args(false, DISPLAY));
    assert_eq!(refused, Err(Error::Invalid));
}

#[test]
fn a_following_run_prints_the_whole_screen_again_for_every_frame() {
    let rig = Rig::new("capture-follow");
    let host = editor(&rig);
    let ctx = host.own();
    drew(&ctx, &screen(1, &["one"]));

    let run = at(&ctx, "/self/bin/capture");
    let cell = ran(&ctx, run, &args(true, DISPLAY)).unwrap();
    let h = ctx.open(cell, Rights::READ).unwrap();
    let first = ops::read(&ctx, h, 0, 64).unwrap();
    assert_eq!(first, b"one\ncursor 0 0 block\n");

    // Nothing between frames: a read at the end with nothing drawn
    // since is `Busy`, an empty one being the end of the cell. A
    // frame that lands answers the whole screen again.
    let end = first.len() as u64;
    assert_eq!(ops::read(&ctx, h, end, 64), Err(Error::Busy));
    drew(&ctx, &screen(1, &["two"]));
    let again = ops::read(&ctx, h, end, 64);
    assert_eq!(again, Ok(b"two\ncursor 0 0 block\n".to_vec()));
    ops::shut(&ctx, &[h]);
}

#[test]
fn a_run_of_keys_types_a_whole_notation_in_one_write() {
    let rig = Rig::new("keys");
    let host = editor(&rig);
    let ctx = host.own();
    let keyboard = at(&ctx, "/self/dev/input/keyboard");
    let h = ctx.open(keyboard, Rights::READ).unwrap();
    let run = at(&ctx, "/self/bin/keys");
    let typing = |notation: &str| keys::Args {
        cwd: ROOT,
        target: "/self/dev/input/keyboard".to_string(),
        notation: notation.to_string(),
    };

    // Seven keys, and a run that says nothing answers nothing.
    assert_eq!(said(&ctx, run, &typing("ihello<Esc>")).as_deref(), Ok(""));
    let mut taken = Vec::new();
    while let Ok(bytes) = ops::read(&ctx, h, 0, 4096) {
        let press: keyboard::Press = decode(&bytes).unwrap();
        taken.extend(Key::of(press.key));
    }
    let typed: Vec<Key> =
        "ihello".chars().map(Key::Char).chain([Key::Esc]).collect();
    assert_eq!(taken, typed);

    // A notation that names nothing is refused, and nothing is typed.
    assert_eq!(said(&ctx, run, &typing("<Nope>")), Err(Error::Invalid));
    assert_eq!(ops::read(&ctx, h, 0, 4096), Err(Error::Busy));
    ops::shut(&ctx, &[h]);
}
