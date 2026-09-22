//! A Client's own node reached by the server's name for it.
//!
//! `/session/<id>/client/<cid>/self` is the row `/self` is: the Kernel
//! mirrors it, and every op it cannot answer from the mirror it
//! forwards to the Client that owns it. Where that Client is the one
//! asking, the forwarded op waits on the thread the asker is standing
//! on, so an op on that row must never leave home. Each case runs on a
//! thread of its own and fails by [`DEADLINE`] rather than hanging the
//! suite.

use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use reovim_client::{LOCAL_BASE, SELF};
use reovim_kernel_interface::{Calls, Rd, Rights};
use reovim_rig::Rig;
use reovim_sdk::ops;

/// How long a walk gets before the case calls it wedged. Every op here
/// is in-process and costs microseconds; five seconds is the
/// difference between slow and never.
const DEADLINE: Duration = Duration::from_secs(5);

/// `walk` on a thread of its own, or the case fails saying it wedged.
fn bounded<T: Send + 'static>(what: &str, walk: fn() -> T) -> T {
    let (tx, rx) = mpsc::channel();
    thread::spawn(move || tx.send(walk()));
    rx.recv_timeout(DEADLINE)
        .unwrap_or_else(|_| panic!("{what} wedged: nothing in {DEADLINE:?}"))
}

/// The names under an rd, in the order it lists them.
fn names(calls: &dyn Calls, rd: Rd) -> Vec<String> {
    ops::children(calls, rd)
        .unwrap()
        .into_iter()
        .filter_map(|rd| ops::info(calls, rd).ok()?.name)
        .collect()
}

/// `ls /session/<id>/client/<cid>/self`: the walk the `:` line takes
/// for an absolute path, and the listing at the end of it.
#[test]
fn a_clients_own_self_by_its_absolute_path_lists_what_self_lists() {
    let (there, home) = bounded("ls /session/1/client/1/self", || {
        let rig = Rig::new("self-path-ls");
        let host = rig.host();
        let calls = host.own();
        let cid = host.wire().cid();
        let own = ops::lookup(&calls, cid, "self").unwrap();
        (names(&calls, own), names(&calls, SELF))
    });
    assert_eq!(there, home);
}

/// `tree /session/<id>/client/<cid>`: the row is met as a child of
/// the client rd and never named, so the rd itself must stand for
/// home.
#[test]
fn a_clients_own_self_met_as_a_child_of_its_client_rd_lists_the_same() {
    let (there, home) = bounded("tree /session/1/client/1", || {
        let rig = Rig::new("self-path-tree");
        let host = rig.host();
        let calls = host.own();
        let cid = host.wire().cid();
        let named = |rd: &&Rd| {
            ops::info(&calls, **rd).ok().and_then(|i| i.name).as_deref()
                == Some("self")
        };
        let kids = ops::children(&calls, cid).unwrap();
        let own = *kids.iter().find(named).expect("a `self` under the client");
        (names(&calls, own), names(&calls, SELF))
    });
    assert_eq!(there, home);
}

/// The op under the mirrored row that opens: the handle it answers
/// is the Client's own, so every `read`, `write` and `call` after it
/// stays home too.
#[test]
fn a_handle_under_the_mirrored_root_is_the_clients_own() {
    let h = bounded("open /session/1/client/1/self/bin/load", || {
        let rig = Rig::new("self-path-open");
        let host = rig.host();
        let calls = host.own();
        let cid = host.wire().cid();
        let own = ops::lookup(&calls, cid, "self").unwrap();
        let bin = ops::lookup(&calls, own, "bin").unwrap();
        let load = ops::lookup(&calls, bin, "load").unwrap();
        calls.open(load, Rights::READ).unwrap()
    });
    assert!(h.0 >= LOCAL_BASE, "{h:?} is the server's, not the Client's");
}
