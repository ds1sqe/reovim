//! Two Clients on one Kernel: what one may read of the other's
//! `/self`, what the walk costs it, and what a second walk costs.
//!
//! Both are on this thread, so the reader's wait is what gives the
//! machine back to the Client that must answer: the rig's Carrier
//! answers every Client that was there before it at each pause.

use std::rc::Rc;

use reovim_client::{Calls, Clip, Host, ROOT, SELF, Table, dev};
use reovim_client_api::{Key, keyboard};
use reovim_kernel_interface::{
    Driver, DriverId, Error, Event, Flags, Kind, Module, Rd, Reply, Rights,
};
use reovim_module_sysutils::bins::{capture, keys};
use reovim_rig::bins::said;
use reovim_rig::{Rig, screen};
use reovim_sdk::{cell, ops};

/// A Client with no Plugin at all: what a region is asked to show is
/// nobody's, which none of this needs.
fn none() -> Table {
    Box::new(|_| None)
}

/// One Client with `dev` booted over `dir`, which is what gives it a
/// `/self/dev/cwd` to be read.
fn editor(rig: &Rig, dir: &str) -> Rc<Host> {
    let host = rig.host();
    dev::boot(&host, none(), Clip::none(), dir, None, None).unwrap();
    host
}

#[test]
fn a_client_reads_a_cell_of_another_clients_self() {
    let rig = Rig::new("wire-two-clients");
    let dir = rig.dir().to_str().unwrap().to_string();
    let a = editor(&rig, &dir);
    let b = rig.host();
    let ctx = b.own();
    let path = "/session/1/client/1/self/dev/cwd";

    let walked = rig.sent();
    let cwd = cell::resolve(&ctx, ROOT, path).unwrap();
    let walk = rig.sent() - walked;
    let read = ops::bytes(&ctx, cwd).unwrap();
    assert_eq!(read, dir.as_bytes(), "the directory a was given");

    // Metadata is the mirror's: the second walk is answered from the
    // Kernel's own table, one message a name and nothing served.
    let again = rig.sent();
    assert_eq!(cell::resolve(&ctx, ROOT, path).unwrap(), cwd);
    let steps = path.split('/').filter(|n| !n.is_empty()).count();
    let names = u32::try_from(steps).unwrap();
    assert_eq!(rig.sent() - again, names, "a name a message and no more");
    assert!(walk > names, "the first walk asked a: {walk}");
    assert_eq!(a.serve(), 0, "nothing was left unanswered");

    // The flags are what a reported and the name is a's own.
    let info = ops::info(&ctx, cwd).unwrap();
    assert_eq!(info.name.as_deref(), Some("cwd"));
    assert_eq!(info.flags, reovim_client::Flags::READ);
    let at = ops::where_is(&ctx, cwd).unwrap();
    assert_eq!(at, "/session/1/client/1/self/dev/cwd");
}

#[test]
fn a_client_lists_another_clients_self_and_serves_nothing_unasked() {
    let rig = Rig::new("wire-two-listings");
    let a = editor(&rig, "");
    let b = rig.host();
    let (mine, theirs) = (a.own(), b.own());

    // What the listing shows is what that Client lists of its own.
    let root =
        cell::resolve(&theirs, ROOT, "/session/1/client/1/self").unwrap();
    let listed = ops::children(&theirs, root).unwrap();
    let named = |ctx: &dyn Calls, rds: &[reovim_client::Rd]| {
        let name = |rd: &reovim_client::Rd| ops::info(ctx, *rd).unwrap().name;
        rds.iter().filter_map(name).collect::<Vec<_>>()
    };
    let here = ops::children(&mine, SELF).unwrap();
    assert_eq!(named(&theirs, &listed), named(&mine, &here));
    assert!(named(&mine, &here).contains(&"dev".to_string()));

    // Nothing crosses unless a remote asks: a's own turn with nobody
    // reading brings no op over and answers none.
    let _ = ops::since(&mine, 0).unwrap();
    assert_eq!(a.serve(), 0, "a served an op nobody asked of it");
    let idle = rig.sent();
    let _ = ops::children(&mine, SELF).unwrap();
    assert_eq!(rig.sent(), idle, "a's own listing is a's own");
}

