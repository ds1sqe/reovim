//! A typed line over the rig: the name resolved under `/bin`, the
//! record filled from the tokens, and the output cell read to its end.

use reovim_client::{Client, Error, Fault};
use reovim_client_sh::{Shell, run};
use reovim_rig::Rig;

/// One typed line from where `shell` stands; its output, or the line a
/// Miss would print.
fn typed(client: &mut Client, shell: &mut Shell, line: &str) -> String {
    match run(client, shell, line) {
        Ok(ran) => String::from_utf8(ran.out).unwrap(),
        Err(miss) => miss.to_string(),
    }
}

/// One typed line from the root, on a screen `width` wide; its output.
fn shown(client: &mut Client, line: &str, width: Option<u16>) -> String {
    let mut shell = Shell::default();
    shell.screen(u64::from(width.unwrap_or(0)), false);
    typed(client, &mut shell, line)
}

#[test]
fn a_listing_is_laid_out_across_the_screen_the_shell_gives() {
    let rig = Rig::new("sh-ls");
    for name in ["gamma", "alpha", "beta"] {
        rig.file(name, b"");
    }
    let mut client = rig.client();
    let root = "/module/fs/root";

    // A pipe: one name a line, sorted; a screen: down the columns that
    // fit across it.
    let piped = shown(&mut client, &format!("ls {root}"), None);
    assert_eq!(piped, "alpha\nbeta\ngamma\n");
    let wide = shown(&mut client, &format!("ls {root}"), Some(80));
    assert_eq!(wide, "alpha  beta  gamma\n");
    let narrow = shown(&mut client, &format!("ls {root}"), Some(12));
    assert_eq!(narrow, "alpha  gamma\nbeta\n");

    // No target is the cwd of the record, which is where the line
    // stands; a cell lists itself.
    assert_eq!(
        shown(&mut client, "ls", None),
        "bin\nmodule\nsession\nsys\nvar\n"
    );
    assert_eq!(shown(&mut client, &format!("ls {root}/beta"), None), "beta\n");

    // More than one target names each listing.
    let both = shown(&mut client, &format!("ls /module {root}"), None);
    assert_eq!(
        both,
        format!(
            "/module:\nclipboard\ncoreutils\nfs\nsysutils\ntext\n\n\
             {root}:\nalpha\nbeta\ngamma\n"
        )
    );

    // A Container with nothing under it prints nothing, on a screen as
    // much as down a pipe.
    std::fs::create_dir(rig.dir().join("empty")).unwrap();
    let empty = format!("ls {root}/empty");
    assert_eq!(shown(&mut client, &empty, Some(80)), "");
    assert_eq!(shown(&mut client, &empty, None), "");
}

#[test]
fn the_long_line_says_type_rights_mtime_apis_and_name() {
    let rig = Rig::new("sh-ls-long");
    rig.file("note", b"hi\n");
    let mut client = rig.client();
    let root = "/module/fs/root";

    let modules = shown(&mut client, "ls -l /module", None);
    let fs = modules.lines().find(|l| l.ends_with("  fs")).unwrap();
    assert!(fs.starts_with("c rw  "), "{fs}");
    let open = shown(&mut client, "ls -l /module/text", None);
    assert!(open.trim_end().ends_with("  open  open"), "{open}");

    // `-a` puts the rd number first, in both forms; `-l -a` and `-la`
    // are the same two fields set.
    let rd = client.resolve(&format!("{root}/note")).unwrap();
    assert_eq!(
        shown(&mut client, &format!("ls -a {root}"), None),
        format!("{} note\n", rd.0)
    );
    let long = shown(&mut client, &format!("ls -la {root}"), None);
    assert_eq!(long, shown(&mut client, &format!("ls -l -a {root}"), None));
    assert!(long.starts_with(&format!("{} - rw  ", rd.0)), "{long}");

    // An rd the Kernel does not name shows as its number.
    let first = shown(&mut client, "stat /", None);
    let root = client.resolve("/").unwrap();
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
    let mut client = rig.client();
    let root = "/module/fs/root";

    let one = shown(&mut client, &format!("cat {root}/one"), None);
    assert_eq!(one, "first\n");
    let both = shown(&mut client, &format!("cat {root}/one {root}/two"), None);
    assert_eq!(both, "first\nsecond\n");

    let said = shown(&mut client, "stat /module/text/open", None);
    let lines: Vec<&str> = said.lines().collect();
    assert_eq!(lines[5], "open v1");
    assert_eq!(lines[6], "  0 open(Rd) -> Rd WRITE");
    assert_eq!(lines.len(), 8, "{said}");

    let drawn = shown(&mut client, &format!("tree {root}"), None);
    assert_eq!(
        drawn,
        "/module/fs/root\n├── sub\n│   └── deep\n\
         ├── one\n└── two\n\n1 containers, 3 cells\n"
    );
    assert_eq!(
        shown(&mut client, &format!("tree {root} -L 1"), None),
        "/module/fs/root\n├── sub\n├── one\n└── two\n\n\
         1 containers, 2 cells\n"
    );
    assert_eq!(
        shown(&mut client, &format!("tree {root}/one"), None),
        "/module/fs/root/one\n\n0 containers, 0 cells\n"
    );
    let top = shown(&mut client, "tree -L 1", None);
    assert!(top.starts_with("/\n├── bin\n├── module\n"), "{top}");
}

