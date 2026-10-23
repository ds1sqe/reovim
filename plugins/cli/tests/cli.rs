//! The cli buffer on a Kernel with fs and text, shown through the tui.
//!
//! The table gives the cli buffer to the name `cli` and the rig's
//! recording stub to a cell, so a region that goes into the buffer and
//! comes back out of it is judged by what that stub was started over.
//! What a typed line does is clients/sh's and is judged there; what
//! these tests judge is the buffer: the prompt, the transcript, the
//! line, and the way out.

use reovim_client::dev::{Clip, Table, Want};
use reovim_client_api::{BLUE, GREEN, Key, display};
use reovim_client_region::Region;
use reovim_client_tui::{Step, Tui};
use reovim_kernel_interface::{Calls, Error, Rd};
use reovim_rig::{Rig, Seen, Stub, frame, opened, pressed};
use reovim_sdk::{ops, sgr};

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
    opened(&rig.host(), table, Clip::none(), "", file, None).unwrap()
}

/// The region on `work/a` in the stub, then the key that hands it to
/// the cli buffer over `work`.
fn buffer(rig: &Rig, seen: &Seen) -> Tui {
    let mut tui = editor(rig, table(seen), Some("work/a"));
    assert_eq!(tui.key(Key::Char('x')), Ok(Step::Changed), "the cli shows");
    tui
}

