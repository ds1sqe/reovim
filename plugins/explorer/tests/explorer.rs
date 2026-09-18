//! The explorer on a Kernel with fs and text, shown through the tui.
//!
//! The table gives the explorer whatever Container the region is asked
//! for and the rig's recording stub for a cell, so what the listing
//! hands on is judged by what that stub was started over: the
//! explorer's tests never use Vim, and Vim is judged by its own
//! corpus.

use std::rc::Rc;

use reovim_client::Client;
use reovim_client::dev::{self, Clip, Table, Want};
use reovim_client_api::Key;
use reovim_client_tui::{Frame, Step, Tui};
use reovim_kernel_interface::{Error, Rd};
use reovim_rig::{Limits, Rig, Seen, Stub};
use reovim_sdk::encode;

/// The app's table in the small: the explorer for a Container, the
/// stub for a cell.
fn table(seen: &Seen) -> Table {
    let seen = seen.clone();
    Box::new(move |want| match want {
        Want::Container => {
            Some((String::from("explorer"), reovim_plugin_explorer::boot()))
        }
        Want::Cell => {
            Some((String::from("stub"), Stub::booted(&seen, "-- STUB --")))
        }
        Want::Named(_) => None,
    })
}

/// A rig whose root holds `work`, which holds `a`, `b` and `sub`.
fn work(test: &str) -> Rig {
    filled(Rig::new(test))
}

/// That directory, in a rig the test made with limits of its own.
fn filled(rig: Rig) -> Rig {
    std::fs::create_dir_all(rig.dir().join("work/sub")).unwrap();
    rig.file("work/b", b"second\n");
    rig.file("work/a", b"first\n");
    rig
}

/// `name` made under `dir` as the fs Module makes a file.
fn made(client: &mut Client, dir: Rd, name: &str) {
    let input = encode(&String::from(name));
    client.call_by_name(dir, "create", input).unwrap();
}

/// The tui over `rig` with `table`, opened over the name `file`.
fn editor(rig: &Rig, table: Table, file: Option<&str>) -> Result<Tui, Error> {
    let host = Rc::new(rig.host());
    let dev = dev::boot(&host, table, Clip::none(), "", file, None)?;
    Tui::open(host, dev)
}

/// The region opened on `name`, a name under the root.
fn over(rig: &Rig, seen: &Seen) -> Tui {
    editor(rig, table(seen), Some("work")).unwrap()
}

/// The frame at the size the app always asks for.
fn frame(tui: &mut Tui) -> Frame {
    tui.frame(80, 24).unwrap()
}

/// One key, whatever it did.
fn press(tui: &mut Tui, key: Key) -> Step {
    tui.key(key).unwrap()
}

/// Those keys, in order, and what the last one did.
fn keys(tui: &mut Tui, keys: &[Key]) -> Step {
    let mut step = Step::Same;
    for key in keys {
        step = press(tui, *key);
    }
    step
}

/// The row the cursor is on, which is the entry selected.
fn row(tui: &mut Tui) -> u16 {
    frame(tui).cursor.0
}

#[test]
fn the_listing_is_the_parent_then_the_containers_then_the_cells() {
    let rig = work("listing");
    let seen = Seen::default();
    let mut tui = over(&rig, &seen);
    let frame = frame(&mut tui);
    assert_eq!(frame.lines, ["../", "sub/", "a", "b"], "each group sorted");
    assert_eq!(frame.cursor, (0, 0), "the selection starts on `../`");
    // The region is named after the directory it is over, and the
    // listing is one revision of a Buffer of the explorer's own.
    assert_eq!(frame.status, "work  1:1  r2[+]   ");
    assert!(seen.borrow().is_empty(), "no cell Plugin was asked for");
}

