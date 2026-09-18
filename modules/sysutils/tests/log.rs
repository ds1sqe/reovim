//! `log` over the rig: what one run's output cell holds, beside the
//! lines the cli's own `log` prints of the same `/sys/log`, and what
//! the cell is once its reader lets go of it.

use reovim_client::{Client, Error, Fault, Rd, Rights};
use reovim_client_cli::{Out, Shell, input};
use reovim_module_sysutils::bins::log::Args;
use reovim_rig::Rig;

/// `call /bin/log run {from}`: the rd of the output cell, or the fault
/// the call answered with.
fn run(client: &mut Client, from: u64) -> Result<Rd, Fault> {
    let log = client.resolve("/bin/log")?;
    let out =
        client.call_by_name(log, "run", reovim_sdk::encode(&Args { from }))?;
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

/// What the cli's own `log` prints, for the comparison.
fn cli(client: &mut Client, args: &[&str]) -> String {
    let mut words = vec!["log".to_string()];
    words.extend(args.iter().map(ToString::to_string));
    let said = input(client, &mut Shell::default(), &words, Out::plain());
    String::from_utf8(said.unwrap()).unwrap()
}

#[test]
fn a_run_says_what_the_cli_says_of_the_same_log() {
    let rig = Rig::new("log-lines");
    let mut client = rig.client();

    // Nothing either run does is written at the level the Kernel logs
    // at, so the two read the same entries: `seq time mark level text`.
    let cell = run(&mut client, 0).unwrap();
    let said = drain(&mut client, cell);
    assert_eq!(said, cli(&mut client, &[]));
    assert!(said.starts_with("1 "), "{said}");
    assert!(said.contains(" kernel 6 load module=fs\n"), "{said}");

    // From a sequence on, as the cli reads it from the same one.
    let last = said.lines().next_back().unwrap();
    let seq: u64 = last.split(' ').next().unwrap().parse().unwrap();
    let cell = run(&mut client, seq).unwrap();
    let from = drain(&mut client, cell);
    assert_eq!(from, cli(&mut client, &[&seq.to_string()]));
    assert_eq!(from.lines().count(), 1, "{from}");

    // Past the last sequence there is nothing to say.
    let cell = run(&mut client, seq + 1).unwrap();
    assert_eq!(drain(&mut client, cell), "");
}

#[test]
fn the_cell_is_gone_from_the_tree_once_its_reader_closes_it() {
    let rig = Rig::new("log-close");
    let mut client = rig.client();
    let log = client.resolve("/bin/log").unwrap();

    let cell = run(&mut client, 0).unwrap();
    assert_eq!(client.children(log), Ok(vec![cell]));

    drain(&mut client, cell);
    assert_eq!(client.info(cell), Err(Fault::Kernel(Error::Invalid)));
    assert_eq!(client.children(log), Ok(Vec::new()));
}
