//! Two Clients on one Kernel: what one may read of the other's
//! `/self`, what the walk costs it, and what a second walk costs.
//!
//! Both are on this thread, so the reader's wait is what gives the
//! machine back to the Client that must answer: the rig's Carrier
//! answers every Client that was there before it at each pause.

use std::rc::Rc;

use reovim_client::{Calls, Clip, Host, ROOT, SELF, Table, dev};
use reovim_kernel_interface::{
    Driver, DriverId, Error, Event, Flags, Kind, Module, Rd, Reply, Rights,
};
use reovim_rig::Rig;
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
