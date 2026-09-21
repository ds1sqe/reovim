//! A typed line over the rig: the name resolved under `/bin`, the
//! record filled from the tokens, and the output cell read to its end.

use reovim_client_api::{BAR, display};
use reovim_client_sh::{Shell, ran, run};
use reovim_kernel_interface::{
    Calls, Decl, DriverId, Error, Flags, Method, Rd, Rights, Ty,
};
use reovim_rig::Rig;
use reovim_sdk::walk::ROOT;
use reovim_sdk::{cell, encode, look, ops};

mod common;
use common::typed;

/// One typed line from the root, on a screen `width` wide; its output.
fn shown(ctx: &dyn Calls, line: &str, width: Option<u16>) -> String {
    let mut shell = Shell::default();
    shell.screen(u64::from(width.unwrap_or(0)), false);
    typed(ctx, &mut shell, line)
}

/// The output cell of one run of `/bin/uptime`, whose record has no
/// field: answered and never opened, as a stage before a `|` leaves it.
fn celled(ctx: &dyn Calls) -> Rd {
    let rd = cell::resolve(ctx, ROOT, "/bin/uptime").unwrap();
    let out = ops::call_by_name(ctx, rd, "run", Vec::new()).unwrap();
    reovim_sdk::decode(&out).unwrap()
}

/// The cells the run of `name` under `/bin` has open.
fn cells(ctx: &dyn Calls, name: &str) -> Vec<Rd> {
    let rd = cell::resolve(ctx, ROOT, &format!("/bin/{name}")).unwrap();
    ops::children(ctx, rd).unwrap()
}

#[test]
fn a_listing_is_laid_out_across_the_screen_the_shell_gives() {
    let rig = Rig::new("sh-ls");
    for name in ["gamma", "alpha", "beta"] {
        rig.file(name, b"");
    }
    let host = rig.host();
    let ctx = &host.own();
    let root = "/module/fs/root";

    // A pipe: one name a line, sorted; a screen: down the columns that
    // fit across it.
    let piped = shown(ctx, &format!("ls {root}"), None);
    assert_eq!(piped, "alpha\nbeta\ngamma\n");
    let wide = shown(ctx, &format!("ls {root}"), Some(80));
    assert_eq!(wide, "alpha  beta  gamma\n");
    let narrow = shown(ctx, &format!("ls {root}"), Some(12));
    assert_eq!(narrow, "alpha  gamma\nbeta\n");

    // No target is the cwd of the record, which is where the line
    // stands; a cell lists itself.
    assert_eq!(shown(ctx, "ls", None), "bin\nmodule\nsession\nsys\nvar\n");
    assert_eq!(shown(ctx, &format!("ls {root}/beta"), None), "beta\n");

    // More than one target names each listing.
    let both = shown(ctx, &format!("ls /module {root}"), None);
    assert_eq!(
        both,
        format!(
            "/module:\ncoreutils\nfs\nsysutils\ntext\n\n\
             {root}:\nalpha\nbeta\ngamma\n"
        )
    );

    // A Container with nothing under it prints nothing, on a screen as
    // much as down a pipe.
    std::fs::create_dir(rig.dir().join("empty")).unwrap();
    let empty = format!("ls {root}/empty");
    assert_eq!(shown(ctx, &empty, Some(80)), "");
    assert_eq!(shown(ctx, &empty, None), "");
}

#[test]
fn the_long_line_says_type_rights_mtime_apis_and_name() {
    let rig = Rig::new("sh-ls-long");
    rig.file("note", b"hi\n");
    let host = rig.host();
    let ctx = &host.own();
    let root = "/module/fs/root";

    let modules = shown(ctx, "ls -l /module", None);
    let fs = modules.lines().find(|l| l.ends_with("  fs")).unwrap();
    assert!(fs.starts_with("c rw  "), "{fs}");
    let open = shown(ctx, "ls -l /module/text", None);
    assert!(open.trim_end().ends_with("  open  open"), "{open}");

    // `-a` puts the rd number first, in both forms; `-l -a` and `-la`
    // are the same two fields set.
    let rd = cell::resolve(ctx, ROOT, &format!("{root}/note")).unwrap();
    assert_eq!(
        shown(ctx, &format!("ls -a {root}"), None),
        format!("{} note\n", rd.0)
    );
    let long = shown(ctx, &format!("ls -la {root}"), None);
    assert_eq!(long, shown(ctx, &format!("ls -l -a {root}"), None));
    assert!(long.starts_with(&format!("{} - rw  ", rd.0)), "{long}");

    // An rd the Kernel does not name shows as its number.
    let first = shown(ctx, "stat /", None);
    let root = cell::resolve(ctx, ROOT, "/").unwrap();
    let line = first.lines().next().unwrap();
    assert!(line.ends_with(&format!("  rd:{}", root.0)), "{line}");
}