#[test]
fn a_remote_reads_the_link_of_the_client_that_carried_it() {
    let rig = Rig::new("wire-link");
    let a = editor(&rig, "");
    let b = editor(&rig, "");
    let theirs = b.own();
    let said = |path: &str| {
        let rd = cell::resolve(&theirs, ROOT, path).unwrap();
        String::from_utf8(ops::bytes(&theirs, rd).unwrap()).unwrap()
    };
    let messages = |said: &str| -> u64 {
        let first = said.lines().next().unwrap();
        first.strip_prefix("messages ").unwrap().parse().unwrap()
    };

    // a has been on the wire and b has hardly, so neither cell can
    // answer the other's number by accident.
    for _ in 0..64 {
        cell::resolve(&a.own(), ROOT, "/module/text").unwrap();
    }

    // b's own `link` is a local read of b's own numbers, exactly.
    let near = said("/self/dev/link");
    assert_eq!(messages(&near), b.wire().link().messages);

    // a's is read through the mirror in the shape a fact prints in,
    // and the number is a's, made while a was serving the read: it
    // stands between what a had counted before the read and after.
    let before = a.wire().link().messages;
    let far = said("/session/1/client/1/self/dev/link");
    let now = a.wire().link().messages;
    let read = messages(&far);
    assert!((before..=now).contains(&read), "a counted {before}..{now}");
    assert!(read > messages(&near), "b read its own numbers");
}

/// A Module of a's own with two cells whose write is a `Modify` on the
/// one written: the one thing a Client announces of its own account,
/// and two of them so a case can say that a change on the cell nobody
/// watches costs nothing.
struct Cells;

impl Module for Cells {
    fn start(&self, ctx: &dyn Calls, home: Rd) -> Result<(), Error> {
        for name in ["one", "two"] {
            let rd = ctx.register(DriverId(0), home, Some(name))?;
            ctx.report(rd, Flags::READ.union(Flags::WRITE), &[])?;
        }
        Ok(())
    }
}

impl Driver for Cells {
    fn open(&self, _: &dyn Calls, rd: Rd, _: Rights) -> Reply<u64> {
        Reply::ok(rd.0)
    }

    fn write(
        &self,
        ctx: &dyn Calls,
        h: u64,
        _: u64,
        buf: &[u8],
    ) -> Reply<usize> {
        let _ = ctx.changed(Rd(h), Kind::Modify);
        Reply::ok(buf.len())
    }
}

/// One turn of a Client's own loop: the `Since` that brings over what
/// the server asks of it, those ops run and answered, and the sweep.
fn turn(host: &Host) -> usize {
    let _ = ops::since(&host.own(), 0).unwrap();
    host.serve()
}

#[test]
fn a_change_on_a_client_reaches_a_remote_watcher_and_nobody_else() {
    let rig = Rig::new("wire-changed");
    let a = rig.host();
    let cells = Rc::new(Cells);
    a.load("cells", cells.clone(), vec![cells]).unwrap();
    let b = rig.host();
    let (mine, theirs) = (a.own(), b.own());
    let at = |name: &str| {
        let path = format!("/session/1/client/1/self/cells/{name}");
        cell::resolve(&theirs, ROOT, &path).unwrap()
    };
    let (one, two) = (at("one"), at("two"));
    let here = |name: &str| {
        cell::resolve(&mine, ROOT, &format!("/self/cells/{name}")).unwrap()
    };

    // Nothing crosses while nobody reads: a's own editing is served to
    // nobody, kept by nobody and announced to nobody.
    let quiet = rig.sent();
    ops::put(&mine, here("one"), b"x".to_vec()).unwrap();
    assert_eq!(a.serve(), 0, "a served an op nobody asked of it");
    assert_eq!(rig.sent(), quiet, "a's own change is a's own");

    // b watches one cell, which costs a an open and a watch and two
    // turns before it is live; nobody waits on either.
    let h = ops::watch(&theirs, one).unwrap();
    assert_eq!(turn(&a), 1, "the open of the proxy's own handle");
    assert_eq!(turn(&a), 1, "the watch behind it");
    assert_eq!(turn(&a), 0, "and nothing more");
    let (_, cursor) = ops::since(&theirs, 0).unwrap();

    // One message a change of that cell, and none for the other.
    let before = rig.sent();
    ops::put(&mine, here("one"), b"y".to_vec()).unwrap();
    assert_eq!(rig.sent(), before, "the write is a's own");
    a.serve();
    assert_eq!(rig.sent(), before + 1, "one message a change");
    ops::put(&mine, here("two"), b"z".to_vec()).unwrap();
    a.serve();
    assert_eq!(rig.sent(), before + 1, "a cell nobody watches is not said");

    // What b hears is a `Modify` on the rd b holds, once, and nothing
    // of the cell it has a mirror row for and no watch on.
    let (events, cursor) = ops::since(&theirs, cursor).unwrap();
    let cells = |e: &&Event| e.rd == one || e.rd == two;
    let heard = |events: &[Event]| {
        let of = |e: &Event| (e.rd, e.kind);
        events.iter().filter(cells).map(of).collect::<Vec<_>>()
    };
    assert_eq!(heard(&events), vec![(one, Kind::Modify)]);

    // b's close implies the unwatch, which the Kernel tells the proxy
    // of: two closes reach a, b's own handle and the proxy's, and a's
    // next write moves the count by nothing.
    ops::close(&theirs, h).unwrap();
    assert_eq!(turn(&a), 2, "b's handle closed and the proxy's with it");
    let after = rig.sent();
    ops::put(&mine, here("one"), b"w".to_vec()).unwrap();
    a.serve();
    assert_eq!(rig.sent(), after, "nothing crosses while nobody reads");
    let (events, _) = ops::since(&theirs, cursor).unwrap();
    assert_eq!(heard(&events), Vec::new());
}

