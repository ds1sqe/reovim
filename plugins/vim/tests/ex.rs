//! The names of the `:` line as Drivers under `/self/vim/bin`,
//! over the tui and the rig.
//!
//! What the corpus pins is what a person types; what is pinned here is
//! the road it takes: the rest of the line as one argument, the range
//! and the bang as the environment the host set, the run's output as
//! what the status row says, and the same name run from outside every
//! frame of Vim's, where nothing of Vim's is on the stack; the run
//! from inside one is tests/cells.rs's, beside the read it re-enters.

use std::rc::Rc;

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
    let vim = reovim_plugin_vim::Editor::new();
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
fn the_range_and_the_bang_reach_a_run_as_the_environment() {
    let (_rig, mut tui, _host) = over("ex-environment");

    // The range is `from` and `to` of `substitute`'s record, lines
    // from 0, and no token of the line names either.
    assert_eq!(typed(&mut tui, ":2,3s/a/x/<CR>"), Step::Changed);
    assert_eq!(lines(&mut tui), ["one a", "two x", "three x"]);
    assert_eq!(typed(&mut tui, "ggVj:s/o/0/<CR>"), Step::Changed);
    assert_eq!(lines(&mut tui), ["0ne a", "tw0 x", "three x"]);

    // Neither reaches a usage line: the environment is the host's and
    // no token ever fills it, so `write` takes a path and nothing else
    // and `substitute` takes its spec.
    assert_eq!(typed(&mut tui, ":help write<CR>"), Step::Changed);
    assert_eq!(said(&mut tui), "write [path]");
    assert_eq!(typed(&mut tui, ":help substitute<CR>"), Step::Changed);
    assert_eq!(said(&mut tui), "substitute [spec]");

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
fn a_name_of_vims_own_runs_from_outside_every_frame_of_its_own() {
    let (rig, mut tui, host) = over("ex-outside");
    assert_eq!(typed(&mut tui, "ix<Esc>"), Step::Changed);

    // Nothing of Vim's is on the stack here: the call comes in from a
    // Client of its own, takes the state afresh, and writes the file
    // the region is editing.
    let ctx = host.own();
    let rd = cell::resolve(&ctx, ROOT, "/self/vim/bin/write").unwrap();
    let args = file::Args { bang: false, path: String::new() };
    let out = ops::call_by_name(&ctx, rd, "run", reovim_sdk::encode(&args));
    let cell: Rd = reovim_sdk::decode(&out.unwrap()).unwrap();
    assert_eq!(ops::bytes(&ctx, cell).unwrap(), b"", "it said nothing");
    assert_eq!(rig.disk("d/note"), b"xone a\ntwo a\nthree a\n");
}