#[test]
fn the_bytes_the_times_and_the_drawing_one_line_prints() {
    let rig = Rig::new("sh-text");
    rig.file("one", b"first\n");
    rig.file("two", b"second\n");
    std::fs::create_dir(rig.dir().join("sub")).unwrap();
    std::fs::write(rig.dir().join("sub/deep"), b"x").unwrap();
    let host = rig.host();
    let ctx = &host.own();
    let root = "/module/fs/root";

    let one = shown(ctx, &format!("cat {root}/one"), None);
    assert_eq!(one, "first\n");
    let both = shown(ctx, &format!("cat {root}/one {root}/two"), None);
    assert_eq!(both, "first\nsecond\n");

    let said = shown(ctx, "stat /module/text/open", None);
    let lines: Vec<&str> = said.lines().collect();
    assert_eq!(lines[5], "open v1");
    assert_eq!(lines[6], "  0 open(Rd) -> Rd WRITE");
    assert_eq!(lines.len(), 8, "{said}");

    let drawn = shown(ctx, &format!("tree {root}"), None);
    assert_eq!(
        drawn,
        "/module/fs/root\n├── sub\n│   └── deep\n\
         ├── one\n└── two\n\n1 containers, 3 cells\n"
    );
    assert_eq!(
        shown(ctx, &format!("tree {root} -L 1"), None),
        "/module/fs/root\n├── sub\n├── one\n└── two\n\n\
         1 containers, 2 cells\n"
    );
    assert_eq!(
        shown(ctx, &format!("tree {root}/one"), None),
        "/module/fs/root/one\n\n0 containers, 0 cells\n"
    );
    let top = shown(ctx, "tree -L 1", None);
    assert!(top.starts_with("/\n├── bin\n├── module\n"), "{top}");
}

#[test]
fn every_name_reads_its_target_from_where_the_line_stands() {
    let rig = Rig::new("sh-targets");
    rig.file("note", b"hi\n");
    std::fs::create_dir(rig.dir().join("sub")).unwrap();
    std::fs::write(rig.dir().join("sub/deep"), b"x").unwrap();
    let host = rig.host();
    let ctx = &host.own();
    let mut shell = Shell::default();
    let mut go = |ctx: &dyn Calls, line: &str| typed(ctx, &mut shell, line);

    assert_eq!(go(ctx, "cd /module/fs/root"), "");
    // No target is the cwd the shell fills, for a listing and for the
    // tree under it.
    assert_eq!(go(ctx, "ls"), "note\nsub\n");
    assert_eq!(
        go(ctx, "tree -L 1"),
        "/module/fs/root\n├── sub\n└── note\n\n1 containers, 1 cells\n"
    );
    assert_eq!(go(ctx, "ls sub"), "deep\n");
    assert_eq!(go(ctx, "cat sub/deep"), "x");
    assert_eq!(go(ctx, "ls .."), "root\n");
    assert_eq!(go(ctx, "ls /sys"), "core\ndecls\ninfo\nlimit\nlog\nloglevel\n");
    assert!(go(ctx, "stat note").starts_with("- rw "));

    // What a name under the cwd cannot be. (A name that is not on
    // disk is a file fs makes on the first write, so the one that
    // resolves nowhere here is under a Container of the Kernel's.)
    assert_eq!(go(ctx, "cat ../../nope/x"), "cat ../../nope/x: invalid");
    assert_eq!(go(ctx, "cd note"), "cd note: unsupported");
    assert_eq!(go(ctx, "pwd"), "/module/fs/root\n");
}

