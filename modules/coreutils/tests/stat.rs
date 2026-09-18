//! `stat` over the rig: the long line, the rd, the times and every api
//! of one rd in one run's output cell, and what the cell is once its
//! reader lets go of it.

use reovim_client::{Client, Error, Fault, ROOT, Rd, Rights};
use reovim_module_coreutils::bins::stat::Args;
use reovim_rig::Rig;

/// `call /bin/stat run {cwd, color, target}`: the rd of the output cell, or
/// the fault the call answered with.
fn run(client: &mut Client, cwd: Rd, target: &str) -> Result<Rd, Fault> {
    let stat = client.resolve("/bin/stat")?;
    let args = Args { cwd, color: false, target: target.to_string() };
    let out = client.call_by_name(stat, "run", reovim_sdk::encode(&args))?;
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
fn text(client: &mut Client, cwd: Rd, target: &str) -> String {
    let cell = run(client, cwd, target).unwrap();
    drain(client, cell)
}

#[test]
fn a_run_says_the_long_line_the_rd_the_times_and_every_api() {
    let rig = Rig::new("stat-text");
    let mut client = rig.client();

    // A cell that offers an api: the long line, the rd, the three
    // times, then every method of every api as the declaration shows
    // it — the number, the types and the right.
    let said = text(&mut client, ROOT, "/module/text/open");
    let lines: Vec<&str> = said.lines().collect();
    assert!(lines[0].ends_with("  open  open"), "{said}");
    let open = client.resolve("/module/text/open").unwrap();
    assert_eq!(lines[1], format!("rd {}", open.0));
    for (at, key) in [(2, "ctime"), (3, "mtime"), (4, "atime")] {
        let (said, date) = lines[at].split_once(' ').unwrap();
        assert_eq!(said, key);
        assert!(date.starts_with("20") && date.len() == 19, "{key}: {date}");
    }
    assert_eq!(lines[5], "open v1");
    assert_eq!(lines[6], "  0 open(Rd) -> Rd WRITE");
    assert_eq!(lines[7], "  1 new() -> Rd WRITE");
    assert_eq!(lines.len(), 8, "{said}");

    // A Container with no api of its own stops after the times.
    let said = text(&mut client, ROOT, "/module/text");
    assert_eq!(said.lines().count(), 5, "{said}");

    // `stat`'s own declaration, as the shell reads it.
    let said = text(&mut client, ROOT, "/bin/stat");
    assert!(said.contains("\nstat v1\n"), "{said}");
    assert!(
        said.contains(
            "  0 run({cwd: Rd, color: bool, target: utf8(256)}) -> Rd READ\n"
        ),
        "{said}"
    );
}

#[test]
fn a_target_is_walked_from_the_cwd_the_run_was_given() {
    let rig = Rig::new("stat-cwd");
    let mut client = rig.client();
    let module = client.resolve("/module").unwrap();
    let absolute = run(&mut client, ROOT, "/module/text/open").unwrap();

    let relative = run(&mut client, module, "text/open").unwrap();
    let text = drain(&mut client, relative);
    assert_eq!(text, drain(&mut client, absolute));
    // The walk's own moves, from the same cwd.
    let up = run(&mut client, module, "../module/./text/open").unwrap();
    assert_eq!(drain(&mut client, up), text);
}

#[test]
fn a_target_that_is_not_there_fails_the_run_and_leaves_no_cell() {
    let rig = Rig::new("stat-missing");
    let mut client = rig.client();
    let stat = client.resolve("/bin/stat").unwrap();

    assert_eq!(
        run(&mut client, ROOT, "/nowhere"),
        Err(Fault::Kernel(Error::Invalid))
    );
    assert_eq!(
        client.children(stat),
        Ok(Vec::new()),
        "a cell for a failed run"
    );
}

#[test]
fn the_cell_is_gone_from_the_tree_once_its_reader_closes_it() {
    let rig = Rig::new("stat-close");
    let mut client = rig.client();
    let stat = client.resolve("/bin/stat").unwrap();

    let cell = run(&mut client, ROOT, "/module/text").unwrap();
    assert_eq!(client.children(stat), Ok(vec![cell]));
    assert!(client.info(cell).is_ok());

    drain(&mut client, cell);
    // The Kernel knows no rd of that number any more, and `stat`'s
    // listing is empty again.
    assert_eq!(client.info(cell), Err(Fault::Kernel(Error::Invalid)));
    assert_eq!(client.children(stat), Ok(Vec::new()));
}
