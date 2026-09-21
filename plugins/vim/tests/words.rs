//! The `:` line over the tui and the rig: every name under
//! `/self/vim/bin` by its full spelling and by the beginning the
//! listing's order computes for it, and a line that is none of them,
//! which goes to the shell.
//!
//! The corpus judges the short forms it uses and nothing else, so
//! what the full names and the shell's lines do is judged here, by
//! what happened: the step the editor ended in, the status row, the
//! text, the file on disk, the search marks, and what a listing was
//! started over.

use std::rc::Rc;

use reovim_client::Host;
use reovim_client::dev::{Clip, Table, Want};
use reovim_client_api::MATCH;
use reovim_client_tui::{Step, Tui};
use reovim_rig::{Rig, Seen, Stub, listed, opened, typed};
use reovim_sdk::sgr;

/// The one editor a test drives, over whichever Plugin the table gives.
type Editor = Tui;

/// The app's table in the small: Vim for a cell, the stub for a
/// Container.
///
/// One `Editor` across every swap, as the app's own table gives one
/// (01.0).
fn table(seen: &Seen) -> Table {
    let seen = seen.clone();
    let vim = reovim_plugin_vim::Editor::new();
    Box::new(move |want| match want {
        Want::Cell => Some((String::from("vim"), vim.booted())),
        Want::Container => Some((
            String::from("listing"),
            Stub::booted(&seen, "-- LISTING --"),
        )),
        Want::Named(_) => None,
    })
}

/// A rig whose root holds `d`, which holds `alpha`, `note`, `other`
/// and `sub`. The editor is over `note` and was started in `d`.
fn work(test: &str) -> Rig {
    let rig = Rig::new(test);
    std::fs::create_dir_all(rig.dir().join("d/sub")).unwrap();
    rig.file("d/alpha", b"");
    rig.file("d/note", b"one\ntwo\n");
    rig.file("d/other", b"three\n");
    rig
}

/// An editor over `d/note` and the Host it stands in, which is how
/// anything else at this Client reads the tree it registered in.
fn hosted(rig: &Rig, seen: &Seen) -> (Rc<Host>, Editor) {
    let host = rig.host();
    let tui =
        opened(&host, table(seen), Clip::none(), "d", Some("d/note"), None)
            .unwrap();
    (host, tui)
}

/// An editor over `d/note`, started in `d`.
fn over(rig: &Rig, seen: &Seen) -> Editor {
    hosted(rig, seen).1
}

/// What `text` did to an editor of its own, in one line: the step it
/// ended in, the status row, the text shown, what `note` holds on
/// disk, how many search marks are on the frame, and how many
/// listings were started.
fn done(name: &str, keys: &str) -> String {
    let rig = work(name);
    let seen = Seen::default();
    let mut tui = over(&rig, &seen);
    let step = typed(&mut tui, keys);
    let frame = tui.frame(80, 24).unwrap();
    let disk = String::from_utf8(rig.disk("d/note")).unwrap();
    // The editor that quit never showed itself again, so its status
    // row still holds the command line as it was typed, which nobody
    // sees and which is not what the word did.
    let status = match step {
        Step::Quit(_) => String::from("quit"),
        _ => frame.status.clone(),
    };
    let marks = frame
        .runs
        .iter()
        .flatten()
        .filter(|run| run.style == sgr::look(MATCH))
        .count();
    format!(
        "{step:?} [{status}] {:?} disk={disk:?} marks={marks} listed={}",
        frame.lines,
        seen.borrow().len()
    )
}

/// One name of Vim's: a line that uses it in full, the same line by
/// its abbreviation, and what both do.
struct Case {
    full: &'static str,
    short: &'static str,
    done: &'static str,
}

