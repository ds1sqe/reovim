//! `date` and `uptime` over the rig: what each reader writes of the
//! fact it is a wording of, what a token given to a run that takes
//! nothing answers, and what the cell is once its reader closes it.

use reovim_client::{Client, Error, Fault, Rd, Rights};
use reovim_module_sysutils::bins::{date, uptime};
use reovim_rig::Rig;

/// `call /bin/<name> run {}`: the rd of the output cell, or the fault
/// the call answered with.
fn run(client: &mut Client, name: &str, input: Vec<u8>) -> Result<Rd, Fault> {
    let rd = client.resolve(&format!("/bin/{name}"))?;
    let out = client.call_by_name(rd, "run", input)?;
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

/// The number the fact at `path` reads as, which is what the reader of
/// it is held against.
fn fact(client: &mut Client, path: &str) -> u64 {
    let rd = client.resolve(path).unwrap();
    let bytes = client.read_all(rd).unwrap();
    String::from_utf8(bytes).unwrap().parse().unwrap()
}

/// `YYYY-MM-DD HH:MM:SS` of one instant of the Kernel clock, which is
/// the wording a long listing and `stat` write a time in.
fn civil(nanos: u64) -> String {
    let at = reovim_sdk::date::civil(nanos);
    let (year, month, day) = (at.year, at.month, at.day);
    format!(
        "{year:04}-{month:02}-{day:02} {:02}:{:02}:{:02}\n",
        at.hour, at.minute, at.second
    )
}

#[test]
fn a_run_of_date_says_the_civil_utc_of_the_clock() {
    let rig = Rig::new("date");
    let mut client = rig.client();

    // The clock is read before the run and after it, since a second
    // may have ticked between any two of the three reads.
    let before = fact(&mut client, "/sys/info/time");
    let cell = run(&mut client, "date", reovim_sdk::encode(&date::Args {}));
    let said = drain(&mut client, cell.unwrap());
    let after = fact(&mut client, "/sys/info/time");
    assert!(said >= civil(before) && said <= civil(after), "{said}");
}

#[test]
fn a_run_of_uptime_says_the_span_since_the_kernel_was_made() {
    let rig = Rig::new("uptime");
    let mut client = rig.client();
    let args = || reovim_sdk::encode(&uptime::Args {});

    // A rig is made a moment before it is asked, and the seconds stand
    // whatever they are: nothing is ever bare.
    let cell = run(&mut client, "uptime", args());
    let first = drain(&mut client, cell.unwrap());
    assert_eq!(first, "up 0h 0m 0s\n", "{first}");

    // A later run is no earlier than the one before it, which the
    // zeros of the first make a plain comparison.
    let cell = run(&mut client, "uptime", args());
    let second = drain(&mut client, cell.unwrap());
    assert!(second >= first, "{second} stands before {first}");
}

#[test]
fn a_token_is_no_field_of_a_run_that_takes_nothing() {
    let rig = Rig::new("info-token");
    let mut client = rig.client();

    // The record has no field, so there is nothing for a token to be:
    // bytes that are not the empty record decode as nothing.
    let token = reovim_sdk::encode(&"x".to_string());
    for name in ["date", "uptime"] {
        let refused = run(&mut client, name, token.clone());
        assert_eq!(refused, Err(Fault::Kernel(Error::Invalid)), "{name}");
    }
}

#[test]
fn the_cell_is_gone_from_the_tree_once_its_reader_closes_it() {
    let rig = Rig::new("info-close");
    let mut client = rig.client();
    let bin = client.resolve("/bin/uptime").unwrap();

    let args = reovim_sdk::encode(&uptime::Args {});
    let cell = run(&mut client, "uptime", args).unwrap();
    assert_eq!(client.children(bin), Ok(vec![cell]));

    drain(&mut client, cell);
    assert_eq!(client.info(cell), Err(Fault::Kernel(Error::Invalid)));
    assert_eq!(client.children(bin), Ok(Vec::new()));
}