#[test]
fn a_flag_is_the_fields_own_name_or_its_first_letter() {
    let rig = Rig::new("sh-flags");
    rig.file("note", b"first\n");
    let host = rig.host();
    let ctx = &host.own();
    let mut shell = Shell::default();
    let mut go = |ctx: &dyn Calls, line: &str| typed(ctx, &mut shell, line);

    // A field named by one letter takes the next token, whatever its
    // type; a `bool` is set where it stands.
    assert_eq!(go(ctx, "tree /module/text -L 2"), go(ctx, "tree /module/text"));
    assert_eq!(
        go(ctx, "cat -n 1 -f /module/fs/root/note"),
        "first\n",
        "the cell holds what the target held when the run began"
    );

    // Letter by letter, case for case: `t` and `x` are two fields.
    let plain = go(ctx, "dmesg -t");
    let decoded = go(ctx, "dmesg -tx");
    assert_ne!(plain, decoded);
    assert_eq!(decoded, go(ctx, "dmesg -t -x"));
    assert_ne!(go(ctx, "dmesg -T"), plain, "T is not t");

    // A number takes its token parsed: `log 5` is the entries from that
    // sequence on, and no token is from the first.
    let from = go(ctx, "log 5");
    assert!(from.starts_with("5 "), "{from}");
    assert!(go(ctx, "log").lines().count() > from.lines().count());

    // A whole name after the letters takes the token after it; a level
    // that is not one of the eight fails the run, and the sentence is
    // the shell's table.
    let level = go(ctx, "dmesg -tx -l err");
    assert!(!level.starts_with("dmesg "), "{level}");
    assert_eq!(go(ctx, "dmesg -tx -l nope"), "dmesg -tx -l nope: invalid");
}

#[test]
fn an_environment_field_is_never_matched_by_a_token() {
    let rig = Rig::new("sh-environment");
    let host = rig.host();
    let ctx = &host.own();
    let mut shell = Shell::default();

    // `cwd`, `width` and `color` are the shell's to fill, so the tokens
    // that spell them are targets and flags nobody declares.
    let form = "usage: ls [-l] [-a] [-w] [targets..]";
    let painted = typed(ctx, &mut shell, "ls -color");
    assert_eq!(painted, format!("ls -color: {form}"));
    let wide = typed(ctx, &mut shell, "ls -width");
    assert_eq!(wide, format!("ls -width: {form}"));
    let named = typed(ctx, &mut shell, "ls cwd");
    assert_eq!(named, "ls cwd: invalid", "a target");

    // The screen the host gives is what a listing is laid out across.
    shell.screen(80, false);
    let across = typed(ctx, &mut shell, "ls /sys");
    assert_eq!(across, "core  decls  info  limit  log  loglevel\n");
    shell.screen(0, false);
    let down = typed(ctx, &mut shell, "ls /sys");
    assert_eq!(down, "core\ndecls\ninfo\nlimit\nlog\nloglevel\n");
}

/// A directory `Shell::first` puts ahead of the Shell's own; PATH is
/// the Shell's and not a directory clients/sh names by its own hand,
/// so a directory it never heard of but Vim's `start` puts first works
/// the same as `/self/bin` and `/bin` do.
#[test]
fn first_puts_a_directory_ahead_and_a_default_shell_never_walks_it() {
    let rig = Rig::new("sh-first");
    let host = rig.host();
    let ctx = &host.own();

    // `ls` is the server's `/bin`'s own, three flags and a list of
    // targets. The same name stands under a directory made here as
    // Vim's `start` makes `/self/vim/bin`, with no Vim loaded at all.
    let self_rd = cell::resolve(ctx, ROOT, "/self").unwrap();
    let vim = ctx.register(DriverId(0), self_rd, Some("vim")).unwrap();
    ctx.report(vim, Flags::READ | Flags::CONTAINER, &[]).unwrap();
    let bin = ctx.register(DriverId(0), vim, Some("bin")).unwrap();
    ctx.report(bin, Flags::READ | Flags::CONTAINER, &[]).unwrap();
    let method = Method {
        number: 0,
        name: "run".to_string(),
        input: Ty::Record(Vec::new()),
        output: Ty::Rd,
        right: Rights::READ,
    };
    let decl =
        Decl { api: "shadow".to_string(), ver: 1, methods: vec![method] };
    let rd = ctx.register(DriverId(0), bin, Some("ls")).unwrap();
    ctx.report(rd, Flags::READ | Flags::CONTAINER, &[decl]).unwrap();

    // A default Shell never walks into it: the server's `ls` answers,
    // as if the directory were not there at all.
    let mut shell = Shell::default();
    let server = "ls [-l] [-a] [-w] [targets..]\n";
    assert_eq!(typed(ctx, &mut shell, "help ls"), server);

    // Put first, it wins over every directory the Shell already had,
    // `/self/bin` and `/bin` both.
    shell.first("/self/vim/bin");
    assert_eq!(typed(ctx, &mut shell, "help ls"), "ls\n");
}

