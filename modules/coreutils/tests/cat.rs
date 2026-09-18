//! `cat` over the rig: what one run's output cell holds, beside the
//! bytes the cli's own `cat` prints of the same targets, and what a
//! target that holds no bytes does to the run.

use reovim_client::{Client, Error, Fault, ROOT, Rd, Rights};
use reovim_client_cli::{Out, Shell, input};
use reovim_module_coreutils::bins::cat::Args;
use reovim_rig::Rig;

/// A run of the targets, as a shell fills the record: from the root.
fn args(targets: &[&str]) -> Args {
    Args {
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
}
