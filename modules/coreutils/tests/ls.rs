//! `ls` over the rig: what one run's output cell holds, beside the
//! listing the cli's own `ls` prints of the same targets in the same
//! tree, with the same width and color.

use reovim_client::{Client, Error, Fault, ROOT, Rd, Rights};
use reovim_client_cli::{Out, Paint, Shell, input};
use reovim_module_coreutils::bins::ls::Args;
use reovim_rig::Rig;

/// A run of the targets, as a shell fills the record: no flag, no
/// width, no color, from the root.
fn args(targets: &[&str]) -> Args {
    Args {
        long: false,
        all: false,
        cwd: ROOT,
        width: 0,
        color: false,
        targets: targets.iter().map(|t| (*t).to_string()).collect(),
    }
}

/// `call /bin/ls run {..}`: the rd of the output cell, or the fault
/// the call answered with.
fn run(client: &mut Client, args: &Args) -> Result<Rd, Fault> {
    let ls = client.resolve("/bin/ls")?;
    let out = client.call_by_name(ls, "run", reovim_sdk::encode(args))?;
    Ok(reovim_sdk::decode(&out)?)
}

/// The cell `rd` read at an offset to the empty read that ends it, and
/// closed, which is how a shell reads one.
fn drain(client: &mut Client, rd: Rd) -> String {
    let h = client.open(rd, Rights::READ).unwrap();
    let mut bytes = Vec::new();
    loop {
        let at = bytes.len() as u64;
        let chunk = client.read(h, at, 64).unwrap();
        if chunk.is_empty() {
            break;
        }
        bytes.extend(chunk);
    }
    client.close(h).unwrap();
    String::from_utf8(bytes).unwrap()
}

/// The run and the cli line that are the same text: that text, once
/// both have printed it. `at` is where the line stands, which the cli
/// keeps as a path and the run as the `cwd` of its record.
fn same(
    client: &mut Client,
    at: (Rd, &str),
    args: &Args,
    line: &str,
    out: Out,
) -> String {
    let cell = run(client, args).unwrap();
    let said = drain(client, cell);
    let words: Vec<String> =
        line.split_whitespace().map(str::to_string).collect();
    let mut shell = Shell::at(at.0, at.1.to_string());
    let printed = input(client, &mut shell, &words, out).unwrap();
    assert_eq!(said, String::from_utf8(printed).unwrap(), "{line}");
    said
}

#[test]
fn a_listing_is_laid_out_down_the_columns_that_fit_the_width() {
    let rig = Rig::new("ls-columns");
    for name in ["gamma", "alpha", "beta"] {
        rig.file(name, b"");
    }
    let mut client = rig.client();
    let root = "/module/fs/root";
    let here = (ROOT, "/");

    // No width is one name a line, as a pipe wants them; a width lays
    // them out down the columns that fit in it.
    let listed = same(
        &mut client,
        here,
        &args(&[root]),
        "ls /module/fs/root",
        Out::plain(),
    );
    assert_eq!(listed, "alpha\nbeta\ngamma\n");
    for width in [80, 12] {
        let mut wide = args(&[root]);
        wide.width = u64::from(width);
        let out = Out::screen(width, false);
        same(&mut client, here, &wide, "ls /module/fs/root", out);
    }

    // No target at all is the `cwd` the run was given; a target that
    // is no Container lists itself.
    let all = same(&mut client, here, &args(&[]), "ls", Out::plain());
    assert_eq!(all, "bin\nmodule\nsession\nsys\nvar\n");
    let one = args(&[&format!("{root}/beta")]);
    let beta =
        same(&mut client, here, &one, &format!("ls {root}/beta"), Out::plain());
    assert_eq!(beta, "beta\n");

    // More than one target names each listing, as `ls` does.
    let both = args(&["/module", root]);
    let listed = same(
        &mut client,
        here,
        &both,
        &format!("ls /module {root}"),
        Out::plain(),
    );
    assert_eq!(
        listed,
        format!(
            "/module:\nclipboard\ncoreutils\nfs\ntext\n\n\
             {root}:\nalpha\nbeta\ngamma\n"
        )
    );

    // A Container with nothing under it prints nothing, laid out
    // across a width as much as down a pipe.
    std::fs::create_dir(rig.dir().join("empty")).unwrap();
    let empty = format!("{root}/empty");
    let line = format!("ls {empty}");
    assert_eq!(
        same(&mut client, here, &args(&[&empty]), &line, Out::plain()),
        ""
    );
    let mut wide = args(&[&empty]);
    wide.width = 80;
    assert_eq!(
        same(&mut client, here, &wide, &line, Out::screen(80, false)),
        ""
    );
}