#[test]
fn a_token_too_many_is_a_miss_with_the_form() {
    let rig = Rig::new("sh-miss");
    let host = rig.host();
    let ctx = &host.own();
    let mut shell = Shell::default();
    let mut go = |ctx: &dyn Calls, line: &str| typed(ctx, &mut shell, line);

    // A name with one positional and one token too many.
    assert_eq!(
        go(ctx, "stat /module /module"),
        "stat /module: usage: stat [target]"
    );
    // A letter no `bool` field is named by, and a flag with nothing
    // after it to take.
    assert_eq!(go(ctx, "ls -z"), "ls -z: usage: ls [-l] [-a] [-w] [targets..]");
    assert_eq!(go(ctx, "tree -L x"), "tree x: usage: tree [-L L] [target]");
    assert_eq!(go(ctx, "tree -L"), "tree -L: usage: tree [-L L] [target]");
    // A number that is not one, and a run whose record has no field at
    // all, so nothing is left for a token to be.
    assert_eq!(go(ctx, "log x"), "log x: usage: log [from]");
    assert_eq!(go(ctx, "uptime x"), "uptime x: usage: uptime");

    // A name PATH does not hold, and a line that names none.
    let held = "lookup: not under /self/bin /bin";
    assert_eq!(go(ctx, "lookup /"), format!("{held}; help lists them"));
    assert_eq!(go(ctx, "   "), "no name; help lists them");
}

#[test]
fn a_run_that_fails_prints_the_name_the_tokens_and_the_cause() {
    let rig = Rig::new("sh-refused");
    rig.file("note", b"hi\n");
    let host = rig.host();
    let ctx = &host.own();
    let mut shell = Shell::default();
    let cat = cell::resolve(ctx, ROOT, "/bin/cat").unwrap();
    let mut go = |ctx: &dyn Calls, line: &str| typed(ctx, &mut shell, line);

    // A Container holds no bytes, and a target that is not there is
    // `Invalid`; the run leaves no cell either way.
    assert_eq!(
        go(ctx, "cat /module/fs/root"),
        "cat /module/fs/root: unsupported"
    );
    assert_eq!(
        go(ctx, "cat /module/fs/root/note /nope"),
        "cat /module/fs/root/note /nope: invalid"
    );
    assert_eq!(ops::children(ctx, cat), Ok(Vec::new()));
}

#[test]
fn a_redirect_puts_a_runs_output_in_the_target() {
    let rig = Rig::new("sh-redirect");
    rig.file("note", b"hi\n");
    rig.file("out", b"");
    let host = rig.host();
    let ctx = &host.own();
    let mut shell = Shell::default();
    let root = "/module/fs/root";
    let out = cell::resolve(ctx, ROOT, &format!("{root}/out")).unwrap();

    // The run prints nothing and the target holds what it printed.
    let line = format!("cat {root}/note > {root}/out");
    assert_eq!(typed(ctx, &mut shell, &line), "");
    assert_eq!(ops::bytes(ctx, out), Ok(b"hi\n".to_vec()));
    // `>>` starts at what the target holds, which a whole read counts.
    let line = format!("cat {root}/note >> {root}/out");
    assert_eq!(typed(ctx, &mut shell, &line), "");
    assert_eq!(ops::bytes(ctx, out), Ok(b"hi\nhi\n".to_vec()));
    // An rd names the same cell, wherever the line stands.
    let line = format!("ls {root} > rd:{}", out.0);
    assert_eq!(typed(ctx, &mut shell, &line), "");
    assert_eq!(ops::bytes(ctx, out), Ok(b"note\nout\n".to_vec()));

    // The target resolves first, so a bad one runs nothing.
    let line = format!("cat {root}/note > {root}/nowhere/x");
    assert_eq!(
        typed(ctx, &mut shell, &line),
        "cat /module/fs/root/nowhere/x: no such rd (nowhere)"
    );
    assert_eq!(
        typed(ctx, &mut shell, "pwd > /nope"),
        "pwd: a redirect takes a run's output"
    );
    // A target that takes no bytes is the one place a line writes and
    // is refused; the run's own cell goes with the Miss.
    let ls = cell::resolve(ctx, ROOT, "/bin/ls").unwrap();
    let line = "ls / > /sys/info/mem";
    assert_eq!(typed(ctx, &mut shell, line), "ls /: denied");
    assert_eq!(ops::children(ctx, ls), Ok(Vec::new()));
}