#[test]
fn every_name_runs_by_its_full_spelling_and_by_its_abbreviation() {
    let cases = [
        Case {
            full: "ix<Esc>:write<CR>",
            short: "ix<Esc>:w<CR>",
            done: "Changed [note  1:1  r2   ] [\"xone\", \"two\"] \
                   disk=\"xone\\ntwo\\n\" marks=0 listed=0",
        },
        Case {
            full: "ix<Esc>:wq<CR>",
            short: "ix<Esc>:wq<CR>",
            done: "Quit(0) [quit] [\"xone\", \"two\"] \
                   disk=\"xone\\ntwo\\n\" marks=0 listed=0",
        },
        Case {
            full: ":quit<CR>",
            short: ":q<CR>",
            done: "Quit(0) [quit] [\"one\", \"two\"] \
                   disk=\"one\\ntwo\\n\" marks=0 listed=0",
        },
        Case {
            full: "ix<Esc>:xit<CR>",
            short: "ix<Esc>:x<CR>",
            done: "Quit(0) [quit] [\"xone\", \"two\"] \
                   disk=\"xone\\ntwo\\n\" marks=0 listed=0",
        },
        Case {
            full: ":cquit<CR>",
            short: ":c<CR>",
            done: "Quit(1) [quit] [\"one\", \"two\"] \
                   disk=\"one\\ntwo\\n\" marks=0 listed=0",
        },
        Case {
            full: ":substitute/one/ONE/<CR>",
            short: ":s/one/ONE/<CR>",
            done: "Changed [note  1:1  r2[+]   ] [\"ONE\", \"two\"] \
                   disk=\"one\\ntwo\\n\" marks=0 listed=0",
        },
        Case {
            full: ":set nothere<CR>",
            short: ":se nothere<CR>",
            done: "Changed [note  1:1  r1   E518: Unknown option: nothere] \
                   [\"one\", \"two\"] disk=\"one\\ntwo\\n\" marks=0 listed=0",
        },
        Case {
            full: ":set hls<CR>/one<CR>:nohlsearch<CR>",
            short: ":set hls<CR>/one<CR>:n<CR>",
            done: "Changed [note  1:1  r1   ] [\"one\", \"two\"] \
                   disk=\"one\\ntwo\\n\" marks=0 listed=0",
        },
        Case {
            full: ":edit other<CR>",
            short: ":e other<CR>",
            done: "Changed [other  1:1  r1   ] [\"three\"] \
                   disk=\"one\\ntwo\\n\" marks=0 listed=0",
        },
        Case {
            full: ":explore<CR>",
            short: ":ex<CR>",
            done: "Changed [d  1:1  r1   -- LISTING --] [\"\"] \
                   disk=\"one\\ntwo\\n\" marks=0 listed=1",
        },
    ];
    for (at, case) in cases.iter().enumerate() {
        let full = done(&format!("words-full-{at}"), case.full);
        let short = done(&format!("words-short-{at}"), case.short);
        assert_eq!(full, case.done, "{}", case.full);
        assert_eq!(short, case.done, "{}", case.short);
    }

    // The marks the search left are what `:nohlsearch` took away.
    let marked = done("words-marked", ":set hls<CR>/one<CR>");
    assert!(marked.contains("marks=1"), "{marked}");
}

/// The shortest beginning of `name` the listing resolves to it: its
/// abbreviation, which the order computes and nothing lists by hand.
fn short(names: &[String], name: &str) -> String {
    let first = |n: &usize| {
        let begins = |held: &&String| held.starts_with(&name[..*n]);
        names.iter().find(begins).is_some_and(|held| held == name)
    };
    name[..(1..=name.len()).find(first).unwrap_or(name.len())].to_string()
}

#[test]
fn the_listing_computes_the_short_form_of_every_name() {
    let rig = work("words-order");
    let seen = Seen::default();
    let (host, _tui) = hosted(&rig, &seen);
    let names = listed(&host, "/self/vim/bin");
    let all = [
        "write",
        "wq",
        "quit",
        "xit",
        "cquit",
        "substitute",
        "set",
        "nohlsearch",
        "edit",
        "explore",
        "cli",
        "global",
        "vglobal",
        "delete",
        "buffer",
        "buffers",
        "bnext",
        "bprevious",
        "bdelete",
        "qall",
        "wall",
        "wqall",
    ];
    assert_eq!(names, all, "the order a `:` line resolves a name in");

    // `cquit` is `c` and not nvim's `cq`: no `change` stands before it
    // here, and no name under PATH stands behind it. `cli` stands
    // behind `cquit`, so it is `cl`.
    let short: Vec<String> = names.iter().map(|n| short(&names, n)).collect();
    assert_eq!(
        short,
        [
            "w", "wq", "q", "x", "c", "s", "se", "n", "e", "ex", "cl", "g",
            "v", "d", "b", "buffers", "bn", "bp", "bd", "qa", "wa", "wqa"
        ]
    );

    // A prefix of a shell name still is not its full spelling.
    assert!(!names.iter().any(|name| name.starts_with("wat")));
    assert!(!names.iter().any(|name| name.starts_with('l')));
}