#[test]
fn a_row_a_client_unregisters_goes_from_the_listing_a_remote_took() {
    let rig = Rig::new("wire-gone");
    let a = rig.host();
    let cells = Rc::new(Cells);
    let id = a.load("cells", cells.clone(), vec![cells]).unwrap();
    let b = rig.host();
    let (mine, theirs) = (a.own(), b.own());
    let path = "/session/1/client/1/self/cells";
    let home = cell::resolve(&theirs, ROOT, path).unwrap();
    let listed = ops::children(&theirs, home).unwrap();
    let two = cell::resolve(&theirs, ROOT, &format!("{path}/two")).unwrap();
    assert_eq!(listed.len(), 2, "one and two");
    let here = cell::resolve(&mine, ROOT, "/self/cells/two").unwrap();

    // a takes `two` out: the next sweep says so in one message, whether
    // or not anybody mirrored it, and the listing the Kernel answers
    // from its mirror is one shorter. The old rd is refused there and
    // costs a nothing.
    let before = rig.sent();
    a.calls(id).unregister(here).unwrap();
    a.serve();
    assert_eq!(rig.sent(), before + 1, "one message a row gone");
    let left = ops::children(&theirs, home).unwrap();
    assert_eq!(left, vec![listed[0]], "the listing is one shorter");
    assert_eq!(ops::info(&theirs, two), Err(Error::Invalid));
    assert_eq!(a.serve(), 0, "a served an op nobody asked of it");
}

#[test]
fn a_row_a_client_registers_after_a_listing_reaches_the_listing_again() {
    let rig = Rig::new("wire-create");
    let a = rig.host();
    let cells = Rc::new(Cells);
    let id = a.load("cells", cells.clone(), vec![cells]).unwrap();
    let b = rig.host();
    let theirs = b.own();
    let path = "/session/1/client/1/self/cells";
    let home = cell::resolve(&theirs, ROOT, path).unwrap();
    let listed = ops::children(&theirs, home).unwrap();
    assert_eq!(listed.len(), 2, "one and two");
    let here = cell::resolve(&a.own(), ROOT, "/self/cells").unwrap();

    // a registers two more under `cells`, which b listed: the next
    // sweep says so in one message, the parent's and not one a row,
    // and the listing b takes again is two longer, the rows that stood
    // keeping their rds. A name b walks to is one of the new rows.
    let mine = a.calls(id);
    let before = rig.sent();
    for name in ["three", "four"] {
        let rd = mine.register(DriverId(0), here, Some(name)).unwrap();
        mine.report(rd, Flags::READ, &[]).unwrap();
    }
    a.serve();
    assert_eq!(rig.sent(), before + 1, "one message a parent");
    let again = ops::children(&theirs, home).unwrap();
    assert_eq!(again.len(), 4, "one, two, three and four");
    assert_eq!(again[..2], listed[..], "the rows that stood keep their rds");
    let four = cell::resolve(&theirs, ROOT, &format!("{path}/four")).unwrap();
    assert_eq!(four, again[3]);
    assert_eq!(a.serve(), 0, "a served an op nobody asked of it");
}

