//! One Vim across every swap of the region (01.0), over the tui and
//! the rig: the cells work the same across a second `start`, a start
//! that fails leaves the `Editor` startable for the next one, a
//! register crosses a listing and a pick, `:cd` is kept, and a stay
//! ended by a door other than Vim's own `shown` leaves nothing
//! pending.
//!
//! The corpus (tests/parity, recorded from nvim) is the six cases a
//! file can carry; these two cannot be recorded — nvim's `:Explore` is
//! netrw and a case's files are flat — so they are rig cases here.

mod common;

use std::rc::Rc;

use common::{cat, one};
use reovim_client::Host;
use reovim_client::dev::{Clip, Table, Want};
use reovim_client_region::Region;
use reovim_client_tui::Step;
use reovim_kernel_interface::{Calls, Error, Rd};
use reovim_plugin_vim::Editor;
use reovim_rig::{Rig, Seen, Stub, lines, listed, opened, said, status, typed};
use reovim_sdk::cell;
use reovim_sdk::walk::ROOT;

/// The rd of `path`, a name under the root, through a Client's own
/// calls.
fn rd(host: &Rc<Host>, path: &str) -> Rd {
    let ctx = host.own();
    cell::resolve(&ctx, ROOT, &format!("/module/fs/root/{path}")).unwrap()
}

/// What `Stub::typing` scripts as a listing's pick: a key asks for
/// `vim` over `target`, as Enter on an entry does.
type Script = Box<dyn Fn(&dyn Calls, &Region, Option<Rd>) -> Result<(), Error>>;

/// Picks `target`, whatever the stub was started over.
fn picks(target: Rd) -> Script {
    Box::new(move |ctx, region, _| region.opens(ctx, target, "vim"))
}

/// The first pick asks for `vim` over the Container itself, which
/// `Editor::start` refuses (`drafted` cannot open one as text); once
/// `seen` says the region came back and the stub started again, the
/// next pick is `target` instead.
fn refused_then_picks(seen: Seen, target: Rd) -> Script {
    Box::new(move |ctx, region, over| {
        if seen.borrow().len() <= 1 {
            region.opens(ctx, over.unwrap(), "vim")
        } else {
            region.opens(ctx, target, "vim")
        }
    })
}

#[test]
fn one_editor_started_twice_serves_every_cell() {
    let rig = Rig::new("restart-cells");
    rig.file("a", b"one\ntwo\n");
    rig.file("b", b"three\n");
    let vim = Editor::new();
    let host = rig.host();
    let mut tui =
        opened(&host, one(vim), Clip::none(), "", Some("a"), None).unwrap();

    typed(&mut tui, "\"ayy");
    assert_eq!(typed(&mut tui, ":e b<CR>"), Step::Changed, "the second start");

    // The rds `start` registers are made the same way the second time:
    // nothing is left stale from the first.
    assert_eq!(
        listed(&host, "/self/vim"),
        [
            "view",
            "mode",
            "pending",
            "registers",
            "marks",
            "jumps",
            "recording",
            "bin"
        ]
    );
    assert_eq!(listed(&host, "/self/vim/registers").len(), 27);
    assert_eq!(listed(&host, "/self/vim/bin").len(), 11);
    assert_eq!(cat(&host, "mode"), "normal\n");
    assert_eq!(cat(&host, "pending"), "");
    assert_eq!(cat(&host, "registers/a"), "one\n", "yanked before the swap");

    // What was yanked in `a` puts into `b`, and the write lands there.
    assert_eq!(typed(&mut tui, "\"ap"), Step::Changed);
    assert_eq!(typed(&mut tui, ":w<CR>"), Step::Changed);
    assert_eq!(rig.disk("b"), b"three\none\n");
}