#[test]
fn the_selection_walks_the_entries_and_stops_at_both_ends() {
    let rig = work("walk");
    let seen = Seen::default();
    let mut tui = over(&rig, &seen);
    assert_eq!(press(&mut tui, Key::Char('j')), Step::Changed);
    assert_eq!(row(&mut tui), 1);
    press(&mut tui, Key::Down);
    assert_eq!(row(&mut tui), 2);
    press(&mut tui, Key::Up);
    assert_eq!(row(&mut tui), 1);
    press(&mut tui, Key::Char('k'));
    press(&mut tui, Key::Char('k'));
    assert_eq!(row(&mut tui), 0, "`k` stops at the first entry");
    press(&mut tui, Key::Char('G'));
    assert_eq!(row(&mut tui), 3, "`G` is the last");
    press(&mut tui, Key::Char('j'));
    assert_eq!(row(&mut tui), 3, "`j` stops at the last entry");
    keys(&mut tui, &[Key::Char('g'), Key::Char('g')]);
    assert_eq!(row(&mut tui), 0, "`gg` is the first");
    keys(&mut tui, &[Key::Char('g'), Key::Char('j')]);
    assert_eq!(row(&mut tui), 0, "a `g` before anything else is nothing");
}

#[test]
fn enter_on_a_cell_gives_the_region_to_the_cell_s_plugin() {
    let rig = work("enter");
    let seen = Seen::default();
    let mut tui = over(&rig, &seen);
    let mut other = rig.client();
    let a = other.resolve("/module/fs/root/work/a").unwrap();
    let down = [Key::Char('j'), Key::Char('j'), Key::Enter];
    keys(&mut tui, &down);
    assert_eq!(*seen.borrow(), [Some(a)], "started over the file");
    let frame = frame(&mut tui);
    assert_eq!(frame.lines, ["first"], "what that file holds");
    assert_eq!(frame.status, "a  1:1  r1   -- STUB --", "the region is it");
}

#[test]
fn enter_on_a_container_lists_it_and_minus_lists_the_parent() {
    let rig = work("down-up");
    let seen = Seen::default();
    let mut tui = over(&rig, &seen);
    keys(&mut tui, &[Key::Char('j'), Key::Enter]);
    let below = frame(&mut tui);
    assert_eq!(below.lines, ["../"], "an empty directory holds its parent");
    assert_eq!(below.status, "sub  1:1  r2[+]   ");

    press(&mut tui, Key::Char('-'));
    let back = frame(&mut tui);
    assert_eq!(back.lines, ["../", "sub/", "a", "b"], "where it came from");
    assert_eq!(back.status, "work  1:1  r2[+]   ");
    assert!(seen.borrow().is_empty(), "no cell Plugin on the way");
}

#[test]
fn minus_walks_the_tree_up_to_the_root_and_stops_there() {
    let rig = work("root");
    let seen = Seen::default();
    let mut tui = over(&rig, &seen);
    // The explorer names no Module: what is above a directory is
    // whatever the tree has above it.
    press(&mut tui, Key::Char('-'));
    assert_eq!(frame(&mut tui).lines, ["../", "work/"], "the fs root");
    press(&mut tui, Key::Char('-'));
    assert_eq!(frame(&mut tui).lines, ["../", "root/"], "/module/fs");
    press(&mut tui, Key::Char('-'));
    let modules = ["../", "coreutils/", "fs/", "sysutils/", "text/"];
    assert_eq!(frame(&mut tui).lines, modules, "/module");
    press(&mut tui, Key::Char('-'));
    // The root the Client sees is the server's with one name more,
    // its own tree, which the listing shows like any other child.
    let root = ["bin/", "module/", "self/", "session/", "sys/", "var/"];
    assert_eq!(frame(&mut tui).lines, root, "the root has no `../`");
    let listing = tui.over();
    press(&mut tui, Key::Char('-'));
    assert_eq!(tui.over(), listing, "nothing above it, so no swap");
    assert_eq!(frame(&mut tui).lines, root);
}