/// That line typed and submitted.
fn line(tui: &mut Tui, text: &str) -> Step {
    let mut typed: Vec<Key> = text.chars().map(Key::Char).collect();
    typed.push(Key::Enter);
    pressed(tui, &typed)
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
fn an_escape_a_batch_ends_inside_waits_for_the_batch_after_it() {
    let rig = work("split");
    // A target whose bytes end in the middle of an escape, which is
    // what a Driver writing one across two batches looks like.
    rig.file("work/esc", b"one\x1b");
    let seen = Seen::default();
    let mut tui = buffer(&rig, &seen);
    assert_eq!(line(&mut tui, "cat -f esc"), Step::Changed);
    let at = prompt("work");
    let shown = [format!("{at}cat -f esc"), "one".into(), at];
    assert_eq!(frame(&mut tui).lines, shown, "no half of an escape is text");

    // The batch after it begins with what was held back, so the escape
    // is read whole and what it paints is a span.
    let mut other = rig.client();
    let esc = other.resolve("/module/fs/root/work/esc").unwrap();
    other.write_all(esc, b"one\x1b[32mtwo\x1b[0m\n".to_vec()).unwrap();
    assert_eq!(tui.tick(), Ok(Step::Changed), "no key was pressed");
    let frame = frame(&mut tui);
    assert_eq!(frame.lines[2], "onetwo", "the escapes are out of the text");
    let run = display::Run { from: 3, to: 6, style: sgr::look(GREEN) };
    assert_eq!(frame.runs[2], [run], "where the run stood");
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
    let at = u64::try_from("a  b  ".len()).unwrap();
    let run = display::Run { from: at, to: at + 3, style: sgr::look(BLUE) };
    let one = core::slice::from_ref(&run);
    assert_eq!(listed.runs[1].as_slice(), one, "where it stood");

    // Every name of the tree's own root is a Container: each is a run
    // of its own on the one row.
    line(&mut tui, "ls /");
    let root = frame(&mut tui);
    assert_eq!(root.lines[3], "bin  module  session  sys  var");
    let names = ["bin", "module", "session", "sys", "var"];
    let mut runs = vec![run];
    let mut from = 0u64;
    for name in names {
        let wide = u64::try_from(name.len()).unwrap();
        let style = sgr::look(BLUE);
        runs.push(display::Run { from, to: from + wide, style });
        from += wide + 2;
    }
    let flat: Vec<display::Run> = root.runs.iter().flatten().cloned().collect();
    assert_eq!(flat, runs, "five runs on the one row, one on the other");
}

#[test]
fn tab_completes_a_name_from_where_the_line_stands() {
    let rig = work("tab");
    let seen = Seen::default();
    let mut tui = buffer(&rig, &seen);
    pressed(&mut tui, &[Key::Char('c'), Key::Char('d'), Key::Char(' ')]);
    pressed(&mut tui, &[Key::Char('s'), Key::Tab]);
    let at = prompt("work");
    // A Container keeps the `/` and takes no space, so the next Tab
    // would go on under it.
    assert_eq!(frame(&mut tui).lines, [format!("{at}cd sub/")]);
    pressed(&mut tui, &[Key::Enter]);
    assert_eq!(frame(&mut tui).lines[1], prompt("work/sub"));
}

#[test]
fn tab_lists_several_under_the_line() {
    let rig = work("several");
    let seen = Seen::default();
    let mut tui = buffer(&rig, &seen);
    let at = prompt("work");

    // Tab on an empty line: every name PATH and the builtins hold,
    // each its own row as the process cli's `offer` writes them,
    // above a fresh prompt; the line has no common prefix across them
    // all and comes back empty. Twenty rows fit under the twenty-three
    // the region shows, so nothing has scrolled off yet.
    pressed(&mut tui, &[Key::Tab]);
    let all = [
        "load", "unload", "stat", "ls", "tree", "cat", "grep", "log", "dmesg",
        "call", "watch", "date", "uptime", "capture", "keys", "cd", "pwd",
        "help",
    ];
    let mut shown = vec![at.clone(), "source".into()];
    shown.extend(all.iter().map(|name| String::from(*name)));
    shown.push(at.clone());
    assert_eq!(frame(&mut tui).lines, shown, "nothing has scrolled off yet");

    // `c` then Tab: the names that begin with it, each its own row in
    // the order `complete` gives them, above a fresh prompt holding
    // their common prefix, `c` itself again. Six more rows scroll the
    // transcript, so only the tail is compared.
    pressed(&mut tui, &[Key::Char('c'), Key::Tab]);
    let cs = ["cat", "call", "capture", "cd"];
    let mut tail = vec![format!("{at}c")];
    tail.extend(cs.iter().map(|name| String::from(*name)));
    tail.push(format!("{at}c"));
    let lines = frame(&mut tui).lines;
    assert_eq!(lines[lines.len() - tail.len()..], tail);

    // `d` then Tab: `cd` is the one candidate, which completes outright
    // and lists nothing, the row before it unchanged.
    pressed(&mut tui, &[Key::Char('d'), Key::Tab]);
    *tail.last_mut().unwrap() = format!("{at}cd ");
    let lines = frame(&mut tui).lines;
    assert_eq!(lines[lines.len() - tail.len()..], tail);
}

#[test]
fn ctrl_l_empties_what_was_run_and_keeps_the_line() {
    let rig = work("clear");
    let seen = Seen::default();
    let mut tui = buffer(&rig, &seen);
    line(&mut tui, "ls");
    pressed(&mut tui, &[Key::Char('p'), Key::Char('w')]);
    assert_eq!(pressed(&mut tui, &[Key::Ctrl('l')]), Step::Changed);
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
    let step = pressed(&mut tui, &[Key::Ctrl('d')]);
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
    assert_eq!(pressed(&mut tui, &[Key::Ctrl('d')]), Step::Quit(0));
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

#[test]
fn source_calls_a_declared_api_and_prints_each_value() {
    let rig = work("fipe-source");
    rig.file(
        "work/new.fipe",
        concat!(
            "use text.open\n",
            "text.open.new (text.open.at \"/module/text/open\")\n",
            "\"finished\"\n",
        )
        .as_bytes(),
    );
    let mut tui = buffer(&rig, &Seen::default());
    line(&mut tui, "source new.fipe");
    let shown = frame(&mut tui).lines.join("\n");
    assert!(shown.contains("\"finished\""), "{shown}");
    assert!(!shown.contains("invalid"), "{shown}");
}

#[test]
fn a_fipe_type_error_runs_nothing() {
    let rig = work("fipe-type");
    rig.file(
        "work/bad.fipe",
        concat!(
            "use sdk.env\n",
            "sdk.env.set (sdk.env.at \"/self/cli/env\") ",
            "-key \"effect\" -value \"changed\"\n",
            "1 + True\n",
        )
        .as_bytes(),
    );
    let host = rig.host();
    let mut tui = opened(
        &host,
        table(&Seen::default()),
        Clip::none(),
        "",
        Some("work/a"),
        None,
    )
    .unwrap();
    tui.key(Key::Char('x')).unwrap();
    line(&mut tui, "source bad.fipe");
    let shown = frame(&mut tui).lines.join("\n");
    assert!(shown.contains("bad.fipe:3:"), "{shown}");
    let ctx = host.own();
    let value = reovim_sdk::env::get(
        &ctx,
        reovim_sdk::cell::resolve(&ctx, reovim_sdk::walk::ROOT, "/self/cli")
            .unwrap(),
        "effect",
    )
    .unwrap();
    assert_eq!(value, None, "the first line had no effect");
}

#[test]
fn a_loaded_public_function_is_a_shell_command() {
    let rig = work("fipe-load");
    rig.file(
        "work/hello.fipe",
        concat!(
            "pub greet: String -> String = name => name\n",
            "pub pick: String -> String -> String = left right => right\n",
        )
        .as_bytes(),
    );
    let mut tui = buffer(&rig, &Seen::default());
    line(&mut tui, "load -l hello.fipe");
    line(&mut tui, "help greet");
    line(&mut tui, "greet --name reovim");
    line(&mut tui, "greet positional");
    line(&mut tui, "pick left right");
    let shown = frame(&mut tui).lines.join("\n");
    assert!(shown.contains("greet [name]"), "{shown}");
    assert!(shown.lines().any(|s| s == "reovim"), "{shown}");
    assert!(shown.lines().any(|s| s == "positional"), "{shown}");
    assert!(shown.lines().any(|s| s == "right"), "{shown}");
}

#[test]
fn load_refuses_a_polymorphic_public_function() {
    let rig = work("fipe-unlabeled");
    rig.file("work/unlabeled.fipe", b"pub greet = name => name\n");
    let mut tui = buffer(&rig, &Seen::default());
    line(&mut tui, "load -l unlabeled.fipe");
    let shown = frame(&mut tui).lines.join("\n");
    assert!(shown.contains("unmappable"), "{shown}");
}

#[test]
fn a_fipe_budget_fault_is_logged_and_the_next_line_runs() {
    let rig = work("fipe-budget");
    rig.file(
        "work/loop.fipe",
        concat!(
            "use text.open\n",
            "h = text.open.at \"/module/text/open\"\n",
            "loop: I64 -> I64 = n => loop (n + 1)\n",
            "loop 0\n",
        )
        .as_bytes(),
    );
    let mut tui = buffer(&rig, &Seen::default());
    line(&mut tui, "source loop.fipe");
    line(&mut tui, "pwd");
    let shown = frame(&mut tui).lines.join("\n");
    assert!(shown.contains("budget is exhausted"), "{shown}");
    assert!(shown.lines().any(|s| s == "/module/fs/root/work"), "{shown}");
    let mut client = rig.client();
    let log = client.resolve("/sys/log").unwrap();
    let log = reovim_sdk::log::parse(&client.read_all(log).unwrap());
    assert!(
        log.iter().any(|entry| {
            let text = entry.text();
            text.contains("fipe: ") && text.contains("budget")
        }),
        "{log:?}"
    );
}

#[test]
fn a_defaulted_parameter_is_exposed_by_its_definition_name() {
    let rig = work("fipe-default");
    rig.file(
        "work/defaults.fipe",
        b"pub depth: I64 -> I64 = (depth = 1) => depth\n",
    );
    let mut tui = buffer(&rig, &Seen::default());
    line(&mut tui, "load -l defaults.fipe");
    line(&mut tui, "depth --depth 7");
    let shown = frame(&mut tui).lines.join("\n");
    assert!(shown.lines().any(|s| s == "7"), "{shown}");
}

#[test]
fn an_exported_fault_is_printed_and_budget_is_read_for_each_run() {
    let rig = work("fipe-export-budget");
    rig.file("work/spin.fipe", b"pub spin: I64 -> I64 = n => spin (n + 1)\n");
    rig.file("work/budget", b"1000");
    let mut tui = buffer(&rig, &Seen::default());
    line(&mut tui, "load -l spin.fipe");
    line(&mut tui, "cat budget > /self/cli/budget");
    line(&mut tui, "spin -n 0");
    line(&mut tui, "pwd");
    let shown = frame(&mut tui).lines.join("\n");
    assert!(shown.contains("budget is exhausted"), "{shown}");
    assert!(shown.lines().any(|s| s == "/module/fs/root/work"), "{shown}");
}