#[test]
fn every_name_reads_its_target_from_where_the_line_stands() {
    let rig = Rig::new("sh-targets");
    rig.file("note", b"hi\n");
    std::fs::create_dir(rig.dir().join("sub")).unwrap();
    std::fs::write(rig.dir().join("sub/deep"), b"x").unwrap();
    let mut client = rig.client();
    let mut shell = Shell::default();
    let mut go =
        |client: &mut Client, line: &str| typed(client, &mut shell, line);

    assert_eq!(go(&mut client, "cd /module/fs/root"), "");
    // No target is the cwd the shell fills, for a listing and for the
    // tree under it.
    assert_eq!(go(&mut client, "ls"), "note\nsub\n");
    assert_eq!(
        go(&mut client, "tree -L 1"),
        "/module/fs/root\n├── sub\n└── note\n\n1 containers, 1 cells\n"
    );
    assert_eq!(go(&mut client, "ls sub"), "deep\n");
    assert_eq!(go(&mut client, "cat sub/deep"), "x");
    assert_eq!(go(&mut client, "ls .."), "root\n");
    assert_eq!(
        go(&mut client, "ls /sys"),
        "core\ndecls\nlimit\nlog\nloglevel\nmem\n"
    );
    assert!(go(&mut client, "stat note").starts_with("- rw "));

    // What a name under the cwd cannot be. (A name that is not on
    // disk is a file fs makes on the first write, so the one that
    // resolves nowhere here is under a Container of the Kernel's.)
    assert_eq!(
        go(&mut client, "cat ../../nope/x"),
        "cat ../../nope/x: invalid"
    );
    assert_eq!(go(&mut client, "cd note"), "cd note: unsupported");
    assert_eq!(go(&mut client, "pwd"), "/module/fs/root\n");
}

#[test]
fn a_flag_is_the_fields_own_name_or_its_first_letter() {
    let rig = Rig::new("sh-flags");
    rig.file("note", b"first\n");
    let mut client = rig.client();
    let mut shell = Shell::default();
    let mut go =
        |client: &mut Client, line: &str| typed(client, &mut shell, line);

    // A field named by one letter takes the next token, whatever its
    // type; a `bool` is set where it stands.
    assert_eq!(
        go(&mut client, "tree /module/text -L 2"),
        go(&mut client, "tree /module/text")
    );
    assert_eq!(
        go(&mut client, "cat -n 1 -f /module/fs/root/note"),
        "first\n",
        "the cell holds what the target held when the run began"
    );

    // Letter by letter, case for case: `t` and `x` are two fields.
    let plain = go(&mut client, "dmesg -t");
    let decoded = go(&mut client, "dmesg -tx");
    assert_ne!(plain, decoded);
    assert_eq!(decoded, go(&mut client, "dmesg -t -x"));
    assert_ne!(go(&mut client, "dmesg -T"), plain, "T is not t");

    // A number takes its token parsed: `log 5` is the entries from that
    // sequence on, and no token is from the first.
    let from = go(&mut client, "log 5");
    assert!(from.starts_with("5 "), "{from}");
    assert!(go(&mut client, "log").lines().count() > from.lines().count());

    // A whole name after the letters takes the token after it; a level
    // that is not one of the eight fails the run, and the sentence is
    // the shell's table.
    let level = go(&mut client, "dmesg -tx -l err");
    assert!(!level.starts_with("dmesg "), "{level}");
    assert_eq!(
        go(&mut client, "dmesg -tx -l nope"),
        "dmesg -tx -l nope: invalid"
    );
}

