//! `:exp[lore]` over the tui and the rig: what Vim asks its Client to
//! show, and what the region becomes once it has.
//!
//! The table gives Vim for a cell and a recording stub for a
//! Container, so what Vim hands on is judged by what that stub was
//! started over: Vim knows no listing, and the explorer is judged by
//! its own tests. The corpus cannot judge any of this either — nvim's
//! netrw draws a buffer of its own and the runner records Vim's
//! frames, not netrw's — so smoke.sh judges the picture by hand.

use std::rc::Rc;

use reovim_client::dev::{self, Clip, Table, Want};

use reovim_client_tui::{Frame, Step, Tui};
use reovim_kernel_interface::Rd;
use reovim_plugin_vim::notation::keys;
use reovim_rig::{Rig, Seen, Stub};
use reovim_sdk::cell;
use reovim_sdk::walk::ROOT;

/// What `:q` and `:explore` say to unsaved changes.
const E37: &str = "E37: No write since last change (add ! to override)";

/// The one editor a test drives, over whichever Plugin the table gives.
type Editor = Tui;

/// The app's table in the small: Vim for a cell, the stub for a
/// Container and for the name `cli`, which no shape gives. What the
/// cli buffer does with the region is judged by its own tests; what
/// is judged here is what Vim asked for.
fn table(seen: &Seen) -> Table {
    let seen = seen.clone();
    Box::new(move |want| match want {
        Want::Cell => Some((String::from("vim"), reovim_plugin_vim::boot())),
        Want::Container | Want::Named("cli") => Some((
            String::from("listing"),
            Stub::booted(&seen, "-- LISTING --"),
        )),
        Want::Named(_) => None,
    })
}

/// A rig whose root holds `d`, which holds the file `f` and `sub`.
fn work(test: &str) -> Rig {
    let rig = Rig::new(test);
    std::fs::create_dir_all(rig.dir().join("d/sub")).unwrap();
    rig.file("d/f", b"one\ntwo\n");
    rig
}

/// An editor over `d/f`, started in `d` as the app starts it there: a
/// relative name the command gives is resolved under that.
fn over(rig: &Rig, seen: &Seen) -> Editor {
    let host = Rc::new(rig.host());
    let dev =
        dev::boot(&host, table(seen), Clip::none(), "d", Some("d/f"), None)
            .unwrap();
    Tui::open(host, dev).unwrap()
}

/// The rd of `path`, a name under the root, through a Client of its
/// own: what the ask is judged against.
fn rd(rig: &Rig, path: &str) -> Rd {
    let host = rig.host();
    let own = host.own();
    cell::resolve(&own, ROOT, &format!("/module/fs/root/{path}")).unwrap()
}

/// Every key of `text` in Vim notation; what the last one did.
fn typed(tui: &mut Editor, text: &str) -> Step {
    let mut step = Step::Same;
    for key in keys(text).unwrap() {
        step = tui.key(key).unwrap();
    }
    step
}

/// The frame at the size the app always asks for.
fn frame(tui: &mut Editor) -> Frame {
    tui.frame(80, 24).unwrap()
}

/// The status row of that frame.
fn status(tui: &mut Editor) -> String {
    frame(tui).status
}

#[test]
fn explore_shows_the_directory_the_file_is_in() {
    let rig = work("explore-here");
    let seen = Seen::default();
    let mut tui = over(&rig, &seen);
    assert_eq!(frame(&mut tui).lines, ["one", "two"], "the file first");

    assert_eq!(typed(&mut tui, ":explore<CR>"), Step::Changed);
    assert_eq!(*seen.borrow(), [Some(rd(&rig, "d"))], "`d`, not the root");
    // The region is the Container now: it is named after it, and the
    // word in the status row is the stub's, no Vim being there.
    assert_eq!(status(&mut tui), "d  1:1  r1   -- LISTING --");
}

#[test]
fn a_name_is_resolved_under_the_directory_the_client_started_in() {
    let rig = work("explore-name");
    for (line, path) in
        [(":explore sub<CR>", "d/sub"), (":explore /d<CR>", "d")]
    {
        let seen = Seen::default();
        let mut tui = over(&rig, &seen);
        assert_eq!(typed(&mut tui, line), Step::Changed, "{line}");
        assert_eq!(*seen.borrow(), [Some(rd(&rig, path))], "{line}");
    }
}

#[test]
fn a_cell_goes_to_the_table_the_same_way_and_comes_back_a_fresh_vim() {
    let rig = work("explore-cell");
    let seen = Seen::default();
    let mut tui = over(&rig, &seen);
    // The cursor moves, so a Vim that kept its place would say 2:1.
    assert_eq!(typed(&mut tui, "j"), Step::Changed);
    assert_eq!(status(&mut tui), "f  2:1  r1   ");

    assert_eq!(typed(&mut tui, ":explore f<CR>"), Step::Changed);
    assert!(seen.borrow().is_empty(), "a cell is no Container");
    assert_eq!(status(&mut tui), "f  1:1  r1   ", "another Vim over it");
    assert_eq!(frame(&mut tui).lines, ["one", "two"]);
}