#[test]
fn a_line_of_the_shells_runs_where_the_buffer_is() {
    let rig = work("words-cli");
    let seen = Seen::default();
    let mut tui = over(&rig, &seen);
    let said = |tui: &mut Editor| {
        let status = tui.frame(80, 24).unwrap().status;
        status.split_once("r1   ").map(|(_, said)| said.to_string()).unwrap()
    };

    // `:ls` lists the directory the file is in, and only its first
    // line is said: the rest waits for the cli buffer.
    assert_eq!(typed(&mut tui, ":ls<CR>"), Step::Changed);
    assert_eq!(said(&mut tui), "alpha");
    assert_eq!(typed(&mut tui, ":pwd<CR>"), Step::Changed);
    assert_eq!(said(&mut tui), "/module/fs/root/d");

    // `:cd` moves that line and nothing else.
    assert_eq!(typed(&mut tui, ":cd sub<CR>"), Step::Changed);
    assert_eq!(said(&mut tui), "");
    assert_eq!(typed(&mut tui, ":pwd<CR>"), Step::Changed);
    assert_eq!(said(&mut tui), "/module/fs/root/d/sub");
    assert_eq!(typed(&mut tui, ":cd -<CR>:pwd<CR>"), Step::Changed);
    assert_eq!(said(&mut tui), "/module/fs/root/d");

    // A line that did not run says why as the shell says it, and a
    // first token under no directory of PATH is E492, as it always
    // was.
    assert_eq!(typed(&mut tui, ":ls /nothere<CR>"), Step::Changed);
    assert_eq!(said(&mut tui), "ls /nothere: invalid");
    assert_eq!(typed(&mut tui, ":zz<CR>"), Step::Changed);
    assert_eq!(said(&mut tui), "E492: Not an editor command: zz");

    // The text is the Buffer's throughout: a line of the shell's reads
    // the tree, it does not write the file.
    assert_eq!(tui.frame(80, 24).unwrap().lines, ["one", "two"]);
    assert_eq!(rig.disk("d/note"), b"one\ntwo\n");
}

#[test]
fn a_following_line_is_read_once_and_its_cell_closed() {
    let rig = work("words-follow");
    let seen = Seen::default();
    let mut tui = over(&rig, &seen);
    // `-f` follows: the status row says what the cell held at the one
    // read, a Plugin having no tick to read it again, and the cell is
    // closed after it, which ends the follow.
    assert_eq!(typed(&mut tui, ":cat -f note<CR>"), Step::Changed);
    let status = tui.frame(80, 24).unwrap().status;
    assert_eq!(status.split_once("r1   ").map(|(_, said)| said), Some("one"));
    let mut other = rig.client();
    let cat = other.resolve("/bin/cat").unwrap();
    assert_eq!(other.children(cat), Ok(Vec::new()), "no cell was left");
}

#[test]
fn a_missing_file_opens_a_named_draft_without_creating_it() {
    let rig = work("words-e345");
    let seen = Seen::default();
    let mut tui = over(&rig, &seen);
    assert_eq!(typed(&mut tui, ":edit nothere<CR>"), Step::Changed);
    let frame = tui.frame(80, 24).unwrap();
    assert!(frame.status.starts_with("nothere  "));
    assert_eq!(frame.lines, [""]);
    assert!(!rig.dir().join("d/nothere").exists());
    typed(&mut tui, "ix<Esc>:w<CR>");
    assert_eq!(rig.disk("d/nothere"), b"x\n");
}

#[test]
fn a_line_that_is_only_a_range_is_that_line() {
    let rig = work("words-range");
    let seen = Seen::default();
    let mut tui = over(&rig, &seen);
    let at = |tui: &mut Editor| {
        let status = tui.frame(80, 24).unwrap().status;
        status.split_whitespace().nth(1).unwrap().to_string()
    };
    assert_eq!(typed(&mut tui, ":2<CR>"), Step::Changed);
    assert_eq!(at(&mut tui), "2:1");
    assert_eq!(typed(&mut tui, ":1<CR>"), Step::Changed);
    assert_eq!(at(&mut tui), "1:1");
    assert_eq!(typed(&mut tui, ":$<CR>"), Step::Changed);
    assert_eq!(at(&mut tui), "2:1");
    // A line with nothing on it at all is nothing.
    assert_eq!(typed(&mut tui, ":<CR>"), Step::Changed);
    assert_eq!(at(&mut tui), "2:1");
}
