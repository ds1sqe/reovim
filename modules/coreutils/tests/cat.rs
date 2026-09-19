//! `cat` over the rig: the bytes of each target in turn in one run's
//! output cell, and what a target that holds no bytes does to the run.
//!
//! Under `f` the cell goes on, and a read at its end answers whatever a
//! second client wrote into the target since the read before.

use reovim_client::{Error, Fault, ROOT, Rights};
use reovim_module_coreutils::bins::cat::Args;
use reovim_rig::Rig;
use reovim_rig::bins::{next, run, text, watchers};

/// A run of the targets, as a shell fills the record: from the root.
fn args(targets: &[&str]) -> Args {
    Args {
        f: false,
        n: 0,
        cwd: ROOT,
        targets: targets.iter().map(|t| (*t).to_string()).collect(),
    }
}

#[test]
fn a_run_writes_the_bytes_of_each_target_in_turn() {
    let rig = Rig::new("cat-bytes");
    rig.file("one", b"first\n");
    rig.file("two", b"second\n");
    let mut client = rig.client();
    let root = "/module/fs/root";

    let one = format!("{root}/one");
    let two = format!("{root}/two");
    assert_eq!(text(&mut client, "cat", &args(&[&one])), "first\n");
    assert_eq!(
        text(&mut client, "cat", &args(&[&one, &two])),
        "first\nsecond\n"
    );

    // A cell of the Kernel's own reads the same way a file does.
    let level = text(&mut client, "cat", &args(&["/sys/loglevel"]));
    assert_eq!(level, "info", "the level the Kernel logs at");
}

#[test]
fn a_target_is_walked_from_the_cwd_the_run_was_given() {
    let rig = Rig::new("cat-cwd");
    rig.file("note", b"hi\n");
    std::fs::create_dir(rig.dir().join("sub")).unwrap();
    std::fs::write(rig.dir().join("sub/deep"), b"x").unwrap();
    let mut client = rig.client();
    let root = client.resolve("/module/fs/root").unwrap();

    for (target, bytes) in
        [("note", "hi\n"), ("sub/deep", "x"), ("./sub/../note", "hi\n")]
    {
        let mut relative = args(&[target]);
        relative.cwd = root;
        assert_eq!(text(&mut client, "cat", &relative), bytes, "{target}");
    }
    // An rd names the same cell wherever the line stands.
    let note = client.resolve("/module/fs/root/note").unwrap();
    let rd = format!("rd:{}", note.0);
    assert_eq!(text(&mut client, "cat", &args(&[&rd])), "hi\n");
}

#[test]
fn a_target_that_holds_no_bytes_fails_the_run_and_leaves_no_cell() {
    let rig = Rig::new("cat-refused");
    rig.file("note", b"hi\n");
    let mut client = rig.client();
    let cat = client.resolve("/bin/cat").unwrap();

    // A Container has no bytes.
    assert_eq!(
        run(&mut client, "cat", &args(&["/module/fs/root"])),
        Err(Fault::Kernel(Error::Unsupported))
    );
    // A run stops at the first target that fails.
    assert_eq!(
        run(&mut client, "cat", &args(&["/module/fs/root/note", "/nowhere"])),
        Err(Fault::Kernel(Error::Invalid))
    );
    assert_eq!(client.children(cat), Ok(Vec::new()), "a cell for a failed run");

    // Under `f` the targets are watched before they are read, and a run
    // that fails after that closes what it opened.
    let root = client.resolve("/module/fs/root").unwrap();
    let watching = Args { f: true, ..args(&["/module/fs/root"]) };
    assert_eq!(
        run(&mut client, "cat", &watching),
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

    // The target is watched before it is read, so nothing written
    // between the two is missed.
    let cell =
        run(&mut client, "cat", &Args { f: true, ..args(&[target]) }).unwrap();
    let h = client.open(cell, Rights::READ).unwrap();
    assert_eq!(next(&mut client, h, 0).unwrap(), "first\n");

    // At the end of what the cell holds, with nothing new: not the end,
    // which an empty read would be.
    assert_eq!(next(&mut client, h, 6), Err(Fault::Kernel(Error::Busy)));

    other.write_all(rd, b"second\n".to_vec()).unwrap();
    assert_eq!(next(&mut client, h, 6).unwrap(), "second\n");
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
    let cell = run(&mut client, "cat", &counted).unwrap();
    let h = client.open(cell, Rights::READ).unwrap();
    assert_eq!(next(&mut client, h, 0).unwrap(), "first\n");

    other.write_all(rd, b"second\n".to_vec()).unwrap();
    assert_eq!(next(&mut client, h, 6).unwrap(), "second\n");
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

    let cell =
        run(&mut client, "cat", &Args { f: true, ..args(&[target]) }).unwrap();
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
