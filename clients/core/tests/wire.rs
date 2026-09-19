//! Two Clients on one Kernel: what one may read of the other's
//! `/self`, what the walk costs it, and what a second walk costs.
//!
//! Both are on this thread, so the reader's wait is what gives the
//! machine back to the Client that must answer: the rig's Carrier
//! answers every Client that was there before it at each pause.

use std::rc::Rc;

use reovim_client::{Calls, Clip, Host, ROOT, SELF, Table, dev};
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