#[test]
fn an_environment_field_is_never_matched_by_a_token() {
    let rig = Rig::new("sh-environment");
    let mut client = rig.client();
    let mut shell = Shell::default();

    // `cwd`, `width` and `color` are the shell's to fill, so the tokens
    // that spell them are targets and flags nobody declares.
    let form = "usage: ls [-l] [-a] [-w] [targets..]";
    let painted = typed(&mut client, &mut shell, "ls -color");
    assert_eq!(painted, format!("ls -color: {form}"));
    let wide = typed(&mut client, &mut shell, "ls -width");
    assert_eq!(wide, format!("ls -width: {form}"));
    let named = typed(&mut client, &mut shell, "ls cwd");
    assert_eq!(named, "ls cwd: invalid", "a target");

    // The screen the host gives is what a listing is laid out across.
    shell.screen(80, false);
    let across = typed(&mut client, &mut shell, "ls /sys");
    assert_eq!(across, "core  decls  limit  log  loglevel  mem\n");
    shell.screen(0, false);
    let down = typed(&mut client, &mut shell, "ls /sys");
    assert_eq!(down, "core\ndecls\nlimit\nlog\nloglevel\nmem\n");
}

#[test]
fn a_token_too_many_is_a_miss_with_the_form() {
    let rig = Rig::new("sh-miss");
    let mut client = rig.client();
    let mut shell = Shell::default();
    let mut go =
        |client: &mut Client, line: &str| typed(client, &mut shell, line);

    // A name with one positional and one token too many.
    assert_eq!(
        go(&mut client, "stat /module /module"),
        "stat /module: usage: stat [target]"
    );
    // A letter no `bool` field is named by, and a flag with nothing
    // after it to take.
    assert_eq!(
        go(&mut client, "ls -z"),
        "ls -z: usage: ls [-l] [-a] [-w] [targets..]"
    );
    assert_eq!(
        go(&mut client, "tree -L x"),
        "tree x: usage: tree [-L L] [target]"
    );
    assert_eq!(
        go(&mut client, "tree -L"),
        "tree -L: usage: tree [-L L] [target]"
    );
    // A number that is not one.
    assert_eq!(go(&mut client, "log x"), "log x: usage: log [from]");

    // A name PATH does not hold, and a line that names none.
    assert_eq!(
        go(&mut client, "lookup /"),
        "lookup: not under /bin; help lists them"
    );
    assert_eq!(go(&mut client, "   "), "no name; help lists them");
}

#[test]
fn a_run_that_fails_prints_the_name_the_tokens_and_the_cause() {
    let rig = Rig::new("sh-refused");
    rig.file("note", b"hi\n");
    let mut client = rig.client();
    let mut shell = Shell::default();
    let cat = client.resolve("/bin/cat").unwrap();
    let mut go =
        |client: &mut Client, line: &str| typed(client, &mut shell, line);

    // A Container holds no bytes, and a target that is not there is
    // `Invalid`; the run leaves no cell either way.
    assert_eq!(
        go(&mut client, "cat /module/fs/root"),
        "cat /module/fs/root: unsupported"
    );
    assert_eq!(
        go(&mut client, "cat /module/fs/root/note /nope"),
        "cat /module/fs/root/note /nope: invalid"
    );
    assert_eq!(client.children(cat), Ok(Vec::new()));
}

