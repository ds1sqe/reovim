//! The names of the `:` line as Drivers under `/self/vim/bin`,
//! over the tui and the rig.
//!
//! What the corpus pins is what a person types; what is pinned here is
//! the road it takes: the rest of the line as one argument, the range
//! and the bang as the environment the host set, the run's output as
//! what the status row says, and the same name run from outside every
//! frame of Vim's, where nothing of Vim's is on the stack; the run
//! from inside one is tests/cells.rs's, beside the read it re-enters.

mod common;

use std::fmt::Write as _;
use std::rc::Rc;

use common::cat;
use reovim_client::Host;
use reovim_client::dev::{Clip, Table, Want};
use reovim_client_tui::{Step, Tui};
use reovim_kernel_interface::Rd;
use reovim_plugin_vim::bins::file;
use reovim_rig::{Rig, lines, opened, said, typed};
use reovim_sdk::walk::ROOT;
use reovim_sdk::{cell, ops};

/// Vim for a cell; nothing else is opened here. One `Editor` across
/// every swap, as the app's own table gives one (01.0).
fn table() -> Table {
    let vim = reovim_plugin_vim_host::Editor::new();
    Box::new(move |want| match want {
        Want::Cell => Some((String::from("vim"), vim.booted())),
        _ => None,
    })
}

/// An editor over `d/note`, started in `d`, and the Host it stands in.
fn over(test: &str) -> (Rig, Tui, Rc<Host>) {
    let rig = Rig::new(test);
    std::fs::create_dir_all(rig.dir().join("d")).unwrap();
    rig.file("d/note", b"one a\ntwo a\nthree a\n");
    let host = rig.host();
    let tui = opened(&host, table(), Clip::none(), "d", Some("d/note"), None)
        .unwrap();
    (rig, tui, host)
}

/// An editor over 12 lines of `d/note`, `a 1` through `a 12`, so a
/// range past a single digit has lines of its own to name.
fn twelve(name: &str) -> (Rig, Tui, Rc<Host>) {
    let rig = Rig::new(name);
    std::fs::create_dir_all(rig.dir().join("d")).unwrap();
    let mut lines = String::new();
    for n in 1..=12 {
        writeln!(lines, "a {n}").unwrap();
    }
    rig.file("d/note", lines.as_bytes());
    let host = rig.host();
    let tui = opened(&host, table(), Clip::none(), "d", Some("d/note"), None)
        .unwrap();
    (rig, tui, host)
}

#[test]
fn the_rest_of_the_line_is_one_argument_and_may_hold_a_space() {
    let (rig, mut tui, _host) = over("ex-one-argument");

    // A name with a space in it is one name: the shell splits a line
    // it was given whole, and this line was not.
    assert_eq!(typed(&mut tui, ":w my file<CR>"), Step::Changed);
    assert_eq!(said(&mut tui), "");
    assert_eq!(rig.disk("d/my file"), b"one a\ntwo a\nthree a\n");

    // A spec with a space in it reaches Vim's own parser whole, which
    // is what the corpus's `subst-caret` and `subst-dot-star` need.
    assert_eq!(typed(&mut tui, ":%s/^/> /<CR>"), Step::Changed);
    assert_eq!(lines(&mut tui), ["> one a", "> two a", "> three a"]);
    assert_eq!(typed(&mut tui, ":%s/a$/= 1/<CR>"), Step::Changed);
    assert_eq!(lines(&mut tui), ["> one = 1", "> two = 1", "> three = 1"]);
}