#[test]
fn a_pipe_puts_one_runs_output_into_the_next_runs_input() {
    let rig = Rig::new("sh-pipe");
    let host = rig.host();
    let ctx = &host.own();
    let mut shell = Shell::default();

    // `input` is an environment field like `cwd`, so no `|` is needed
    // for the mechanism: a run reads whatever cell stands there, and
    // reading it ends it, this run being its one reader.
    let cell = celled(ctx);
    shell.input(cell);
    let out = ran(ctx, &shell, "grep", &["^up"]).unwrap().out;
    assert!(String::from_utf8(out).unwrap().starts_with("up "));
    assert_eq!(cells(ctx, "uptime"), Vec::new());
    assert_eq!(ops::info(ctx, cell), Err(Error::Invalid));

    // What a `|` does is fill it: the first stage answers a cell it
    // never opens, the second opens, reads and closes it, and the line
    // prints what the last stage found. Three stages the same way.
    let mut shell = Shell::default();
    let mut go = |ctx: &dyn Calls, line: &str| typed(ctx, &mut shell, line);
    assert_eq!(go(ctx, "ls /module | grep text"), "text\n");
    assert_eq!(go(ctx, "ls /module | grep text | grep ex"), "text\n");
    assert_eq!(cells(ctx, "ls"), Vec::new(), "the cell `ls` answered");
    assert_eq!(cells(ctx, "grep"), Vec::new());
}

#[test]
fn a_stage_that_cannot_stand_is_refused_and_leaves_no_cell_behind() {
    let rig = Rig::new("sh-stages");
    rig.file("out", b"");
    let host = rig.host();
    let ctx = &host.own();
    let mut shell = Shell::default();
    let root = "/module/fs/root";
    let out = cell::resolve(ctx, ROOT, &format!("{root}/out")).unwrap();
    let mut go = |ctx: &dyn Calls, line: &str| typed(ctx, &mut shell, line);

    // A stage that names nothing is the line that names nothing.
    let none = "no name; help lists them";
    assert_eq!(go(ctx, "ls /module |"), none);
    assert_eq!(go(ctx, "| grep x"), none);
    assert_eq!(go(ctx, "ls | | grep x"), none);

    // A builtin answers bytes and not a cell, wherever it stands; and a
    // stage's output being the next stage's input, only the last one
    // has anywhere else to put it.
    let takes = "a pipe takes a run's output";
    assert_eq!(go(ctx, "cd | grep x"), format!("cd: {takes}"));
    assert_eq!(go(ctx, "ls | cd"), format!("cd: {takes}"));
    let line = format!("ls > {root}/out | grep x");
    assert_eq!(go(ctx, &line), "a redirect belongs to the last stage");

    // And a stage after a `|` is filled through a declared `input: Rd`,
    // so a name that declares none is refused from the declarations,
    // wherever it stands and before anything is called.
    assert_eq!(go(ctx, "ls /module | cat"), "cat: takes no input");
    assert_eq!(go(ctx, "ls | stat | grep x"), "stat: takes no input");

    // A later stage that fails takes the cell it was handed with it.
    assert_eq!(go(ctx, "ls /module | nope"), {
        "nope: not under /self/bin /bin; help lists them"
    });
    assert_eq!(go(ctx, "ls /module | grep a\\{2}"), "grep a\\{2}: invalid");
    assert_eq!(cells(ctx, "ls"), Vec::new(), "the cells `ls` answered");
    assert_eq!(cells(ctx, "grep"), Vec::new());
    assert_eq!(ops::bytes(ctx, out), Ok(Vec::new()), "none of it ran");
}

