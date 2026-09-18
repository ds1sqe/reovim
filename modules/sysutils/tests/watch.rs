//! `watch` over the rig: a cell that holds nothing and goes on, whose
//! reads stand beside the cli's own follower of the same line in the
//! same rig, event for event.

use reovim_client::{Client, Error, Fault, Handle, ROOT, Rd, Rights};
use reovim_client_cli::{Follow, Out, Paint, Shell, following};
use reovim_module_sysutils::bins::watch::Args;
use reovim_rig::Rig;

/// A run on one target, as a shell fills the record: from the root, no
/// color.
fn args(target: &str) -> Args {
    Args { cwd: ROOT, color: false, target: target.to_string() }
}

/// `call /bin/watch run {..}`: the rd of the output cell, or the fault
/// the call answered with.
fn run(client: &mut Client, args: &Args) -> Result<Rd, Fault> {
    let watch = client.resolve("/bin/watch")?;
    let out = client.call_by_name(watch, "run", reovim_sdk::encode(args))?;
    Ok(reovim_sdk::decode(&out)?)
}

/// The cli's own follower of `line`, begun in this rig beside the run
/// it is compared with.
fn planned(client: &mut Client, line: &str, out: Out) -> Follow {
    let words: Vec<String> =
        line.split_whitespace().map(str::to_string).collect();
    following(client, &Shell::default(), &words, out).unwrap().unwrap()
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

/// Everything the cell holds from `at` on, read as a shell reads a
/// following one, to the `Busy` that says it has nothing more yet.
fn so_far(client: &mut Client, h: Handle, at: u64) -> String {
    let mut bytes: Vec<u8> = Vec::new();
    loop {
        let read = client.read(h, at + bytes.len() as u64, 64);
        let Ok(chunk) = read else {
            assert_eq!(read, Err(Fault::Kernel(Error::Busy)));
            return String::from_utf8(bytes).unwrap();
        };
        assert!(!chunk.is_empty(), "a follower is never over");
        bytes.extend(chunk);
    }
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
fn a_run_says_what_happened_under_a_container() {
    let rig = Rig::new("watch");
    rig.file("note", b"hi\n");
    let mut client = rig.client();
    let mut other = rig.client();
    let root = "/module/fs/root";
    let rd = client.resolve(root).unwrap();
    // The children stand in the tree before either follower begins: a
    // first listing registers them, and a registration is an event.
    client.children(rd).unwrap();

    let cell = run(&mut client, &args(root)).unwrap();
    let mut plan = planned(&mut client, &format!("watch {root}"), Out::plain());
    let h = client.open(cell, Rights::READ).unwrap();
    // A cell that holds nothing is not over: nothing has happened yet.
    assert_eq!(client.read(h, 0, 64), Err(Fault::Kernel(Error::Busy)));

    let made = reovim_sdk::encode(&"made".to_string());
    other.call_by_name(rd, "create", made).unwrap();
    let said = so_far(&mut client, h, 0);
    assert_eq!(said, printed(&mut client, &mut plan));

    // The Create of the new child, with its own seq, and nothing the
    // ring held before the run began.
    let line = said.lines().find(|line| line.contains("Create")).unwrap();
    let words: Vec<&str> = line.split(' ').collect();
    assert_eq!((words[1], words[2]), ("Create", "made"), "{said}");
    assert!(words[0].parse::<u64>().is_ok(), "{said}");
    assert!(!said.contains("Attrib"), "the backlog is not ours: {said}");
}

#[test]
fn color_paints_the_kind_of_what_happened() {
    let rig = Rig::new("watch-color");
    let mut client = rig.client();
    let mut other = rig.client();
    let root = "/module/fs/root";
    let rd = client.resolve(root).unwrap();
    client.children(rd).unwrap();

    let painted = Args { color: true, ..args(root) };
    let cell = run(&mut client, &painted).unwrap();
    let line = format!("watch {root}");
    let lit = Out { width: None, paint: Paint::on() };
    let mut plan = planned(&mut client, &line, lit);
    let h = client.open(cell, Rights::READ).unwrap();

    let made = reovim_sdk::encode(&"made".to_string());
    other.call_by_name(rd, "create", made).unwrap();
    let said = so_far(&mut client, h, 0);
    assert_eq!(said, printed(&mut client, &mut plan));
    assert!(said.contains("\x1b[32mCreate\x1b[0m made\n"), "{said}");
}

#[test]
fn the_close_takes_the_watch_the_cell_and_the_rd() {
    let rig = Rig::new("watch-close");
    let mut client = rig.client();
    let root = "/module/fs/root";
    let rd = client.resolve(root).unwrap();
    let watch = client.resolve("/bin/watch").unwrap();

    let cell = run(&mut client, &args(root)).unwrap();
    let h = client.open(cell, Rights::READ).unwrap();
    assert_eq!(watchers(&mut client, rd).len(), 1);

    client.close(h).unwrap();
    assert_eq!(watchers(&mut client, rd), Vec::<String>::new());
    assert_eq!(client.children(watch), Ok(Vec::new()));
    assert_eq!(client.info(cell), Err(Fault::Kernel(Error::Invalid)));
}

#[test]
fn a_target_that_is_not_there_fails_the_run_and_leaves_no_cell() {
    let rig = Rig::new("watch-missing");
    let mut client = rig.client();
    let watch = client.resolve("/bin/watch").unwrap();

    assert_eq!(
        run(&mut client, &args("/nowhere")),
        Err(Fault::Kernel(Error::Invalid))
    );
    assert_eq!(client.children(watch), Ok(Vec::new()));
}
