//! The jump list over the tui and the rig: what the corpus cannot
//! record — a stay left for the cli, a Buffer over nothing, the
//! wildmenu's own Tab, a crossing landing on its column, a crossing
//! E37 refuses — and the cell settlement 8 gives.

mod common;

use std::rc::Rc;

use common::{cat, one};
use reovim_client::Host;
use reovim_client::dev::{Clip, Table, Want};
use reovim_client_region::Region;
use reovim_client_tui::{Step, Tui};
use reovim_kernel_interface::{Calls, Error, Rd};
use reovim_plugin_vim::Editor;
use reovim_rig::{Rig, Seen, Stub, frame, lines, opened, said, typed};

/// What `:q` and a crossing E37 refuses say to unsaved changes.
const E37: &str = "E37: No write since last change (add ! to override)";

/// The tree over `files`, opened on the first, one `Editor` given for
/// every swap.
fn over(name: &str, files: &[(&str, &[u8])]) -> (Rig, Rc<Host>, Tui) {
    let rig = Rig::new(name);
    for &(path, bytes) in files {
        rig.file(path, bytes);
    }
    let vim = Editor::new();
    let host = rig.host();
    let first = files.first().map(|&(path, _)| path);
    let tui = opened(&host, one(vim), Clip::none(), "", first, None).unwrap();
    (rig, host, tui)
}

/// What each start of the stub was over, and what it does at the
/// first frame that took a key: hand the region straight back.
type Script = Box<dyn Fn(&dyn Calls, &Region, Option<Rd>) -> Result<(), Error>>;

fn gives_it_back() -> Script {
    Box::new(|ctx, region, _| region.backs(ctx))
}

#[test]
fn a_stay_left_for_the_cli_is_one_jump_and_back_lands_where_it_was() {
    let rig = Rig::new("jumps-cli");
    rig.file("note", b"one\ntwo\nthree\n");
    let vim = Editor::new();
    let seen = Seen::default();
    let table: Table = Box::new(move |want| match want {
        Want::Named("cli") => Some((
            String::from("cli"),
            Stub::typing(&seen, "$ ", gives_it_back()),
        )),
        _ => Some((String::from("vim"), vim.booted())),
    });
    let host = rig.host();
    let mut tui =
        opened(&host, table, Clip::none(), "", Some("note"), None).unwrap();

    // `3G` pushes where the cursor stood, line 0; `:cli` (a door of
    // its own, not a walk of the list) pushes where it left the
    // Buffer, line 2, once — the stub hands the region straight back.
    typed(&mut tui, "3G");
    assert_eq!(typed(&mut tui, ":cli<CR>"), Step::Changed, "the cli shown");
    assert_eq!(said(&mut tui), "$ ");
    assert_eq!(typed(&mut tui, " "), Step::Changed, "vim shown again");
    assert_eq!(lines(&mut tui)[..3], ["one", "two", "three"], "back on note");

    assert_eq!(typed(&mut tui, "<C-o>x"), Step::Changed);
    assert_eq!(lines(&mut tui)[..1], ["ne"], "line 0, not line 2 twice over");
}

#[test]
fn a_buffer_over_nothing_keeps_its_jumps_at_its_first_write() {
    let rig = Rig::new("jumps-sourced");
    let vim = Editor::new();
    let host = rig.host();
    let mut tui =
        opened(&host, one(vim), Clip::none(), "", Some("new"), None).unwrap();

    typed(&mut tui, "ione<CR>two<CR>three<Esc>");
    typed(&mut tui, "gg");
    assert_eq!(typed(&mut tui, "G"), Step::Changed, "the jump, gg's line kept");
    assert_eq!(typed(&mut tui, ":w<CR>"), Step::Changed, "sourced now");
    assert_eq!(typed(&mut tui, "<C-o>"), Step::Changed, "still tracked");
    assert_eq!(typed(&mut tui, "x"), Step::Changed);
    assert_eq!(lines(&mut tui)[..1], ["on"], "landed back on gg's own line");
}