#[test]
fn a_line_that_redirects_or_pipes_writes_for_no_screen_at_all() {
    let rig = Rig::new("sh-color");
    rig.file("out", b"");
    let host = rig.host();
    let ctx = &host.own();
    let mut shell = Shell::default();
    let root = "/module/fs/root";
    let out = cell::resolve(ctx, ROOT, &format!("{root}/out")).unwrap();

    // The host's screen, as the cli buffer sets it: wide and painted.
    shell.screen(80, true);
    let seen = typed(ctx, &mut shell, "ls /module");
    assert!(seen.contains("\x1b["), "what the screen shows is painted");

    // A line with a `>` writes for no screen, so the file holds the
    // text and not the escapes 01.8 left in it; and so does every stage
    // of a pipe but the last, which is why a pattern matches a name.
    let line = format!("ls /module > {root}/out");
    assert_eq!(typed(ctx, &mut shell, &line), "");
    let held = ops::bytes(ctx, out).unwrap();
    assert_eq!(held, b"coreutils\nfs\nsysutils\ntext\n".to_vec());
    let line = format!("ls /module | grep text > {root}/out");
    assert_eq!(typed(ctx, &mut shell, &line), "");
    assert_eq!(ops::bytes(ctx, out), Ok(b"text\n".to_vec()));
}

/// A `Screen` of one line, its whole span inverse: what `capture`
/// always paints, a bin that declares no `color` of its own and so
/// asks the shell nothing about it.
fn inverse(text: &str) -> Vec<u8> {
    let to = u64::try_from(text.len()).unwrap_or(u64::MAX);
    let run = display::Run { from: 0, to, style: look::INVERSE };
    let line = display::Line { text: text.to_string(), runs: vec![run] };
    encode(&display::Screen {
        rows: 2,
        row: 0,
        col: 0,
        shape: BAR,
        lines: vec![line],
        status: String::new(),
    })
}

#[test]
fn color_strips_or_keeps_a_bin_s_own_escapes_at_the_last_stage() {
    let rig = Rig::new("sh-color-strip");
    rig.file("screen", &inverse("hi"));
    let host = rig.host();
    let ctx = &host.own();
    let root = "/module/fs/root";
    let line = format!("capture {root}/screen");

    // `capture` never asks the shell for `color`: it always paints its
    // runs, and the shell strips them at the read where color is off.
    let mut shell = Shell::default();
    shell.screen(0, false);
    let plain = typed(ctx, &mut shell, &line);
    assert!(!plain.contains('\x1b'), "no escape is left: {plain:?}");
    assert_eq!(plain, "hi\n\ncursor 0 0 bar\n");

    // The same run, asked for color: the escapes stand.
    let mut shell = Shell::default();
    shell.screen(0, true);
    let painted = typed(ctx, &mut shell, &line);
    assert!(painted.contains("\x1b[7mhi\x1b[0m"), "escapes stand: {painted:?}");
}

#[test]
fn a_redirect_target_holds_exactly_what_the_bin_wrote() {
    let rig = Rig::new("sh-color-redirect");
    rig.file("screen", &inverse("hi"));
    rig.file("out", b"");
    let host = rig.host();
    let ctx = &host.own();
    let root = "/module/fs/root";
    let out = cell::resolve(ctx, ROOT, &format!("{root}/out")).unwrap();

    // A file is not a reader for `color` to be asked of: capture should
    // be byte, so a redirect target holds the run's own bytes whole,
    // escapes and all, whatever the shell's own color asks for.
    for color in [false, true] {
        let mut shell = Shell::default();
        shell.screen(0, color);
        let line = format!("capture {root}/screen > {root}/out");
        let printed = typed(ctx, &mut shell, &line);
        assert_eq!(printed, "", "a redirect prints nothing, color={color}");
        let held = ops::bytes(ctx, out).unwrap();
        let expected = b"\x1b[7mhi\x1b[0m\n\ncursor 0 0 bar\n";
        assert_eq!(held, expected, "color={color}");
    }
}