#[test]
fn a_child_made_under_the_container_shows_at_the_next_tick() {
    let rig = work("appears");
    let seen = Seen::default();
    let mut tui = over(&rig, &seen);
    press(&mut tui, Key::Char('G'));
    let mut other = rig.client();
    let dir = other.resolve("/module/fs/root/work").unwrap();
    made(&mut other, dir, "c");
    assert_eq!(tui.tick(), Ok(Step::Changed), "the watch said so");
    let frame = frame(&mut tui);
    assert_eq!(frame.lines, ["../", "sub/", "a", "b", "c"]);
    assert_eq!(frame.cursor, (3, 0), "the selection stays on `b`");
    assert_eq!(tui.tick(), Ok(Step::Same), "and nothing more happened");
}

#[test]
fn a_listing_behind_a_full_ring_is_read_again() {
    // A ring that moved past the explorer hands it one Overflow on
    // the tree's root and nothing else: what happened under the
    // Container is gone with it, so the listing is read again.
    let small = Limits { watch_ring: 2, ..Limits::default() };
    let rig = filled(Rig::with("overflow", small));
    let seen = Seen::default();
    let mut tui = over(&rig, &seen);
    let mut other = rig.client();
    let dir = other.resolve("/module/fs/root/work").unwrap();
    for name in ["c", "d", "e"] {
        made(&mut other, dir, name);
    }
    assert_eq!(tui.tick(), Ok(Step::Changed), "the ring overflowed");
    let whole = ["../", "sub/", "a", "b", "c", "d", "e"];
    assert_eq!(frame(&mut tui).lines, whole, "every child, read again");
}

#[test]
fn the_prompt_is_the_status_and_esc_drops_it() {
    let rig = work("prompt");
    let seen = Seen::default();
    let mut tui = over(&rig, &seen);
    press(&mut tui, Key::Char(':'));
    assert_eq!(frame(&mut tui).status, "work  1:1  r2[+]   :");
    keys(&mut tui, &[Key::Char('q'), Key::Char('!')]);
    assert_eq!(frame(&mut tui).status, "work  1:1  r2[+]   :q!");
    press(&mut tui, Key::Backspace);
    assert_eq!(frame(&mut tui).status, "work  1:1  r2[+]   :q");
    press(&mut tui, Key::Esc);
    assert_eq!(frame(&mut tui).status, "work  1:1  r2[+]   ", "dropped");
    // The line was dropped and not run: Enter is the listing's own
    // key again, and the `q` under it never reached the prompt.
    assert_eq!(press(&mut tui, Key::Enter), Step::Changed);
    assert_eq!(frame(&mut tui).lines, ["../", "work/"], "into the parent");
}

#[test]
fn a_line_the_explorer_has_not_got_is_the_message_nvim_gives() {
    let rig = work("e492");
    let seen = Seen::default();
    let mut tui = over(&rig, &seen);
    let typed = [Key::Char(':'), Key::Char('x'), Key::Enter];
    assert_eq!(keys(&mut tui, &typed), Step::Changed);
    let said = "work  1:1  r2[+]   E492: Not an editor command: x";
    assert_eq!(frame(&mut tui).status, said);
    press(&mut tui, Key::Char('j'));
    assert_eq!(frame(&mut tui).status, "work  2:1  r2[+]   ", "until a key");
}

#[test]
fn q_and_q_bang_leave_with_nothing_to_lose() {
    for line in ["q", "q!"] {
        let rig = work("quit");
        let seen = Seen::default();
        let mut tui = over(&rig, &seen);
        let mut typed = vec![Key::Char(':')];
        typed.extend(line.chars().map(Key::Char));
        typed.push(Key::Enter);
        assert_eq!(keys(&mut tui, &typed), Step::Quit(0), ":{line}");
    }
}

#[test]
fn a_region_over_a_cell_is_none_of_the_explorer_s() {
    let rig = work("cell");
    // A table that hands the explorer a region over a file: it is not
    // the explorer's to show, and it says so before anything is drawn.
    let table: Table = Box::new(|_| {
        Some((String::from("explorer"), reovim_plugin_explorer::boot()))
    });
    let opened = editor(&rig, table, Some("work/a"));
    assert_eq!(opened.err(), Some(Error::Unsupported));
}