#[test]
fn a_bare_address_pushes_nothing_and_c_o_finds_the_seed() {
    // A Buffer is opened with its jump list already seeded at line 0
    // (`opened`), so `<C-o>` has somewhere to go from the very first
    // key. `:3` is not a jump: it pushes nothing of its own, so a
    // `<C-o>` right after it still finds the seed, not line 2
    // (`jump-ex-line-sets-previous` in the corpus, its own case of
    // this).
    let (_rig, _host, mut tui) =
        over("jumps-bare-address", &[("a", b"a\nb\nc\n")]);

    assert_eq!(typed(&mut tui, ":3<CR>"), Step::Changed, "no push of its own");
    assert_eq!(typed(&mut tui, "<C-o>"), Step::Changed, "the seed, not line 2");
    assert_eq!(typed(&mut tui, "x"), Step::Changed);
    assert_eq!(lines(&mut tui)[..1], [""], "line 0, not where :3 left it");
}

#[test]
fn a_c_o_on_the_seeds_own_line_pops_it_and_refuses() {
    // What looked like a `<C-o>` spending some one-time chance
    // (`jump-refused-empty` in the corpus): the seed sits at line 0,
    // where the cursor already stands, so the walk's own `clean` pops
    // it before answering — nothing to walk to, and nothing left
    // after it for a later `<C-o>` to find either.
    let (_rig, host, mut tui) =
        over("jumps-tried-first", &[("a", b"a\nb\nc\n")]);

    assert_eq!(
        typed(&mut tui, "<C-o>"),
        Step::Same,
        "the seed popped, not walked"
    );
    assert_eq!(cat(&host, "jumps"), "", "the seed gone, not stood on");
    typed(&mut tui, "jj");
    assert_eq!(typed(&mut tui, "<C-o>"), Step::Same, "nothing left to answer");
}

#[test]
fn opened_at_a_line_still_seeds_line_0_for_c_o_to_find() {
    let rig = Rig::new("jumps-plus-3");
    rig.file("a", b"a\nb\nc\nd\ne\n");
    let vim = Editor::new();
    let host = rig.host();
    let mut tui =
        opened(&host, one(vim), Clip::none(), "", Some("a"), Some(3)).unwrap();

    assert_eq!(frame(&mut tui).cursor.0, 2, "opened on line 3, 1-indexed");
    assert_eq!(
        typed(&mut tui, "<C-o>"),
        Step::Changed,
        "the seed, not refused"
    );
    assert_eq!(typed(&mut tui, "x"), Step::Changed);
    assert_eq!(lines(&mut tui)[..1], [""], "line 0, the seed named by start");
}

#[test]
fn tab_on_the_colon_line_completes_and_moves_no_jump() {
    let (_rig, _host, mut tui) =
        over("jumps-tab-colon", &[("a", b"a\nb\nc\n")]);

    typed(&mut tui, "G");
    typed(&mut tui, "gg");
    assert_eq!(typed(&mut tui, "<C-o>"), Step::Changed, "back onto c");
    // `:se<Tab>` is the wildmenu's own Tab, command mode's; it must
    // not touch the jump list `<C-o>` just walked.
    typed(&mut tui, ":se<Tab>");
    typed(&mut tui, "<Esc>");
    assert_eq!(typed(&mut tui, "<Tab>"), Step::Changed, "forth, not stuck");
    assert_eq!(typed(&mut tui, "x"), Step::Changed);
    assert_eq!(lines(&mut tui)[..1], [""], "line 0, gg's own jump reached");
}

#[test]
fn a_jump_into_another_buffer_lands_on_its_column() {
    let (_rig, _host, mut tui) =
        over("jumps-other-column", &[("a", b"one two\n"), ("b", b"x\n")]);

    typed(&mut tui, "w");
    assert_eq!(typed(&mut tui, ":e b<CR>"), Step::Changed);
    assert_eq!(typed(&mut tui, "<C-o>"), Step::Changed, "crossed back to a");
    assert_eq!(typed(&mut tui, "x"), Step::Changed);
    assert_eq!(lines(&mut tui)[..1], ["one wo"], "landed on two's own column");
}