#[test]
fn a_redirect_copies_a_file_byte_for_byte_escapes_and_all() {
    let rig = Rig::new("sh-color-cat-copy");
    // A whole SGR run and a bare escape with nothing after it: neither
    // is a Driver's own colour to strip, the file holding them as its
    // own content — `cat` declares no `color` either.
    let content: &[u8] = b"a\x1b[7mb\x1b[0m\x00\x1b";
    rig.file("f", content);
    rig.file("g", b"");
    let host = rig.host();
    let ctx = &host.own();
    let root = "/module/fs/root";
    let g = cell::resolve(ctx, ROOT, &format!("{root}/g")).unwrap();
    let mut shell = Shell::default();
    shell.screen(0, false);
    let line = format!("cat {root}/f > {root}/g");
    assert_eq!(typed(ctx, &mut shell, &line), "");
    assert_eq!(ops::bytes(ctx, g).unwrap(), content, "byte for byte");
}

#[test]
fn a_following_run_comes_back_open_at_its_offset_and_closes_whole() {
    let rig = Rig::new("sh-following");
    rig.file("note", b"first\n");
    let host = rig.host();
    let ctx = &host.own();
    let mut other = rig.client();
    let mut shell = Shell::default();
    let target = "/module/fs/root/note";
    let rd = cell::resolve(ctx, ROOT, target).unwrap();
    let cat = cell::resolve(ctx, ROOT, "/bin/cat").unwrap();

    let ran = run(ctx, &mut shell, &format!("cat -f {target}")).unwrap();
    assert_eq!(ran.out, b"first\n".to_vec());
    let mut open = ran.open.expect("a follower comes back open");
    // At the end of what the cell holds, with nothing new: not the end.
    assert_eq!(open.more(ctx), Ok((Vec::new(), false)));

    other.write_all(rd, b"second\n".to_vec()).unwrap();
    assert_eq!(open.more(ctx), Ok((b"second\n".to_vec(), false)));

    // The close ends the follow whole: the watch goes with it and the
    // Kernel holds no such rd.
    let cell = ops::children(ctx, cat).unwrap();
    assert_eq!(cell.len(), 1);
    assert_eq!(open.close(ctx), Ok(()));
    assert_eq!(ops::children(ctx, cat), Ok(Vec::new()));
    assert_eq!(ops::info(ctx, cell[0]), Err(Error::Invalid));
}

#[test]
fn a_following_run_that_redirects_pours_every_batch_where_the_first_went() {
    let rig = Rig::new("sh-following-redirect");
    rig.file("note", b"first\n");
    rig.file("out", b"");
    let host = rig.host();
    let ctx = &host.own();
    let mut other = rig.client();
    let mut shell = Shell::default();
    let root = "/module/fs/root";
    let target = format!("{root}/note");
    let rd = cell::resolve(ctx, ROOT, &target).unwrap();
    let out = cell::resolve(ctx, ROOT, &format!("{root}/out")).unwrap();

    // The run comes back open with nothing to print, the first batch
    // being in the target.
    let ran = run(ctx, &mut shell, &format!("cat -f {target} > {root}/out"));
    let ran = ran.unwrap();
    assert_eq!(ran.out, Vec::new());
    assert_eq!(ops::bytes(ctx, out), Ok(b"first\n".to_vec()));
    let mut open = ran.open.expect("a follower comes back open");

    // And the batches after it go where that one went, after it: the
    // redirect is the run's and not the first read's.
    other.write_all(rd, b"second\n".to_vec()).unwrap();
    assert_eq!(open.more(ctx), Ok((Vec::new(), false)), "none of it printed");
    assert_eq!(ops::bytes(ctx, out), Ok(b"first\nsecond\n".to_vec()));
    assert_eq!(open.close(ctx), Ok(()));
}

#[test]
fn a_following_run_that_cannot_pour_says_so_and_leaves_no_cell_behind() {
    let rig = Rig::new("sh-following-refused");
    rig.file("note", b"first\n");
    rig.file("out", b"");
    let host = rig.host();
    let ctx = &host.own();
    let mut other = rig.client();
    let mut shell = Shell::default();
    let root = "/module/fs/root";
    let target = format!("{root}/note");
    let rd = cell::resolve(ctx, ROOT, &target).unwrap();
    let cat = cell::resolve(ctx, ROOT, "/bin/cat").unwrap();

    let ran = run(ctx, &mut shell, &format!("cat -f {target} > {root}/out"));
    let mut open = ran.unwrap().open.expect("a follower comes back open");
    assert_eq!(cells(ctx, "cat").len(), 1, "the cell it follows");

    // The target is gone from under the run: the Miss says so as the
    // first batch's would have, and the cell goes with it, nobody
    // being left to come back to it.
    std::fs::remove_file(rig.dir().join("out")).unwrap();
    other.write_all(rd, b"second\n".to_vec()).unwrap();
    let stopped = open.more(ctx).expect_err("the write is refused");
    assert_eq!(stopped.to_string(), format!("cat -f {target}: gone"));
    assert_eq!(ops::children(ctx, cat), Ok(Vec::new()));
}