#[test]
fn the_address_and_the_bang_reach_a_run_as_flags() {
    let (_rig, mut tui, _host) = over("ex-address-flags");

    // The address is `substitute`'s own `address` field, filled from
    // the `--address` flag `ex` put on the line and resolved once.
    assert_eq!(typed(&mut tui, ":2,3s/a/x/<CR>"), Step::Changed);
    assert_eq!(lines(&mut tui), ["one a", "two x", "three x"]);
    assert_eq!(typed(&mut tui, "ggVj:s/o/0/<CR>"), Step::Changed);
    assert_eq!(lines(&mut tui), ["0ne a", "tw0 x", "three x"]);

    // Both are flags now, read off the line and not an environment a
    // token never fills: `write` takes `-b`, `--address ADDRESS`
    // (read and not used yet) and a path, `substitute` `--address
    // ADDRESS` and its spec.
    assert_eq!(typed(&mut tui, ":help write<CR>"), Step::Changed);
    assert_eq!(said(&mut tui), "write [-b] [--address ADDRESS] [path]");
    assert_eq!(typed(&mut tui, ":help substitute<CR>"), Step::Changed);
    assert_eq!(said(&mut tui), "substitute [--address ADDRESS] [spec]");

    // The bang is `bang`, which neither of these lines holds a token
    // for: `:q` over a draft is refused and `:q!` is not.
    assert_eq!(typed(&mut tui, ":q<CR>"), Step::Changed);
    let e37 = "E37: No write since last change (add ! to override)";
    assert_eq!(said(&mut tui), e37);
    assert_eq!(typed(&mut tui, ":q!<CR>"), Step::Quit(0));
}

#[test]
fn what_vim_would_have_said_is_what_the_run_printed() {
    let (_rig, mut tui, _host) = over("ex-said");

    // A name prints; it does not fault. Every sentence a person saw
    // arrives as the first line of the run's output cell.
    assert_eq!(typed(&mut tui, "ix<Esc>:q<CR>"), Step::Changed);
    assert_eq!(
        said(&mut tui),
        "E37: No write since last change (add ! to override)"
    );
    assert_eq!(typed(&mut tui, ":set nothere<CR>"), Step::Changed);
    assert_eq!(said(&mut tui), "E518: Unknown option: nothere");
    assert_eq!(typed(&mut tui, ":e nothere<CR>"), Step::Changed);
    assert_eq!(said(&mut tui), "\"nothere\" [New]");
    assert_eq!(typed(&mut tui, ":u<CR>"), Step::Changed);
    assert_eq!(said(&mut tui), "E492: Not an editor command: u", "no `undo`");

    // A run that said nothing leaves the row empty, and the cell it
    // opened is gone again.
    assert_eq!(typed(&mut tui, ":noh<CR>"), Step::Changed);
    assert_eq!(said(&mut tui), "");
}

#[test]
fn a_colon_line_is_a_shell_line() {
    // `:5,10s/a/b/`: the address put back as `--address 5,10`, read
    // once through `Vim::lines_of` and not carried as a second range.
    let (_rig, mut tui, _host) = twelve("ex-run-subst");
    assert_eq!(typed(&mut tui, ":5,10s/a/b/<CR>"), Step::Changed);
    let text = lines(&mut tui);
    for (at, line) in text.iter().enumerate() {
        let changed = (4..=9).contains(&at);
        assert_eq!(line.starts_with('b'), changed, "line {at}: {line}");
    }

    // `:w! my file`: the bang and the whole rest, spaces included, as
    // one shell line — `w --bang my file`.
    let (rig, mut tui, _host) = over("ex-run-write");
    typed(&mut tui, "ix<Esc>:w! my file<CR>");
    assert_eq!(rig.disk("d/my file"), b"xone a\ntwo a\nthree a\n");

    // `:'<,'>d` from visual: the binding's door puts `'<,'>` on the
    // line as text, and `ex` reads it off the front the same as a
    // typed one.
    let (_rig, mut tui, _host) = over("ex-run-visual-delete");
    assert_eq!(typed(&mut tui, "Vj:d<CR>"), Step::Changed);
    assert_eq!(lines(&mut tui), ["three a"]);

    // `:ls /module | grep coreutils`: a first token no name of Vim's
    // begins is the shell's own line, whole — the `|` its own and not
    // Vim's to read at all.
    let (_rig, mut tui, _host) = over("ex-run-pipe");
    assert_eq!(typed(&mut tui, ":ls /module<CR>"), Step::Changed);
    let listed = said(&mut tui);
    assert!(listed.contains("coreutils"), "{listed}");
    assert_eq!(
        typed(&mut tui, ":ls /module | grep coreutils<CR>"),
        Step::Changed
    );
    assert_eq!(said(&mut tui), "coreutils", "the pipe filters it");
}

