//! `dmesg` over the rig: what one run's output cell holds of `/sys/log`
//! under each form. Nothing a run does is written at the level the
//! Kernel logs at, so two runs read one log and their stamps agree.
//!
//! The wall clock is the Kernel's own, so what a run stamps with it is
//! the rig's time and only its shape is here; the forms over entries
//! made by hand are unit tests beside `show`.
//!
//! Under `w` and `W` the cell goes on, and a read at its end answers
//! whatever the log gained since the read before.

use reovim_client::{Client, Error, Fault, Handle, Rd, Rights};
use reovim_module_sysutils::bins::dmesg::Args;
use reovim_rig::Rig;

/// A run with nothing asked for: the plain stamp, every mark and every
/// level, no color.
const fn plain() -> Args {
    Args {
        t: false,
        T: false,
        H: false,
        d: false,
        r: false,
        x: false,
        k: false,
        w: false,
        W: false,
        f: String::new(),
        l: String::new(),
        color: false,
    }
}

/// `call /bin/dmesg run {..}`: the rd of the output cell, or the fault
/// the call answered with.
fn run(client: &mut Client, args: &Args) -> Result<Rd, Fault> {
    let dmesg = client.resolve("/bin/dmesg")?;
    let out = client.call_by_name(dmesg, "run", reovim_sdk::encode(args))?;
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
fn text(client: &mut Client, args: &Args) -> String {
    let cell = run(client, args).unwrap();
    drain(client, cell)
}

/// The first line of it, which every form stamps its own way.
fn first(said: &str) -> &str {
    said.lines().next().unwrap()
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

#[test]
fn every_form_writes_its_own_stamp() {
    let rig = Rig::new("dmesg-stamp");
    let mut client = rig.client();

    // The default: seconds since the Kernel's first entry.
    let said = text(&mut client, &plain());
    assert!(said.starts_with("[    0.000000] load module=fs"), "{said}");

    // `t` drops the stamp, and `r` is the entry as the Kernel wrote it,
    // which for one with no number in it is that same text.
    let bare = text(&mut client, &Args { t: true, ..plain() });
    assert_eq!(first(&bare), "load module=fs");
    let raw = text(&mut client, &Args { r: true, ..plain() });
    assert_eq!(first(&raw), "load module=fs");

    // `T` is the Kernel's wall clock and `H` its minute, so the stamp
    // is the rig's own time; the forms themselves are the unit tests.
    let ctime = text(&mut client, &Args { T: true, ..plain() });
    assert!(first(&ctime).ends_with("] load module=fs"), "{ctime}");
    assert!(first(&ctime).starts_with('['), "{ctime}");
    let human = text(&mut client, &Args { H: true, ..plain() });
    assert!(first(&human).ends_with("] load module=fs"), "{human}");

    // `x` decodes the mark and the level in front of the stamp, and the
    // letters combine.
    let decoded = text(&mut client, &Args { x: true, ..plain() });
    assert!(decoded.starts_with("kernel:info  : ["), "{decoded}");
    let both = text(&mut client, &Args { t: true, x: true, ..plain() });
    assert_eq!(first(&both), "kernel:info  : load module=fs");
}

#[test]
fn the_delta_form_says_the_time_since_the_line_before() {
    let rig = Rig::new("dmesg-delta");
    let mut client = rig.client();

    // The first line of a run has nothing before it to count from.
    let said = text(&mut client, &Args { d: true, ..plain() });
    assert!(said.starts_with("[    0.000000 <    0.000000>]"), "{said}");
    let ctime = text(&mut client, &Args { d: true, T: true, ..plain() });
    assert!(first(&ctime).contains(" <    0.000000>] "), "{ctime}");

    // `H` already says the time since the line before; `d` adds nothing
    // to it. With no stamp at all the delta stands alone.
    let human = text(&mut client, &Args { d: true, H: true, ..plain() });
    assert_eq!(human, text(&mut client, &Args { H: true, ..plain() }));
    let alone = text(&mut client, &Args { d: true, t: true, ..plain() });
    assert!(alone.starts_with("[<    0.000000>]"), "{alone}");
}

#[test]
fn the_filters_choose_the_marks_and_the_levels() {
    let rig = Rig::new("dmesg-filter");
    let mut client = rig.client();
    let mut other = rig.client();

    // An err entry of another mark, as printk's `<3>` writes one.
    let log = other.resolve("/sys/log").unwrap();
    other.write_all(log, b"<3>boom".to_vec()).unwrap();

    let marks = |f: &str| Args { t: true, f: f.to_string(), ..plain() };
    let levels = |l: &str| Args { t: true, l: l.to_string(), ..plain() };

    // `k` is the kernel mark named, so the entry another mark wrote is
    // not in either listing.
    let kernel = text(&mut client, &Args { t: true, k: true, ..plain() });
    assert_eq!(kernel, text(&mut client, &marks("kernel")));
    assert!(kernel.starts_with("load module=fs\n"), "{kernel}");
    assert!(!kernel.contains("boom"), "{kernel}");

    let boom = text(&mut client, &levels("err"));
    assert_eq!(boom, "boom\n");
    assert_eq!(text(&mut client, &levels("emerg")), "");
    // A comma list is every level in it.
    let both = text(&mut client, &levels("err,info"));
    assert!(both.contains("load module=fs\n") && both.ends_with("boom\n"));

    // A level that is not one of the eight fails the run and leaves no
    // cell; naming the eight is the shell's, not the Driver's.
    let dmesg = client.resolve("/bin/dmesg").unwrap();
    assert_eq!(
        run(&mut client, &levels("nope")),
        Err(Fault::Kernel(Error::Invalid))
    );
    assert_eq!(client.children(dmesg), Ok(Vec::new()));
}

#[test]
fn color_paints_the_stamp_and_what_is_urgent() {
    let rig = Rig::new("dmesg-color");
    let mut client = rig.client();
    let mut other = rig.client();
    let log = other.resolve("/sys/log").unwrap();
    other.write_all(log, b"<3>boom".to_vec()).unwrap();

    // `color` is what the shell fills from the host's screen; the
    // stamp goes green and an `err` entry red.
    let painted = Args { color: true, ..plain() };
    let said = text(&mut client, &painted);
    assert!(said.starts_with("\x1b[32m[    0.000000]\x1b[0m "), "{said}");
    let last = said.lines().next_back().unwrap();
    assert!(last.ends_with("\x1b[31mboom\x1b[0m"), "{said}");

    // The `H` minute break is bold blue and the `x` decode dim.
    let human = Args { H: true, x: true, color: true, ..plain() };
    let said = text(&mut client, &human);
    assert!(
        said.starts_with("\x1b[2mkernel:info  : \x1b[0m\x1b[1;34m[")
            && said.contains("]\x1b[0m load module=fs\n"),
        "{said}"
    );
}

#[test]
fn a_kernel_line_says_the_rd_the_api_and_the_method_in_words() {
    let rig = Rig::new("dmesg-decode");
    let mut client = rig.client();
    let root = client.resolve("/module/fs/root").unwrap();

    // The trace and the `call` pair are measurement, kept at `debug`.
    let level = client.resolve("/sys/loglevel").unwrap();
    client.write_all(level, b"debug".to_vec()).unwrap();
    let made = reovim_sdk::encode(&"made".to_string());
    client.call_by_name(root, "create", made).unwrap();

    let debug = Args { t: true, l: "debug".to_string(), ..plain() };
    let cell = run(&mut client, &debug).unwrap();
    let said = drain(&mut client, cell);
    let pair: Vec<&str> = said
        .lines()
        .filter(|line| line.starts_with("call rd=/module/fs/root"))
        .collect();
    assert_eq!(pair.len(), 2, "{said}");
    assert_eq!(
        pair[0],
        "call rd=/module/fs/root api=fs method=create dispatch"
    );
    let head = "call rd=/module/fs/root api=fs method=create reply took=";
    let spent = pair[1].strip_prefix(head).unwrap_or_else(|| panic!("{said}"));
    // A unit, not the Kernel's bare nanoseconds; `heap=` is bytes
    // already, so it stays as the Kernel wrote it.
    let (span, bytes) = spent.split_once(" heap=").unwrap();
    assert!(span.ends_with('s') && !span.ends_with("=s"), "{said}");
    assert!(span.starts_with(|c: char| c.is_ascii_digit()), "{said}");
    assert!(bytes.starts_with(['+', '-']), "{said}");
    // The run's own dispatch line names this Driver by its path.
    assert!(
        said.contains("call rd=/bin/dmesg api=dmesg method=run "),
        "{said}"
    );
    // Nothing the log's own reading did is in it.
    assert!(!said.contains("rd=/sys/log"), "{said}");

    // `r` is what the Kernel wrote: the numbers it had.
    let (hash, decl) = client.decls(root).unwrap().pop().unwrap();
    let number = decl.methods.iter().find(|m| m.name == "create").unwrap();
    let raw = Args { r: true, l: "debug".to_string(), ..plain() };
    let cell = run(&mut client, &raw).unwrap();
    let said = drain(&mut client, cell);
    let line = format!(
        "call rd={} api={:016x} method={} dispatch",
        root.0, hash.0, number.number
    );
    assert!(said.lines().any(|said| said == line), "{said}");
}

#[test]
fn a_tailing_run_writes_the_entry_another_client_writes() {
    let rig = Rig::new("dmesg-tail");
    let mut client = rig.client();
    let mut other = rig.client();

    // `W` takes only what comes after it, so the cell holds nothing and
    // is not over: the read at its end is sent back.
    let new = Args { W: true, t: true, l: "err".to_string(), ..plain() };
    let cell = run(&mut client, &new).unwrap();
    let h = client.open(cell, Rights::READ).unwrap();
    assert_eq!(client.read(h, 0, 64), Err(Fault::Kernel(Error::Busy)));

    // The err entry the other client writes, with printk's `<3>` in
    // front of it.
    let log = other.resolve("/sys/log").unwrap();
    other.write_all(log, b"<3>boom".to_vec()).unwrap();
    assert_eq!(so_far(&mut client, h, 0), "boom\n");
}

#[test]
fn w_writes_what_the_log_held_and_then_what_it_gains() {
    let rig = Rig::new("dmesg-tail-all");
    let mut client = rig.client();
    let mut other = rig.client();

    let all = Args { w: true, t: true, ..plain() };
    let cell = run(&mut client, &all).unwrap();
    let h = client.open(cell, Rights::READ).unwrap();
    let held = so_far(&mut client, h, 0);
    assert!(held.starts_with("load module=fs\n"), "{held}");

    let log = other.resolve("/sys/log").unwrap();
    other.write_all(log, b"<3>boom".to_vec()).unwrap();
    assert_eq!(so_far(&mut client, h, held.len() as u64), "boom\n");
}

#[test]
fn a_tailing_run_closes_its_handle_on_the_log_with_the_cell() {
    let rig = Rig::new("dmesg-close");
    let mut client = rig.client();
    let dmesg = client.resolve("/bin/dmesg").unwrap();
    let log = client.resolve("/sys/log").unwrap();
    let held = |client: &mut Client| {
        let core = client.resolve("/sys/core").unwrap();
        let text = String::from_utf8(client.read_all(core).unwrap()).unwrap();
        let on = format!(" rd={} ", log.0);
        text.lines().filter(|line| line.contains(&on)).count()
    };

    let before = held(&mut client);
    let cell = run(&mut client, &Args { W: true, ..plain() }).unwrap();
    let h = client.open(cell, Rights::READ).unwrap();
    assert_eq!(held(&mut client), before + 1, "the handle the run kept");

    client.close(h).unwrap();
    assert_eq!(held(&mut client), before);
    assert_eq!(client.children(dmesg), Ok(Vec::new()));
    assert_eq!(client.info(cell), Err(Fault::Kernel(Error::Invalid)));
}