#[test]
fn a_redirect_puts_a_runs_output_in_the_target() {
    let rig = Rig::new("sh-redirect");
    rig.file("note", b"hi\n");
    rig.file("out", b"");
    let mut client = rig.client();
    let mut shell = Shell::default();
    let root = "/module/fs/root";
    let out = client.resolve(&format!("{root}/out")).unwrap();

    // The run prints nothing and the target holds what it printed.
    let line = format!("cat {root}/note > {root}/out");
    assert_eq!(typed(&mut client, &mut shell, &line), "");
    assert_eq!(client.read_all(out), Ok(b"hi\n".to_vec()));
    // `>>` starts at what the target holds, which a whole read counts.
    let line = format!("cat {root}/note >> {root}/out");
    assert_eq!(typed(&mut client, &mut shell, &line), "");
    assert_eq!(client.read_all(out), Ok(b"hi\nhi\n".to_vec()));
    // An rd names the same cell, wherever the line stands.
    let line = format!("ls {root} > rd:{}", out.0);
    assert_eq!(typed(&mut client, &mut shell, &line), "");
    assert_eq!(client.read_all(out), Ok(b"note\nout\n".to_vec()));

    // The target resolves first, so a bad one runs nothing.
    let line = format!("cat {root}/note > {root}/nowhere/x");
    assert_eq!(
        typed(&mut client, &mut shell, &line),
        "cat /module/fs/root/nowhere/x: no such rd (nowhere)"
    );
    assert_eq!(
        typed(&mut client, &mut shell, "pwd > /nope"),
        "pwd: a redirect takes a run's output"
    );
    // A target that takes no bytes is the one place a line writes and
    // is refused; the run's own cell goes with the Miss.
    let ls = client.resolve("/bin/ls").unwrap();
    assert_eq!(
        typed(&mut client, &mut shell, "ls / > /sys/mem"),
        "ls /: denied"
    );
    assert_eq!(client.children(ls), Ok(Vec::new()));
}

#[test]
fn a_following_run_comes_back_open_at_its_offset_and_closes_whole() {
    let rig = Rig::new("sh-following");
    rig.file("note", b"first\n");
    let mut client = rig.client();
    let mut other = rig.client();
    let mut shell = Shell::default();
    let target = "/module/fs/root/note";
    let rd = client.resolve(target).unwrap();
    let cat = client.resolve("/bin/cat").unwrap();

    let ran =
        run(&mut client, &mut shell, &format!("cat -f {target}")).unwrap();
    assert_eq!(ran.out, b"first\n".to_vec());
    let mut open = ran.open.expect("a follower comes back open");
    // At the end of what the cell holds, with nothing new: not the end.
    assert_eq!(open.more(&mut client), Ok((Vec::new(), false)));

    other.write_all(rd, b"second\n".to_vec()).unwrap();
    assert_eq!(open.more(&mut client), Ok((b"second\n".to_vec(), false)));

    // The close ends the follow whole: the watch goes with it and the
    // Kernel holds no such rd.
    let cell = client.children(cat).unwrap();
    assert_eq!(cell.len(), 1);
    assert_eq!(open.close(&mut client), Ok(()));
    assert_eq!(client.children(cat), Ok(Vec::new()));
    assert_eq!(client.info(cell[0]), Err(Fault::Kernel(Error::Invalid)));
}

#[test]
fn a_run_that_counts_its_answers_ends_at_the_empty_read() {
    let rig = Rig::new("sh-counted");
    rig.file("note", b"first\n");
    let mut client = rig.client();
    let mut other = rig.client();
    let mut shell = Shell::default();
    let target = "/module/fs/root/note";
    let rd = client.resolve(target).unwrap();
    let cat = client.resolve("/bin/cat").unwrap();

    let line = format!("cat -f -n 1 {target}");
    let ran = run(&mut client, &mut shell, &line).unwrap();
    let mut open = ran.open.expect("a follower comes back open");
    other.write_all(rd, b"second\n".to_vec()).unwrap();
    // The one answer is spent, so the read past it is the empty read
    // that ends the cell, and the close comes with it.
    assert_eq!(open.more(&mut client), Ok((b"second\n".to_vec(), true)));
    assert_eq!(client.children(cat), Ok(Vec::new()));
}
