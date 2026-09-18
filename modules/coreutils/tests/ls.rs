//! `ls` over the rig: the listing one run's output cell holds of each
//! target, laid out across the width its record carries and painted by
//! its `color`.
//!
//! Under `w` the cell goes on, and a read at its end answers the
//! listing again for every event under one of the targets.

use reovim_client::log::{self, Entry};
use reovim_client::{Client, Error, Fault, Handle, ROOT, Rd, Rights};
use reovim_module_coreutils::bins::ls::Args;
use reovim_rig::Rig;

/// A run of the targets, as a shell fills the record: no flag, no
/// width, no color, from the root.
fn args(targets: &[&str]) -> Args {
    Args {
        long: false,
        all: false,
        w: false,
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

/// The text one run's output cell holds, read to its end.
fn text(client: &mut Client, args: &Args) -> String {
    let cell = run(client, args).unwrap();
    drain(client, cell)
}

/// The cell read at `at`, as a shell reads a following one: the bytes,
/// or the fault it was sent back with.
fn next(client: &mut Client, h: Handle, at: u64) -> Result<String, Fault> {
    Ok(String::from_utf8(client.read(h, at, 64)?).unwrap())
}

/// The id of the caller the Module named `name` submits its own ops as,
/// as `/sys/core` says it.
fn caller_of(client: &mut Client, name: &str) -> u64 {
    let core = client.resolve("/sys/core").unwrap();
    let text = String::from_utf8(client.read_all(core).unwrap()).unwrap();
    let named = format!(" name={name} ");
    let mine =
        |line: &&str| line.starts_with("module ") && line.contains(&named);
    let line = text.lines().find(mine).unwrap();
    line.rsplit_once("caller=").unwrap().1.parse().unwrap()
}

/// The Kernel's own trace lines of the ops `caller` has submitted; the
/// trace is kept at `debug` and `/sys/log` writes none of its own.
fn traced(client: &mut Client, caller: u64) -> Vec<String> {
    let mark = format!(" caller={caller} ");
    log::tail(client, 0)
        .unwrap()
        .iter()
        .map(Entry::text)
        .filter(|text| text.contains(&mark))
        .collect()
}

#[test]
fn a_listing_is_laid_out_down_the_columns_that_fit_the_width() {
    let rig = Rig::new("ls-columns");
    for name in ["gamma", "alpha", "beta"] {
        rig.file(name, b"");
    }
    let mut client = rig.client();
    let root = "/module/fs/root";

    // No width is one name a line, as a pipe wants them; a width lays
    // them out down the columns that fit in it.
    assert_eq!(text(&mut client, &args(&[root])), "alpha\nbeta\ngamma\n");
    for (width, laid) in
        [(80, "alpha  beta  gamma\n"), (12, "alpha  gamma\nbeta\n")]
    {
        let mut wide = args(&[root]);
        wide.width = width;
        assert_eq!(text(&mut client, &wide), laid, "width {width}");
    }

    // No target at all is the `cwd` the run was given; a target that
    // is no Container lists itself.
    let all = text(&mut client, &args(&[]));
    assert_eq!(all, "bin\nmodule\nsession\nsys\nvar\n");
    let one = args(&[&format!("{root}/beta")]);
    assert_eq!(text(&mut client, &one), "beta\n");

    // More than one target names each listing, as `ls` does.
    let listed = text(&mut client, &args(&["/module", root]));
    assert_eq!(
        listed,
        format!(
            "/module:\ncoreutils\nfs\nsysutils\ntext\n\n\
             {root}:\nalpha\nbeta\ngamma\n"
        )
    );

    // A Container with nothing under it prints nothing, laid out
    // across a width as much as down a pipe.
    std::fs::create_dir(rig.dir().join("empty")).unwrap();
    let empty = format!("{root}/empty");
    assert_eq!(text(&mut client, &args(&[&empty])), "");
    let mut wide = args(&[&empty]);
    wide.width = 80;
    assert_eq!(text(&mut client, &wide), "");
}

#[test]
fn the_long_line_and_the_rds_are_the_flags_the_record_carries() {
    let rig = Rig::new("ls-long");
    rig.file("note", b"hi\n");
    let mut client = rig.client();
    let root = "/module/fs/root";

    // The long line: type, rights, mtime, apis, name. A Container of
    // the Kernel's own, a plain cell, and a cell that offers an api.
    let modules = text(&mut client, &Args { long: true, ..args(&["/module"]) });
    let fs = modules.lines().find(|l| l.ends_with("  fs")).unwrap();
    assert!(fs.starts_with("c rw  "), "{fs}");
    let note = text(&mut client, &Args { long: true, ..args(&[root]) });
    let fields: Vec<&str> = note.trim_end().split("  ").collect();
    assert_eq!(fields.len(), 4, "{note}");
    assert_eq!((fields[0], fields[2], fields[3]), ("- rw", "-", "note"));
    assert!(fields[1].starts_with("20") && fields[1].len() == 19, "{note}");
    let text_module =
        text(&mut client, &Args { long: true, ..args(&["/module/text"]) });
    assert!(text_module.trim_end().ends_with("  open  open"), "{text_module}");

    // `-a` puts the rd number first, in both forms.
    let rd = client.resolve(&format!("{root}/note")).unwrap();
    let rds = text(&mut client, &Args { all: true, ..args(&[root]) });
    assert_eq!(rds, format!("{} note\n", rd.0));
    let long = Args { long: true, all: true, ..args(&[root]) };
    let both = text(&mut client, &long);
    assert!(both.starts_with(&format!("{} - rw  ", rd.0)), "{both}");
}

#[test]
fn color_wears_the_kind_and_the_columns_count_the_name_only() {
    let rig = Rig::new("ls-color");
    rig.file("note", b"hi\n");
    let mut client = rig.client();

    let mut painted = args(&["/module"]);
    painted.color = true;
    painted.width = 20;
    let two = text(&mut client, &painted);
    assert_eq!(
        two.lines().next().unwrap(),
        "\x1b[1;34mcoreutils\x1b[0m  \x1b[1;34msysutils\x1b[0m"
    );

    // A cell that offers an api is green, a plain one is as it is.
    let mut both = args(&["/module/text", "/module/fs/root"]);
    both.color = true;
    let said = text(&mut client, &both);
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

    let mut here = args(&[]);
    here.cwd = root;
    assert_eq!(text(&mut client, &here), "note\nsub\n");
    for (target, listed) in
        [("sub", "deep\n"), ("..", "root\n"), ("./sub/../sub", "deep\n")]
    {
        let mut relative = args(&[target]);
        relative.cwd = root;
        assert_eq!(text(&mut client, &relative), listed, "{target}");
    }

    // An rd names the same Container wherever the line stands.
    let rd = format!("rd:{}", root.0);
    let mut named = args(&[&rd]);
    named.cwd = ROOT;
    assert_eq!(text(&mut client, &named), "note\nsub\n");
}

#[test]
fn a_target_that_is_not_there_fails_the_run_and_leaves_no_cell() {
    let rig = Rig::new("ls-missing");
    let mut client = rig.client();
    let ls = client.resolve("/bin/ls").unwrap();

    // The Kernel's own root answers `Invalid` for a name it does not
    // hold, which is what a host prints as `ls /nothere: invalid`; fs
    // answers `Gone`, the pair modules/sysutils/tests/watch.rs holds.
    assert_eq!(
        run(&mut client, &args(&["/module", "/nowhere"])),
        Err(Fault::Kernel(Error::Invalid))
    );
    assert_eq!(client.children(ls), Ok(Vec::new()), "a cell for a failed run");
}

#[test]
fn a_watching_run_lists_again_for_every_event_under_its_target() {
    let rig = Rig::new("ls-watching");
    rig.file("note", b"hi\n");
    let mut client = rig.client();
    let mut other = rig.client();
    let root = "/module/fs/root";
    let rd = client.resolve(root).unwrap();
    // The children stand in the tree before either follower begins: a
    // first listing registers them, and a registration is an event.
    client.children(rd).unwrap();

    // The targets are watched before the first listing, so a child that
    // appears between the two is not missed.
    let cell = run(&mut client, &Args { w: true, ..args(&[root]) }).unwrap();
    let h = client.open(cell, Rights::READ).unwrap();
    assert_eq!(next(&mut client, h, 0).unwrap(), "note\n");
    assert_eq!(next(&mut client, h, 5), Err(Fault::Kernel(Error::Busy)));

    let made = reovim_sdk::encode(&"made".to_string());
    other.call_by_name(rd, "create", made).unwrap();
    assert_eq!(next(&mut client, h, 5).unwrap(), "made\nnote\n");
    assert_eq!(next(&mut client, h, 15), Err(Fault::Kernel(Error::Busy)));
}

#[test]
fn a_watching_run_submits_nothing_while_nobody_reads() {
    let rig = Rig::new("ls-idle");
    rig.file("note", b"hi\n");
    let mut client = rig.client();
    let mut other = rig.client();
    let root = "/module/fs/root";
    let rd = client.resolve(root).unwrap();
    let ls = client.resolve("/bin/ls").unwrap();
    client.children(rd).unwrap();

    // A listing leaves no atime, so the trace is what says the Module
    // stood still; it is written at `debug`.
    let level = client.resolve("/sys/loglevel").unwrap();
    client.write_all(level, b"debug".to_vec()).unwrap();
    let module = caller_of(&mut client, "coreutils");

    let cell = run(&mut client, &Args { w: true, ..args(&[root]) }).unwrap();
    let h = client.open(cell, Rights::READ).unwrap();
    assert_eq!(next(&mut client, h, 0).unwrap(), "note\n");

    let idle = traced(&mut client, module).len();
    let made = reovim_sdk::encode(&"made".to_string());
    other.call_by_name(rd, "create", made).unwrap();
    // Nothing delivers the event into the Driver, and the watch costs
    // nothing until the next read of the cell asks for it.
    assert_eq!(traced(&mut client, module).len(), idle, "nobody was reading");
    assert_eq!(next(&mut client, h, 5).unwrap(), "made\nnote\n");
    assert!(traced(&mut client, module).len() > idle);

    client.close(h).unwrap();
    assert_eq!(client.children(ls), Ok(Vec::new()));
    assert_eq!(client.info(cell), Err(Fault::Kernel(Error::Invalid)));
}