#[test]
fn a_run_that_counts_its_answers_ends_at_the_empty_read() {
    let rig = Rig::new("sh-counted");
    rig.file("note", b"first\n");
    let host = rig.host();
    let ctx = &host.own();
    let mut other = rig.client();
    let mut shell = Shell::default();
    let target = "/module/fs/root/note";
    let rd = cell::resolve(ctx, ROOT, target).unwrap();
    let cat = cell::resolve(ctx, ROOT, "/bin/cat").unwrap();

    let line = format!("cat -f -n 1 {target}");
    let ran = run(ctx, &mut shell, &line).unwrap();
    let mut open = ran.open.expect("a follower comes back open");
    other.write_all(rd, b"second\n".to_vec()).unwrap();
    // The one answer is spent, so the read past it is the empty read
    // that ends the cell, and the close comes with it.
    assert_eq!(open.more(ctx), Ok((b"second\n".to_vec(), true)));
    assert_eq!(ops::children(ctx, cat), Ok(Vec::new()));
}

#[test]
fn a_quoted_run_is_one_token() {
    let rig = Rig::new("quotes-joined");
    rig.file("my file", b"spaced\n");
    rig.file("f", b"a b\na|b\n>\n(a)\n");
    let host = rig.host();
    let ctx = &host.own();
    assert_eq!(shown(ctx, "cat '/module/fs/root/my file'", None), "spaced\n");
    assert_eq!(shown(ctx, "grep a\" \"b /module/fs/root/f", None), "a b\n");
    assert_eq!(
        shown(ctx, "grep '' /module/fs/root/f", None),
        "a b\na|b\n>\n(a)\n"
    );
}

#[test]
fn a_quoted_operator_is_a_word() {
    let rig = Rig::new("quotes-operators");
    rig.file("|", b"pipe\n");
    rig.file(">", b"greater\n");
    rig.file(">>", b"append\n");
    rig.file("out file", b"");
    rig.file("f", b"a|b\n>\n");
    let host = rig.host();
    let ctx = &host.own();
    let mut shell = Shell::default();
    typed(ctx, &mut shell, "cd /module/fs/root");
    for (line, expected) in [
        ("grep \"a|b\" f", "a|b\n"),
        ("grep '>' f", ">\n"),
        ("cat '|'", "pipe\n"),
        ("cat '>'", "greater\n"),
        ("cat \">>\"", "append\n"),
        ("cat ''|", "pipe\n"),
        ("cat >''", "greater\n"),
        ("cat >''>", "append\n"),
        ("cat '|' | grep pipe", "pipe\n"),
        ("cat '>' > 'out file'", ""),
        ("cat '|' >> 'out file'", ""),
    ] {
        assert_eq!(typed(ctx, &mut shell, line), expected, "{line}");
    }
    assert_eq!(rig.disk("out file"), b"greater\npipe\n");
}

#[test]
fn a_backslash_is_a_char_as_before() {
    let rig = Rig::new("quotes-backslash");
    rig.file("f", b"(a)\na\n");
    let host = rig.host();
    assert_eq!(
        shown(&host.own(), r"grep \(a\) /module/fs/root/f", None),
        "(a)\na\n"
    );
}

#[test]
fn an_open_quote_is_a_miss_and_nothing_ran() {
    let rig = Rig::new("quotes-unclosed");
    rig.file("out", b"kept\n");
    let host = rig.host();
    let ctx = &host.own();
    let mut shell = Shell::default();
    for line in [
        "cat \"my file > /module/fs/root/out",
        "cat /module/fs/root/out | cat 'unfinished",
        "cd /module/fs/root '",
    ] {
        assert_eq!(typed(ctx, &mut shell, line), "unclosed quote");
        assert_eq!(rig.disk("out"), b"kept\n");
        assert_eq!(typed(ctx, &mut shell, "pwd"), "/\n");
        assert_eq!(cells(ctx, "cat"), Vec::new());
    }
}