/// `:{range}w [FILE]`, as nvim 0.12.5 has it (the oracle table in
/// tmp/ranged-write-evidence/00-oracle.md): the lines the address
/// names, each ended by its newline, written to FILE and nothing
/// else, while the draft, its source and its modified flag stay
/// exactly as they were. A write is not a save.
#[test]
fn a_ranged_write_writes_its_lines_and_leaves_the_draft_alone() {
    let (rig, mut tui, _host) = over("ex-range-write");

    assert_eq!(typed(&mut tui, ":2,3w out<CR>"), Step::Changed);
    assert_eq!(said(&mut tui), "", "a write of its own says nothing");
    assert_eq!(rig.disk("d/out"), b"two a\nthree a\n");
    assert_eq!(rig.disk("d/note"), b"one a\ntwo a\nthree a\n");
    assert_eq!(lines(&mut tui), ["one a", "two a", "three a"]);

    // A file that is there already is E13 and takes nothing; the
    // bang writes over it, as `:w! NAME` does.
    assert_eq!(typed(&mut tui, ":1w out<CR>"), Step::Changed);
    assert_eq!(said(&mut tui), "E13: File exists (add ! to override)");
    assert_eq!(rig.disk("d/out"), b"two a\nthree a\n");
    assert_eq!(typed(&mut tui, ":1w! out<CR>"), Step::Changed);
    assert_eq!(said(&mut tui), "");
    assert_eq!(rig.disk("d/out"), b"one a\n");

    // The modified flag is the draft's own: an insert this write
    // carried to another file is still unsaved here, and `:q`
    // refuses it as it would have before the write.
    assert_eq!(typed(&mut tui, "ix<Esc>:2,3w! out<CR>"), Step::Changed);
    assert_eq!(rig.disk("d/out"), b"two a\nthree a\n");
    assert_eq!(rig.disk("d/note"), b"one a\ntwo a\nthree a\n");
    assert_eq!(typed(&mut tui, ":q<CR>"), Step::Changed);
    let e37 = "E37: No write since last change (add ! to override)";
    assert_eq!(said(&mut tui), e37);
}

/// The two refusals nvim words for a range: E140 for a partial buffer
/// with no file of its own to go to, E481 for a range on a name whose
/// grammar has none.
#[test]
fn a_range_with_no_file_is_e140_and_a_range_on_edit_is_e481() {
    let (rig, mut tui, _host) = over("ex-range-refused");

    assert_eq!(typed(&mut tui, "ix<Esc>:2,3w<CR>"), Step::Changed);
    assert_eq!(said(&mut tui), "E140: Use ! to write partial buffer");
    assert_eq!(rig.disk("d/note"), b"one a\ntwo a\nthree a\n");

    // nvim's test is a partial draft over *the draft's own file*,
    // which a name reaches as well as an empty one: E140 either way,
    // and E13 belongs to another file that is there already.
    assert_eq!(typed(&mut tui, ":2,3w note<CR>"), Step::Changed);
    assert_eq!(said(&mut tui), "E140: Use ! to write partial buffer");
    assert_eq!(rig.disk("d/note"), b"one a\ntwo a\nthree a\n");

    // The bang writes the range over the draft's own file, which is
    // what nvim's `:{range}w!` leaves on disk; the draft is untouched.
    assert_eq!(typed(&mut tui, ":2,3w!<CR>"), Step::Changed);
    assert_eq!(said(&mut tui), "");
    assert_eq!(rig.disk("d/note"), b"two a\nthree a\n");
    assert_eq!(lines(&mut tui), ["xone a", "two a", "three a"]);

    // `:e` and reovim's own `:explore` take no range at all.
    assert_eq!(typed(&mut tui, ":2,3e<CR>"), Step::Changed);
    assert_eq!(said(&mut tui), "E481: No range allowed");
    assert_eq!(typed(&mut tui, ":2,3explore<CR>"), Step::Changed);
    assert_eq!(said(&mut tui), "E481: No range allowed");
    assert_eq!(lines(&mut tui), ["xone a", "two a", "three a"]);

    // The bang takes the draft's own file by name too.
    assert_eq!(typed(&mut tui, ":1w! note<CR>"), Step::Changed);
    assert_eq!(said(&mut tui), "");
    assert_eq!(rig.disk("d/note"), b"xone a\n");
}