#[test]
fn a_name_that_is_not_there_is_e344_and_the_region_stays() {
    let rig = work("explore-e344");
    let seen = Seen::default();
    let mut tui = over(&rig, &seen);
    assert_eq!(typed(&mut tui, ":explore nothere<CR>"), Step::Changed);
    let said = "E344: Can't find directory \"nothere\" in cdpath";
    assert_eq!(status(&mut tui), format!("f  1:1  r1   {said}"));
    assert!(seen.borrow().is_empty(), "nothing was asked for");
    assert_eq!(frame(&mut tui).lines, ["one", "two"], "the file is still it");
}

#[test]
fn a_draft_holds_the_region_until_the_bang() {
    let rig = work("explore-e37");
    let seen = Seen::default();
    let mut tui = over(&rig, &seen);
    assert_eq!(typed(&mut tui, "ix<Esc>"), Step::Changed);

    assert_eq!(typed(&mut tui, ":explore<CR>"), Step::Changed);
    assert_eq!(status(&mut tui), format!("f  1:1  r2[+]   {E37}"));
    assert!(seen.borrow().is_empty(), "the draft would have gone with it");
    assert_eq!(frame(&mut tui).lines, ["xone", "two"], "and it is still here");

    assert_eq!(typed(&mut tui, ":explore!<CR>"), Step::Changed);
    assert_eq!(*seen.borrow(), [Some(rd(&rig, "d"))], "the bang goes");
    assert_eq!(status(&mut tui), "d  1:1  r1   -- LISTING --");
    assert_eq!(rig.disk("d/f"), b"one\ntwo\n", "the draft was never written");
}

#[test]
fn every_beginning_the_table_resolves_is_the_one_command() {
    let rig = work("explore-exp");
    // `ex` is the shortest the table leaves it, `edit` standing
    // before `explore` and taking `e` and `ed`.
    for word in ["ex", "exp", "expl", "explo", "explor", "explore"] {
        let seen = Seen::default();
        let mut tui = over(&rig, &seen);
        let line = format!(":{word}<CR>");
        assert_eq!(typed(&mut tui, &line), Step::Changed, "{line}");
        assert_eq!(*seen.borrow(), [Some(rd(&rig, "d"))], "{line}");
    }

    let seen = Seen::default();
    let mut tui = over(&rig, &seen);
    // `:edit` without a name reloads the file in Vim and does nothing
    // here, and says nothing either way.
    for word in ["e", "ed", "edi", "edit"] {
        assert_eq!(typed(&mut tui, &format!(":{word}<CR>")), Step::Changed);
        assert_eq!(status(&mut tui), "f  1:1  r1   ", "{word}");
    }
    // A name no word of either table begins is E492, as it always was.
    for word in ["expo", "exploree", "Explore"] {
        assert_eq!(typed(&mut tui, &format!(":{word}<CR>")), Step::Changed);
        let said = format!("E492: Not an editor command: {word}");
        assert_eq!(status(&mut tui), format!("f  1:1  r1   {said}"));
    }
    assert!(seen.borrow().is_empty(), "no ask was made");
}

#[test]
fn cli_asks_for_the_cli_buffer_where_the_line_s_words_stand() {
    let rig = work("cli-here");
    let seen = Seen::default();
    let mut tui = over(&rig, &seen);
    // The region goes to the Plugin the name `cli` stands for, over
    // the directory the `:` line's own words stand in.
    assert_eq!(typed(&mut tui, ":cli<CR>"), Step::Changed);
    assert_eq!(*seen.borrow(), [Some(rd(&rig, "d"))], "`d`, not the root");
    assert_eq!(status(&mut tui), "d  1:1  r1   -- LISTING --");
}

#[test]
fn cd_moves_what_cli_opens_over() {
    let rig = work("cli-cd");
    let seen = Seen::default();
    let mut tui = over(&rig, &seen);
    // `:cd` is the cli's own word, and it moves the Shell `:cli`
    // opens over as it moves what `:ls` lists.
    assert_eq!(typed(&mut tui, ":cd sub<CR>"), Step::Changed);
    assert_eq!(typed(&mut tui, ":cli<CR>"), Step::Changed);
    assert_eq!(*seen.borrow(), [Some(rd(&rig, "d/sub"))]);
}

#[test]
fn cl_is_cli_and_a_draft_holds_the_region_until_the_bang() {
    let rig = work("cli-e37");
    let seen = Seen::default();
    let mut tui = over(&rig, &seen);
    assert_eq!(typed(&mut tui, "ix<Esc>"), Step::Changed);

    // `:c` is `cquit`, which would leave with 1; `:cl` is the buffer.
    assert_eq!(typed(&mut tui, ":cl<CR>"), Step::Changed);
    assert_eq!(status(&mut tui), format!("f  1:1  r2[+]   {E37}"));
    assert!(seen.borrow().is_empty(), "the draft would have gone with it");

    assert_eq!(typed(&mut tui, ":cli!<CR>"), Step::Changed);
    assert_eq!(*seen.borrow(), [Some(rd(&rig, "d"))], "the bang goes");
    assert_eq!(status(&mut tui), "d  1:1  r1   -- LISTING --");
    assert_eq!(rig.disk("d/f"), b"one\ntwo\n", "the draft was never written");
}