#[test]
fn a_crossing_refused_by_e37_leaves_the_next_door_still_a_jump() {
    // `arrive` is set to `Land::Walked` only once the ask has gone
    // out, never by the refused crossing below, so `leave`'s own
    // `matches!` on it sees no walk here and still pushes.
    let (_rig, _host, mut tui) = over(
        "jumps-crossing-e37",
        &[("a", b"one\ntwo\nthree\n"), ("b", b"x\ny\nz\n")],
    );

    typed(&mut tui, "G");
    assert_eq!(typed(&mut tui, ":e b<CR>"), Step::Changed);
    typed(&mut tui, "x");
    typed(&mut tui, "<C-o>");
    assert_eq!(said(&mut tui), E37, "b unsaved, the crossing refused");
    assert_eq!(typed(&mut tui, ":w<CR>"), Step::Changed);
    typed(&mut tui, "G");

    // A normal door out of `b`, not a walk of the list: it must still
    // be a jump, or the stuck `crossing` above would have swallowed
    // it.
    assert_eq!(typed(&mut tui, ":e a<CR>"), Step::Changed);
    assert_eq!(typed(&mut tui, "<C-o>"), Step::Changed, "crosses back to b");
    assert_eq!(typed(&mut tui, "x"), Step::Changed);
    assert_eq!(lines(&mut tui)[..3], ["", "y", ""], "b's own last line, G's");
}

#[test]
fn a_jump_to_a_line_the_tail_took_says_e19() {
    // `jump-tail-gone`'s own case, engine-side: the corpus judges the
    // file, the cursor and the register, never a message, so nothing
    // there pins this. Probed against nvim 0.12.5 directly
    // (`v:errmsg` after the same keys): "E19: Mark has invalid line
    // number", the wording this asserts.
    let (_rig, _host, mut tui) =
        over("jumps-e19", &[("a", b"a\nb\nc\nd\ne\n")]);

    typed(&mut tui, "j");
    assert_eq!(typed(&mut tui, "dG"), Step::Changed, "b..e gone, a left");
    assert_eq!(typed(&mut tui, "<C-o>"), Step::Changed, "the ask, refused");
    assert_eq!(said(&mut tui), "E19: Mark has invalid line number");
    assert_eq!(lines(&mut tui)[..1], ["a"], "nothing moved");
}

#[test]
fn the_jumps_cell_lists_the_list() {
    let (_rig, host, mut tui) =
        over("jumps-cell", &[("a", b"one\ntwo\nthree\nfour\n"), ("b", b"x\n")]);

    typed(&mut tui, "G");
    typed(&mut tui, "gg");
    assert_eq!(
        cat(&host, "jumps"),
        " 1 0\n 4 0\n",
        "oldest first, nothing stood on yet"
    );

    typed(&mut tui, "<C-o>");
    assert_eq!(
        cat(&host, "jumps"),
        " 1 0\n>4 0\n 1 0\n",
        "walk's own push, raw: line 0 stands twice until the next clean"
    );

    assert_eq!(typed(&mut tui, ":e b<CR>"), Step::Changed);
    let listing = cat(&host, "jumps");
    let rows: Vec<&str> = listing.lines().collect();
    assert_eq!(rows.len(), 3, "the un-cleaned duplicate still stands");
    assert!(rows[0].starts_with(" 1 0 "), "a's line 1, another source named");
    assert!(
        rows[0].ends_with("/a"),
        "where_is's own half, not just its prefix"
    );
    assert!(rows[1].starts_with(" 1 0 "), "the raw duplicate, still there");
    assert!(rows[1].ends_with("/a"));
    assert!(rows[2].starts_with(" 4 0 "), "a's line 4, pushed fresh by leave");
    assert!(rows[2].ends_with("/a"));
}