#[test]
fn the_long_line_and_the_rds_are_the_flags_the_record_carries() {
    let rig = Rig::new("ls-long");
    rig.file("note", b"hi\n");
    let mut client = rig.client();
    let root = "/module/fs/root";
    let here = (ROOT, "/");

    for target in ["/module", root, "/module/text"] {
        let mut long = args(&[target]);
        long.long = true;
        let line = format!("ls -l {target}");
        let said = same(&mut client, here, &long, &line, Out::plain());
        assert!(said.ends_with('\n'), "{said}");

        // `-a` puts the rd number first, in both forms.
        let mut rds = args(&[target]);
        rds.all = true;
        same(&mut client, here, &rds, &format!("ls -a {target}"), Out::plain());
        long.all = true;
        same(
            &mut client,
            here,
            &long,
            &format!("ls -la {target}"),
            Out::plain(),
        );
    }

    // The long line of a Container of the Kernel's own, and of a cell
    // that offers an api, as the cli writes them.
    let mut long = args(&["/module"]);
    long.long = true;
    let modules = same(&mut client, here, &long, "ls -l /module", Out::plain());
    let fs = modules.lines().find(|l| l.ends_with("  fs")).unwrap();
    assert!(fs.starts_with("c rw  "), "{fs}");
}

#[test]
fn color_wears_the_kind_and_the_columns_count_the_name_only() {
    let rig = Rig::new("ls-color");
    rig.file("note", b"hi\n");
    let mut client = rig.client();
    let here = (ROOT, "/");

    let mut painted = args(&["/module"]);
    painted.color = true;
    painted.width = 20;
    let out = Out { width: Some(20), paint: Paint::on() };
    let two = same(&mut client, here, &painted, "ls /module", out);
    assert_eq!(
        two.lines().next().unwrap(),
        "\x1b[1;34mclipboard\x1b[0m  \x1b[1;34mfs\x1b[0m"
    );

    // A cell that offers an api is green, a plain one is as it is.
    let mut text = args(&["/module/text", "/module/fs/root"]);
    text.color = true;
    let out = Out { width: None, paint: Paint::on() };
    let said =
        same(&mut client, here, &text, "ls /module/text /module/fs/root", out);
    assert!(said.contains("\x1b[32mopen\x1b[0m\n"), "{said}");
    assert!(said.ends_with("note\n"), "{said}");
}

#[test]
fn a_target_is_walked_from_the_cwd_the_run_was_given() {
    let rig = Rig::new("ls-cwd");
    rig.file("note", b"hi\n");
    std::fs::create_dir(rig.dir().join("sub")).unwrap();
    std::fs::write(rig.dir().join("sub/deep"), b"x").unwrap();
    let mut client = rig.client();
    let root = client.resolve("/module/fs/root").unwrap();
    let at = (root, "/module/fs/root");

    let mut here = args(&[]);
    here.cwd = root;
    let listed = same(&mut client, at, &here, "ls", Out::plain());
    assert_eq!(listed, "note\nsub\n");
    for target in ["sub", "..", "./sub/../sub"] {
        let mut relative = args(&[target]);
        relative.cwd = root;
        same(&mut client, at, &relative, &format!("ls {target}"), Out::plain());
    }

    // An rd names the same Container wherever the line stands.
    let rd = format!("rd:{}", root.0);
    let mut named = args(&[&rd]);
    named.cwd = ROOT;
    let said = same(
        &mut client,
        (ROOT, "/"),
        &named,
        &format!("ls {rd}"),
        Out::plain(),
    );
    assert_eq!(said, "note\nsub\n");
}

#[test]
fn a_target_that_is_not_there_fails_the_run_and_leaves_no_cell() {
    let rig = Rig::new("ls-missing");
    let mut client = rig.client();
    let ls = client.resolve("/bin/ls").unwrap();

    assert_eq!(
        run(&mut client, &args(&["/module", "/nowhere"])),
        Err(Fault::Kernel(Error::Invalid))
    );
    assert_eq!(client.children(ls), Ok(Vec::new()), "a cell for a failed run");
}