#[test]
fn a_start_that_fails_leaves_the_editor_startable() {
    let rig = Rig::new("restart-refused");
    rig.file("a", b"one\ntwo\n");
    std::fs::create_dir(rig.dir().join("d")).unwrap();
    let vim = Editor::new();
    let host = rig.host();
    let file = rd(&host, "a");
    let seen = Seen::default();
    let table: Table = Box::new(move |want| match want {
        Want::Container => {
            let script = refused_then_picks(seen.clone(), file);
            let stub = Stub::typing(&seen, "-- LISTING --", script);
            Some((String::from("stub"), stub))
        }
        _ => Some((String::from("vim"), vim.booted())),
    });
    let mut tui =
        opened(&host, table, Clip::none(), "", Some("a"), None).unwrap();

    typed(&mut tui, "yy");
    assert_eq!(typed(&mut tui, ":explore<CR>"), Step::Changed);
    assert_eq!(said(&mut tui), "-- LISTING --", "the listing shown");

    // The pick asks for `vim` over the Container; `start` refuses it,
    // so the region comes back to the listing that stood before, and
    // the name is free again.
    assert_eq!(typed(&mut tui, "l"), Step::Changed);
    assert_eq!(said(&mut tui), "-- LISTING --", "the region came back");

    // Its next pick is the file: `start` runs again on the same
    // `Editor`, which it left startable.
    assert_eq!(typed(&mut tui, "l"), Step::Changed);
    assert!(status(&mut tui).starts_with("a  "), "vim over the file again");

    // What was yanked before the failed start puts here.
    assert_eq!(typed(&mut tui, "p"), Step::Changed);
    assert_eq!(lines(&mut tui)[..2], ["one", "one"]);
}

#[test]
fn a_register_crosses_a_listing_and_a_pick() {
    let rig = Rig::new("restart-pick");
    std::fs::create_dir(rig.dir().join("d")).unwrap();
    rig.file("d/f", b"one\ntwo\n");
    rig.file("d/g", b"three\n");
    let vim = Editor::new();
    let host = rig.host();
    let g = rd(&host, "d/g");
    let seen = Seen::default();
    let table: Table = Box::new(move |want| match want {
        Want::Container => {
            let stub = Stub::typing(&seen, "-- LISTING --", picks(g));
            Some((String::from("stub"), stub))
        }
        _ => Some((String::from("vim"), vim.booted())),
    });
    let mut tui =
        opened(&host, table, Clip::none(), "", Some("d/f"), None).unwrap();

    typed(&mut tui, "yy");
    assert_eq!(typed(&mut tui, ":explore<CR>"), Step::Changed);
    assert_eq!(said(&mut tui), "-- LISTING --");

    assert_eq!(typed(&mut tui, "l"), Step::Changed, "the pick");
    assert!(status(&mut tui).starts_with("g  "), "vim over g");

    assert_eq!(typed(&mut tui, "p"), Step::Changed);
    assert_eq!(lines(&mut tui)[..2], ["three", "one"]);
}

#[test]
fn cd_is_kept_across_a_swap() {
    let rig = Rig::new("restart-cd");
    std::fs::create_dir(rig.dir().join("sub")).unwrap();
    rig.file("a", b"one\n");
    rig.file("sub/g", b"two\n");
    rig.file("sub/h", b"three\n");
    let vim = Editor::new();
    let host = rig.host();
    let mut tui =
        opened(&host, one(vim), Clip::none(), "", Some("a"), None).unwrap();

    assert_eq!(typed(&mut tui, ":cd sub<CR>"), Step::Changed);
    assert_eq!(typed(&mut tui, ":e g<CR>"), Step::Changed);
    assert!(status(&mut tui).starts_with("g  "), "g, under sub");

    // The swap into `g` has already run `stop` and `start` once on
    // this `Editor`: `cwd` is kept, not reset, so `h` resolves under
    // `sub` too.
    assert_eq!(typed(&mut tui, ":e h<CR>"), Step::Changed);
    assert!(status(&mut tui).starts_with("h  "), "h, still under sub");
}

#[test]
fn a_stay_ended_by_another_door_leaves_nothing_pending() {
    let rig = Rig::new("restart-door");
    rig.file("a", b"one two\n");
    rig.file("b", b"three\n");
    let vim = Editor::new();
    let host = rig.host();
    let mut tui =
        opened(&host, one(vim), Clip::none(), "", Some("a"), None).unwrap();

    typed(&mut tui, "d");
    assert_eq!(cat(&host, "pending"), "d");

    // Another door than Vim's own `shown`: an ask written straight to
    // `/self/dev/open`, as `host.own()` can, which is what a `Region`
    // anywhere at this Client speaks through. The swap an ask makes
    // happens at the next read of `view` (dev.rs), so a tick, which
    // reads it unconditionally, runs before it shows.
    let ctx = host.own();
    let region = Region::of(&ctx).unwrap();
    let b = rd(&host, "b");
    region.opens(&ctx, b, "").unwrap();
    tui.tick().unwrap();

    assert_eq!(cat(&host, "pending"), "", "the half-typed d is gone");
    assert_eq!(typed(&mut tui, "w"), Step::Changed, "a motion, not d's");
    assert_eq!(lines(&mut tui), ["three"], "w moved; nothing was deleted");
}
