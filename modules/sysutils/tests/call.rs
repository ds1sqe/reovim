//! `call` over the rig: what one run's output cell holds of what the
//! target's method answered, and what a method that is not there, or an
//! argument that does not fit, answers.

use reovim_client::{Client, Error, Fault, ROOT, Rd, Rights};
use reovim_module_sysutils::bins::call::Args;
use reovim_rig::Rig;

/// A run of `method` on `target`, from the root, with no argument.
fn args(target: &str, method: &str) -> Args {
    Args {
        cwd: ROOT,
        target: target.to_string(),
        method: method.to_string(),
        arg: String::new(),
    }
}

/// `call /bin/call run {..}`: the rd of the output cell, or the fault
/// the call answered with.
fn run(client: &mut Client, args: &Args) -> Result<Rd, Fault> {
    let call = client.resolve("/bin/call")?;
    let out = client.call_by_name(call, "run", reovim_sdk::encode(args))?;
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

#[test]
fn a_run_answers_what_the_method_answered() {
    let rig = Rig::new("call-run");
    rig.file("note", b"hi\n");
    let mut client = rig.client();
    let note = client.resolve("/module/fs/root/note").unwrap();

    // One Buffer per source, so a second run of the same method answers
    // that same rd, decoded by the declaration that says it is one.
    let open =
        Args { arg: note.0.to_string(), ..args("/module/text/open", "open") };
    let cell = run(&mut client, &open).unwrap();
    let said = drain(&mut client, cell);
    let buffer: u64 = said.trim_end().parse().unwrap_or_else(|_| {
        panic!("an rd: {said}");
    });
    assert_eq!(said, format!("{buffer}\n"));
    let cell = run(&mut client, &open).unwrap();
    assert_eq!(drain(&mut client, cell), said);

    // A method that answers nothing says nothing.
    let save = args(&format!("rd:{buffer}"), "save");
    let cell = run(&mut client, &save).unwrap();
    assert_eq!(drain(&mut client, cell), "");
    assert_eq!(rig.disk("note"), b"hi\n");

    // `new` is a Buffer of its own per run, so two runs answer two rds,
    // both printed the same way.
    let new = args("/module/text/open", "new");
    let cell = run(&mut client, &new).unwrap();
    let made = drain(&mut client, cell);
    assert!(client.info(Rd(made.trim_end().parse().unwrap())).is_ok());
    let cell = run(&mut client, &new).unwrap();
    let other = drain(&mut client, cell);
    assert_ne!(made, other);
    assert!(other.trim_end().parse::<u64>().is_ok(), "an rd: {other}");

    // An argument the declaration takes, and a relative target walked
    // from the `cwd` the run was given.
    let root = client.resolve("/module/fs/root").unwrap();
    let create =
        Args { cwd: root, arg: "made".to_string(), ..args(".", "create") };
    let cell = run(&mut client, &create).unwrap();
    let rd = drain(&mut client, cell);
    assert!(rd.trim_end().parse::<u64>().is_ok(), "an rd: {rd}");
    assert!(rig.dir().join("made").exists());
}

#[test]
fn a_method_that_is_not_there_fails_the_run_and_leaves_no_cell() {
    let rig = Rig::new("call-missing");
    let mut client = rig.client();
    let call = client.resolve("/bin/call").unwrap();

    assert_eq!(
        run(&mut client, &args("/module/fs/root", "nothing")),
        Err(Fault::Kernel(Error::Unsupported))
    );
    // An rd that offers no api at all answers the same way.
    assert_eq!(
        run(&mut client, &args("/sys/info/mem", "nothing")),
        Err(Fault::Kernel(Error::Unsupported))
    );
    assert_eq!(client.children(call), Ok(Vec::new()));
}

#[test]
fn an_argument_that_does_not_fit_the_declaration_fails_the_run() {
    let rig = Rig::new("call-arg");
    let mut client = rig.client();
    let call = client.resolve("/bin/call").unwrap();

    // A record has no one-argument spelling, so a run of another
    // Driver under `/bin` is `Invalid` until the shell can spell one.
    assert_eq!(
        run(&mut client, &args("/bin/stat", "run")),
        Err(Fault::Kernel(Error::Invalid))
    );
    // The fs Driver refuses a name with a slash in it, and the run
    // fails with what the method failed with.
    let slash =
        Args { arg: "a/b".to_string(), ..args("/module/fs/root", "create") };
    assert_eq!(run(&mut client, &slash), Err(Fault::Kernel(Error::Invalid)));
    assert_eq!(client.children(call), Ok(Vec::new()));
}

#[test]
fn the_cell_is_gone_from_the_tree_once_its_reader_closes_it() {
    let rig = Rig::new("call-close");
    let mut client = rig.client();
    let driver = client.resolve("/bin/call").unwrap();

    let cell = run(&mut client, &args("/module/text/open", "new")).unwrap();
    assert_eq!(client.children(driver), Ok(vec![cell]));

    drain(&mut client, cell);
    assert_eq!(client.info(cell), Err(Fault::Kernel(Error::Invalid)));
    assert_eq!(client.children(driver), Ok(Vec::new()));
}
