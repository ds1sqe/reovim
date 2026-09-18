//! `stat` over the rig: what one run's output cell holds, beside the
//! text the cli's own `stat` prints of the same rd in the same tree,
//! and what the cell is once its reader lets go of it.

use reovim_bin_stat::Args;
use reovim_client::{Client, Error, Fault, ROOT, Rd, Rights};
use reovim_client_cli::{Out, Paint, Shell, input};
use reovim_rig::Rig;

/// `call /bin/stat run {cwd, target}`: the rd of the output cell, or
/// the fault the call answered with.
fn run(client: &mut Client, cwd: Rd, target: &str) -> Result<Rd, Fault> {
    let stat = client.resolve("/bin/stat")?;
    let args = Args { cwd, target: target.to_string() };
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

/// What the cli's own `stat` prints of `target`, for the comparison.
fn cli(client: &mut Client, target: &str) -> String {
    let words = ["stat".to_string(), target.to_string()];
    let out = Out { width: None, paint: Paint::off() };
    let said = input(client, &mut Shell::default(), &words, out).unwrap();
    String::from_utf8(said).unwrap()
}

#[test]
fn a_run_says_what_the_cli_says_of_the_same_rd() {
    let rig = Rig::new("stat-text");
    let mut client = rig.client();

    // A Container with no api of its own, and a cell with one: the
    // long line, the times, then every method of every api.
    for target in ["/module/text", "/module/text/open", "/bin/stat"] {
        let cell = run(&mut client, ROOT, target).unwrap();
        assert_eq!(
            drain(&mut client, cell),
            cli(&mut client, target),
            "{target}"
        );
    }

    // `stat`'s own declaration, as the shell will read it.
    let said = cli(&mut client, "/bin/stat");
    assert!(said.contains("\nstat v1\n"), "{said}");
    assert!(
        said.contains("  0 run({cwd: Rd, target: utf8(256)}) -> Rd READ\n"),
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