/// A selection is an address like any other: the `'<,'>` the binding
/// puts on the line names the lines the write takes.
#[test]
fn a_selection_writes_the_lines_it_covers() {
    let (rig, mut tui, _host) = over("ex-visual-write");

    assert_eq!(typed(&mut tui, "Vj:w sel<CR>"), Step::Changed);
    assert_eq!(said(&mut tui), "");
    assert_eq!(rig.disk("d/sel"), b"one a\ntwo a\n");
    assert_eq!(rig.disk("d/note"), b"one a\ntwo a\nthree a\n");
}

#[test]
fn a_name_of_vims_own_runs_from_outside_every_frame_of_its_own() {
    let (rig, mut tui, host) = over("ex-outside");
    assert_eq!(typed(&mut tui, "ix<Esc>"), Step::Changed);

    // Nothing of Vim's is on the stack here: the call comes in from a
    // Client of its own, takes the state afresh, and writes the file
    // the region is editing.
    let ctx = host.own();
    let rd = cell::resolve(&ctx, ROOT, "/self/vim/bin/write").unwrap();
    let args =
        file::Args { bang: false, address: String::new(), path: String::new() };
    let out = ops::call_by_name(&ctx, rd, "run", reovim_sdk::encode(&args));
    let cell: Rd = reovim_sdk::decode(&out.unwrap()).unwrap();
    assert_eq!(ops::bytes(&ctx, cell).unwrap(), b"", "it said nothing");
    assert_eq!(rig.disk("d/note"), b"xone a\ntwo a\nthree a\n");
}

#[test]
fn an_abbreviation_is_a_name_on_path() {
    let (_rig, mut tui, host) = over("ex-abbreviation");

    // `se` is `set`'s own abbreviation, registered under
    // `/self/vim/bin` as an exact name and not resolved at the call
    // by any prefix: the shell's lookup finds it directly, and the
    // option it sets is the same `:set ic` would.
    assert_eq!(cat(&host, "options/ignorecase"), "0\n");
    assert_eq!(typed(&mut tui, ":se ic<CR>"), Step::Changed);
    assert_eq!(cat(&host, "options/ignorecase"), "1\n");
}

/// The host holds no name table and no parser of its own: a run of
/// one of Vim's own names is the shell's, whole, so `typed`,
/// `expanded`, `positionals`, `cut` and `ranged` are none of its
/// functions any more. Its own source is what says so, read here and
/// asked of no tool outside the test.
#[test]
fn the_host_holds_no_parser() {
    let src = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src");
    let gone = ["typed", "expanded", "positionals", "cut", "ranged"];
    for file in std::fs::read_dir(&src).unwrap() {
        let file = file.unwrap().path();
        let text = std::fs::read_to_string(&file).unwrap();
        for name in gone {
            let declared = format!("fn {name}(");
            assert!(
                !text.contains(&declared),
                "{} still declares {declared}",
                file.display()
            );
        }
    }
}
