//! `cat` over the rig: what one run's output cell holds, beside the
//! bytes the cli's own `cat` prints of the same targets, and what a
//! target that holds no bytes does to the run.
//!
//! Under `f` the cell goes on, and the reads of it stand beside the
//! cli's own follower of the same line in the same rig, event for
//! event.

use reovim_client::{Client, Error, Fault, Handle, ROOT, Rd, Rights};
use reovim_client_cli::{Follow, Out, Shell, following, input};
use reovim_module_coreutils::bins::cat::Args;
use reovim_rig::Rig;

/// A run of the targets, as a shell fills the record: from the root.
fn args(targets: &[&str]) -> Args {
    Args {
        f: false,
        n: 0,
        cwd: ROOT,
        targets: targets.iter().map(|t| (*t).to_string()).collect(),
    }
}

/// `call /bin/cat run {..}`: the rd of the output cell, or the fault
/// the call answered with.
fn run(client: &mut Client, args: &Args) -> Result<Rd, Fault> {
    let cat = client.resolve("/bin/cat")?;
    let out = client.call_by_name(cat, "run", reovim_sdk::encode(args))?;
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

/// The run and the cli line that are the same bytes: those bytes, once
/// both have printed them. `at` is where the line stands, which the
/// cli keeps as a path and the run as the `cwd` of its record.
fn same(
    client: &mut Client,
    at: (Rd, &str),
    args: &Args,
    line: &str,
) -> String {
    let cell = run(client, args).unwrap();
    let said = drain(client, cell);
    let words: Vec<String> =
        line.split_whitespace().map(str::to_string).collect();
    let mut shell = Shell::at(at.0, at.1.to_string());
    let printed = input(client, &mut shell, &words, Out::plain()).unwrap();
    assert_eq!(said, String::from_utf8(printed).unwrap(), "{line}");
    said
}

/// The cli's own follower of `line`, begun in this rig beside the run
/// it is compared with.
fn planned(client: &mut Client, line: &str) -> Follow {
    let words: Vec<String> =
        line.split_whitespace().map(str::to_string).collect();
    following(client, &Shell::default(), &words, Out::plain()).unwrap().unwrap()
}

/// Everything the cli's follower prints of what has happened since it
/// was asked last.
fn printed(client: &mut Client, plan: &mut Follow) -> String {
    let mut said = Vec::new();
    for event in client.since().unwrap() {
        if let Some(bytes) = plan.event(client, event).unwrap() {
            said.extend(bytes);
        }
    }
    String::from_utf8(said).unwrap()
}

/// The cell read at `at`, as a shell reads a following one: the bytes,
/// or the fault it was sent back with.
fn next(client: &mut Client, h: Handle, at: u64) -> Result<String, Fault> {
    Ok(String::from_utf8(client.read(h, at, 64)?).unwrap())
}

/// The handles `/sys/core` says are watching `rd`.
fn watchers(client: &mut Client, rd: Rd) -> Vec<String> {
    let core = client.resolve("/sys/core").unwrap();
    let text = String::from_utf8(client.read_all(core).unwrap()).unwrap();
    let held = format!(" rd={} ", rd.0);
    text.lines()
        .filter(|line| line.starts_with("handle ") && line.contains(&held))
        .filter(|line| line.ends_with("watch=on"))
        .map(str::to_string)
        .collect()
}

#[test]
fn a_run_writes_the_bytes_of_each_target_in_turn() {
    let rig = Rig::new("cat-bytes");
    rig.file("one", b"first\n");
    rig.file("two", b"second\n");
    let mut client = rig.client();
    let root = "/module/fs/root";
    let here = (ROOT, "/");

    let one = format!("{root}/one");
    let two = format!("{root}/two");
    let said = same(&mut client, here, &args(&[&one]), &format!("cat {one}"));
    assert_eq!(said, "first\n");
    let both = args(&[&one, &two]);
    let said = same(&mut client, here, &both, &format!("cat {one} {two}"));
    assert_eq!(said, "first\nsecond\n");

    // A cell of the Kernel's own reads the same way a file does.
    let level =
        same(&mut client, here, &args(&["/sys/loglevel"]), "cat /sys/loglevel");
    assert!(!level.is_empty(), "the level the Kernel logs at");
}

#[test]
fn a_target_is_walked_from_the_cwd_the_run_was_given() {
    let rig = Rig::new("cat-cwd");
    rig.file("note", b"hi\n");
    std::fs::create_dir(rig.dir().join("sub")).unwrap();
    std::fs::write(rig.dir().join("sub/deep"), b"x").unwrap();
    let mut client = rig.client();
    let root = client.resolve("/module/fs/root").unwrap();
    let at = (root, "/module/fs/root");

    for target in ["note", "sub/deep", "./sub/../note"] {
        let mut relative = args(&[target]);
        relative.cwd = root;
        same(&mut client, at, &relative, &format!("cat {target}"));
    }
    // An rd names the same cell wherever the line stands.
    let note = client.resolve("/module/fs/root/note").unwrap();
    let rd = format!("rd:{}", note.0);
    let said =
        same(&mut client, (ROOT, "/"), &args(&[&rd]), &format!("cat {rd}"));
    assert_eq!(said, "hi\n");
}

#[test]
fn a_target_that_holds_no_bytes_fails_the_run_and_leaves_no_cell() {
    let rig = Rig::new("cat-refused");
    rig.file("note", b"hi\n");
    let mut client = rig.client();
    let cat = client.resolve("/bin/cat").unwrap();

    // A Container has no bytes, as the cli says of the same target.
    assert_eq!(
        run(&mut client, &args(&["/module/fs/root"])),
        Err(Fault::Kernel(Error::Unsupported))
    );
    // A run stops at the first target that fails, as the cli does.
    assert_eq!(
        run(&mut client, &args(&["/module/fs/root/note", "/nowhere"])),
        Err(Fault::Kernel(Error::Invalid))
    );
    assert_eq!(client.children(cat), Ok(Vec::new()), "a cell for a failed run");

    // Under `f` the targets are watched before they are read, and a run
    // that fails after that closes what it opened.
    let root = client.resolve("/module/fs/root").unwrap();
    let watching = Args { f: true, ..args(&["/module/fs/root"]) };
    assert_eq!(
        run(&mut client, &watching),
        Err(Fault::Kernel(Error::Unsupported))
    );
    assert_eq!(watchers(&mut client, root), Vec::<String>::new());
    assert_eq!(client.children(cat), Ok(Vec::new()));
}

#[test]
fn a_following_run_answers_what_a_second_client_writes() {
    let rig = Rig::new("cat-following");
    rig.file("note", b"first\n");
    let mut client = rig.client();
    let mut other = rig.client();
    let target = "/module/fs/root/note";
    let rd = client.resolve(target).unwrap();

    // Both watch before anything is written: the run, and the cli's own
    // follower of the same line.
    let cell = run(&mut client, &Args { f: true, ..args(&[target]) }).unwrap();
    let mut plan = planned(&mut client, &format!("cat -f {target}"));
    let h = client.open(cell, Rights::READ).unwrap();
    assert_eq!(next(&mut client, h, 0).unwrap(), "first\n");
    assert_eq!(plan.first, b"first\n".to_vec());

    // At the end of what the cell holds, with nothing new: not the end,
    // which an empty read would be.
    assert_eq!(next(&mut client, h, 6), Err(Fault::Kernel(Error::Busy)));

    other.write_all(rd, b"second\n".to_vec()).unwrap();
    let said = next(&mut client, h, 6).unwrap();
    assert_eq!(said, printed(&mut client, &mut plan));
    assert_eq!(said, "second\n");
    assert_eq!(next(&mut client, h, 13), Err(Fault::Kernel(Error::Busy)));

    // The cell grew where it stood: every offset stays readable.
    assert_eq!(next(&mut client, h, 0).unwrap(), "first\nsecond\n");
}

#[test]
fn a_run_that_counts_its_answers_ends_after_them() {
    let rig = Rig::new("cat-count");
    rig.file("note", b"first\n");
    let mut client = rig.client();
    let mut other = rig.client();
    let target = "/module/fs/root/note";
    let rd = client.resolve(target).unwrap();
    let cat = client.resolve("/bin/cat").unwrap();

    let counted = Args { f: true, n: 1, ..args(&[target]) };
    let cell = run(&mut client, &counted).unwrap();
    let mut plan = planned(&mut client, &format!("cat -f -n 1 {target}"));
    assert_eq!(plan.limit, Some(1));
    let h = client.open(cell, Rights::READ).unwrap();
    assert_eq!(next(&mut client, h, 0).unwrap(), "first\n");

    other.write_all(rd, b"second\n".to_vec()).unwrap();
    let said = next(&mut client, h, 6).unwrap();
    assert_eq!(said, printed(&mut client, &mut plan));
    assert_eq!(said, "second\n");
    // The one answer is spent, so the read past it is the empty read
    // that ends any cell.
    assert_eq!(next(&mut client, h, 13).unwrap(), "");

    client.close(h).unwrap();
    assert_eq!(client.children(cat), Ok(Vec::new()));
    assert_eq!(client.info(cell), Err(Fault::Kernel(Error::Invalid)));
}

#[test]
fn a_follower_does_nothing_between_two_reads_and_closes_whole() {
    let rig = Rig::new("cat-idle");
    rig.file("note", b"first\n");
    let mut client = rig.client();
    let mut other = rig.client();
    let target = "/module/fs/root/note";
    let rd = client.resolve(target).unwrap();
    let cat = client.resolve("/bin/cat").unwrap();

    let cell = run(&mut client, &Args { f: true, ..args(&[target]) }).unwrap();
    let h = client.open(cell, Rights::READ).unwrap();
    assert_eq!(next(&mut client, h, 0).unwrap(), "first\n");

    // The run read the target, so the atime stands at that read; it
    // moves again only at the read of the cell that reads it again.
    let idle = client.info(rd).unwrap().atime;
    other.write_all(rd, b"second\n".to_vec()).unwrap();
    assert_eq!(client.info(rd).unwrap().atime, idle, "nobody was reading");
    assert_eq!(next(&mut client, h, 6).unwrap(), "second\n");
    assert_ne!(client.info(rd).unwrap().atime, idle);

    // The close ends the follow whole: the Module's watch on the target
    // goes, the name lists no cell, and the rd is not there.
    assert_eq!(watchers(&mut client, rd).len(), 1);
    client.close(h).unwrap();
    assert_eq!(watchers(&mut client, rd), Vec::<String>::new());
    assert_eq!(client.children(cat), Ok(Vec::new()));
    assert_eq!(client.info(cell), Err(Fault::Kernel(Error::Invalid)));
}