/// Changes made with nobody serving: more than the Host's ring holds
/// (`RING`, clients/core/src/host.rs), so the sweep's own cursor falls
/// off the front of it.
const PAST: usize = 1025;

#[test]
fn a_watcher_that_missed_the_ring_is_told_to_read_the_cell_again() {
    let rig = Rig::new("wire-overflow");
    let a = rig.host();
    let cells = Rc::new(Cells);
    a.load("cells", cells.clone(), vec![cells]).unwrap();
    let b = rig.host();
    let (mine, theirs) = (a.own(), b.own());
    let path = "/session/1/client/1/self/cells/one";
    let one = cell::resolve(&theirs, ROOT, path).unwrap();
    let here = cell::resolve(&mine, ROOT, "/self/cells/one").unwrap();

    let h = ops::watch(&theirs, one).unwrap();
    assert_eq!(turn(&a), 1, "the open of the proxy's own handle");
    assert_eq!(turn(&a), 1, "the watch behind it");
    let (_, cursor) = ops::since(&theirs, 0).unwrap();

    // a edits that cell more times than its own ring holds and serves
    // nobody while it does: where the sweep stood is off the front of
    // the ring, and what was missed cannot be told one change at a
    // time. `Op::Changed` carries `Modify` and `Attrib` and no third
    // kind, so every rd a remote watches is told `Modify` and the
    // watcher reads the cell again, which is what it would do anyway.
    for _ in 0..PAST {
        ops::put(&mine, here, b"x".to_vec()).unwrap();
    }
    let before = rig.sent();
    a.serve();
    assert_eq!(rig.sent(), before + 1, "one message, and the ring is deep");
    let (events, _) = ops::since(&theirs, cursor).unwrap();
    let heard: Vec<(Rd, Kind)> = events
        .iter()
        .filter(|event| event.rd == one)
        .map(|event| (event.rd, event.kind))
        .collect();
    assert_eq!(heard, [(one, Kind::Modify)]);
    ops::close(&theirs, h).unwrap();
}

#[test]
fn a_client_captures_another_clients_screen_and_types_into_it() {
    let rig = Rig::new("wire-hands");
    let a = editor(&rig, "");
    let b = rig.host();
    let booted = reovim_module_sysutils::boot("");
    b.load("sysutils", booted.module, booted.drivers).unwrap();
    let (mine, theirs) = (a.own(), b.own());
    let at =
        |ctx: &dyn Calls, path: &str| cell::resolve(ctx, ROOT, path).unwrap();
    let there = "/session/1/client/1/self/dev";

    // a draws a frame of its own, as its tui would.
    let drawn = reovim_sdk::encode(&screen(1, &["hi"]));
    ops::put(&mine, at(&mine, "/self/dev/display/0/text"), drawn).unwrap();

    // b's own `capture` reads it through the mirror: one file, and a
    // mirrored rd is a target like any other.
    let args = capture::Args {
        f: false,
        cwd: ROOT,
        target: format!("{there}/display/0/text"),
    };
    let run = at(&theirs, "/self/bin/capture");
    let seen = said(&theirs, run, &args);
    assert_eq!(seen.as_deref(), Ok("hi\ncursor 0 0 block\n"));

    // And b's `keys` types into a's keyboard: one write of every code,
    // so the line costs a one tick and not seven.
    let keyboard = at(&mine, "/self/dev/input/keyboard");
    let h = mine.open(keyboard, Rights::READ).unwrap();
    let typing = keys::Args {
        cwd: ROOT,
        target: format!("{there}/input/keyboard"),
        notation: String::from("ihello<Esc>"),
    };
    let run = at(&theirs, "/self/bin/keys");
    assert_eq!(said(&theirs, run, &typing).as_deref(), Ok(""));

    // What a reads of its own keyboard is what b typed, stamped with
    // a's own rd: a slot does not name its caller.
    let mut taken = Vec::new();
    while let Ok(bytes) = ops::read(&mine, h, 0, 4096) {
        let press: keyboard::Press = reovim_sdk::decode(&bytes).unwrap();
        assert_eq!(press.source, a.wire().cid(), "the Client's own rd");
        taken.extend(Key::of(press.key));
    }
    let typed: Vec<Key> =
        "ihello".chars().map(Key::Char).chain([Key::Esc]).collect();
    assert_eq!(taken, typed);
    ops::close(&mine, h).unwrap();
}
