//! `grep` over the rig: the lines of each target the pattern is found
//! in, the name a second target puts before them, what a pattern the
//! matcher has not does to the run, and the dialect being Vim's.

use reovim_client::{Client, Error, Fault, ROOT, Rd, Rights};
use reovim_module_coreutils::bins::grep::Args;
use reovim_rig::Rig;

/// A run of the pattern over the targets, as a shell fills the record:
/// from the root.
fn args(pattern: &str, targets: &[&str]) -> Args {
    Args {
        cwd: ROOT,
        pattern: pattern.to_string(),
        targets: targets.iter().map(|t| (*t).to_string()).collect(),
    }
}

/// `call /bin/grep run {..}`: the rd of the output cell, or the fault
/// the call answered with.
fn run(client: &mut Client, args: &Args) -> Result<Rd, Fault> {
    let grep = client.resolve("/bin/grep")?;
    let out = client.call_by_name(grep, "run", reovim_sdk::encode(args))?;
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

/// The lines one run's output cell holds, read to its end.
fn text(client: &mut Client, args: &Args) -> String {
    let cell = run(client, args).unwrap();
    drain(client, cell)
}

#[test]
fn a_run_writes_every_line_of_a_target_the_pattern_is_found_in() {
    let rig = Rig::new("grep-lines");
    rig.file("one", b"alpha\nbeta\ngamma\n");
    let mut client = rig.client();
    let one = "/module/fs/root/one";

    assert_eq!(text(&mut client, &args("ph", &[one])), "alpha\n");
    assert_eq!(text(&mut client, &args("^beta$", &[one])), "beta\n");
    // Every line a match stands in, in the order they stand.
    let all = "alpha\nbeta\ngamma\n";
    assert_eq!(text(&mut client, &args("a", &[one])), all);
    // A line is what a `\n` ends, so the last one needs none of its own.
    rig.file("two", b"delta\nepsilon");
    let two = "/module/fs/root/two";
    assert_eq!(text(&mut client, &args("psi", &[two])), "epsilon\n");
    // Nothing found is an empty cell, which is nothing printed.
    assert_eq!(text(&mut client, &args("zeta", &[one])), "");
    // No target at all is nowhere to look.
    assert_eq!(text(&mut client, &args("a", &[])), "");
}

#[test]
fn more_than_one_target_puts_each_targets_name_before_its_lines() {
    let rig = Rig::new("grep-named");
    rig.file("one", b"alpha\nbeta\n");
    rig.file("two", b"phi\nalpha\n");
    let mut client = rig.client();
    let root = client.resolve("/module/fs/root").unwrap();

    // One target names nothing: the lines are its own.
    let mut here = args("ph", &["one"]);
    here.cwd = root;
    assert_eq!(text(&mut client, &here), "alpha\n");

    // Two name each line, which is the only thing that makes them
    // readable, and each target is walked from the cwd of the record.
    let mut both = args("ph", &["one", "two"]);
    both.cwd = root;
    let said = "one:alpha\ntwo:phi\ntwo:alpha\n";
    assert_eq!(text(&mut client, &both), said);
}

#[test]
fn a_pattern_or_a_target_the_run_cannot_take_leaves_no_cell() {
    let rig = Rig::new("grep-refused");
    rig.file("one", b"alpha\n");
    let mut client = rig.client();
    let grep = client.resolve("/bin/grep").unwrap();
    let one = "/module/fs/root/one";

    // The pattern is parsed before any target is read, so a pattern the
    // matcher has not is the answer even where the target is not there.
    for target in [one, "/nowhere"] {
        assert_eq!(
            run(&mut client, &args("a\\{2}", &[target])),
            Err(Fault::Kernel(Error::Invalid)),
            "{target}"
        );
    }
    // A Container holds no bytes, and a run stops at the first target
    // that fails.
    assert_eq!(
        run(&mut client, &args("a", &["/module/fs/root"])),
        Err(Fault::Kernel(Error::Unsupported))
    );
    assert_eq!(
        run(&mut client, &args("a", &[one, "/nowhere"])),
        Err(Fault::Kernel(Error::Invalid))
    );
    assert_eq!(
        client.children(grep),
        Ok(Vec::new()),
        "a cell for a failed run"
    );
}

#[test]
fn the_dialect_is_vims_and_not_posix() {
    let rig = Rig::new("grep-dialect");
    rig.file("one", b"fn main\nfnb\nAlpha\naaa\nax\n");
    let mut client = rig.client();
    let one = "/module/fs/root/one";
    let found = |client: &mut Client, pattern: &str| {
        text(client, &args(pattern, &[one]))
    };

    // `\<` and `\>` are the ends of a name; `\b` is no atom of this
    // dialect, so it is the letter it spells.
    assert_eq!(found(&mut client, "\\<fn\\>"), "fn main\n");
    assert_eq!(found(&mut client, "fn\\b"), "fnb\n");
    // One or more is `\+`, a group is `\(...\)` and its alternatives
    // are `\|`; the POSIX spelling of each is the literal it reads as.
    assert_eq!(found(&mut client, "a\\+a"), "aaa\n");
    assert_eq!(found(&mut client, "a+a"), "");
    assert_eq!(found(&mut client, "\\(a\\|b\\)x"), "ax\n");
    assert_eq!(found(&mut client, "(a|b)x"), "");
    // Case is the pattern's own: `\c` asks for case-blindness, there
    // being no flag to ask with.
    assert_eq!(found(&mut client, "alpha"), "");
    assert_eq!(found(&mut client, "\\calpha"), "Alpha\n");
}
