//! Marks over the tui and the rig: what `moved` alone cannot show,
//! one Buffer's local table kept apart from another's, a file mark
//! left untouched by edits where it is not loaded, an operator that
//! cannot reach across a swap, `gv` after an edit above it, and the
//! cell settlement 9 gives.

mod common;

use std::rc::Rc;

use common::{cat, one};
use reovim_client::Host;
use reovim_client::dev::Clip;
use reovim_client_tui::{Step, Tui};
use reovim_plugin_vim::Editor;
use reovim_rig::{Rig, lines, opened, said, typed};

/// The tree over `files`, opened on the first, one `Editor` given for
/// every swap. `Rig` is kept alive alongside what it serves: its own
/// temporary tree is gone once it drops, which a swap to another of
/// `files` still needs to read.
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

#[test]
fn marks_are_kept_per_buffer_across_e() {
    let (_rig, _host, mut tui) =
        over("marks-per-buffer", &[("a", b"one\ntwo\n"), ("b", b"three\n")]);

    typed(&mut tui, "jma");
    assert_eq!(typed(&mut tui, ":e b<CR>"), Step::Changed);
    typed(&mut tui, "'a");
    assert_eq!(said(&mut tui), "E20: Mark not set", "b's own, empty table");
    assert_eq!(typed(&mut tui, ":e a<CR>"), Step::Changed);
    typed(&mut tui, "gg'ax");
    assert_eq!(lines(&mut tui)[..2], ["one", "wo"], "a's mark, kept for it");
}

#[test]
fn a_file_mark_in_another_buffer_is_not_moved_by_edits_here() {
    let (_rig, _host, mut tui) = over(
        "marks-file-elsewhere",
        &[("a", b"one\ntwo\nthree\n"), ("b", b"x\ny\n")],
    );

    typed(&mut tui, "jmA");
    assert_eq!(typed(&mut tui, ":e b<CR>"), Step::Changed);
    // Lines put above in `b`: if `A` were moved by edits here, its
    // line would shift, though it names `a` and not `b`. Saved first,
    // so leaving is not E37's to refuse.
    typed(&mut tui, "Onew<Esc>Onew<Esc>");
    assert_eq!(typed(&mut tui, ":w<CR>"), Step::Changed);
    assert_eq!(typed(&mut tui, "'A"), Step::Changed, "back to a");
    typed(&mut tui, "x");
    assert_eq!(
        lines(&mut tui)[..3],
        ["one", "wo", "three"],
        "A still on two, untouched by b's own edits"
    );
}

#[test]
fn a_mark_under_an_operator_in_another_buffer_does_nothing() {
    let (_rig, _host, mut tui) = over(
        "marks-operator-elsewhere",
        &[("a", b"one\ntwo\n"), ("b", b"three\n")],
    );

    typed(&mut tui, "mA");
    assert_eq!(typed(&mut tui, ":e b<CR>"), Step::Changed);
    typed(&mut tui, "d'A");
    assert_eq!(lines(&mut tui)[..1], ["three"], "nothing deleted; no swap");
}

#[test]
fn a_mark_set_after_the_change_survives_its_undo() {
    // `dd`'s own snapshot holds no `b`: `u` extends the current marks
    // with it rather than replacing them, so `b`, set after `dd`,
    // survives where a wholesale restore would have wiped it.
    let (_rig, _host, mut tui) =
        over("marks-undo-survives", &[("a", b"one\ntwo\nthree\n")]);
    assert_eq!(typed(&mut tui, "dd"), Step::Changed, "one is gone");
    typed(&mut tui, "jmb");
    assert_eq!(typed(&mut tui, "u"), Step::Changed, "one is back");
    typed(&mut tui, "'bx");
    assert_eq!(
        lines(&mut tui)[..3],
        ["one", "two", "hree"],
        "b, set after dd, still on three's own line"
    );
}

#[test]
fn undo_does_not_touch_the_visual_marks_set_after_it() {
    // `<` and `>` are not among the `a`-`z` a revision snapshots, so
    // `u` neither restores nor wipes them: they are the selection
    // made after `dd`, not `dd`'s own empty table of them.
    let (_rig, _host, mut tui) =
        over("marks-undo-visual", &[("a", b"one\ntwo\nthree\n")]);
    assert_eq!(typed(&mut tui, "dd"), Step::Changed);
    typed(&mut tui, "vj<Esc>");
    assert_eq!(typed(&mut tui, "u"), Step::Changed);
    // `said` right after the jump alone: a further key's own
    // `message.clear()` would hide E20 if this read it after `x` too.
    typed(&mut tui, "`>");
    assert_eq!(said(&mut tui), "", "not wiped by the undo, so no E20");
}

#[test]
fn a_range_reads_a_file_mark_of_this_source() {
    // `range`'s own `mark` closure used to read only `marks.local`,
    // which never holds `A`-`Z` at all: `:'A,'A` was E20 even in the
    // Buffer `A` itself names. `found_mark`, filtered to a source
    // that is this one's (`None` for elsewhere), is the fix.
    let (_rig, _host, mut tui) =
        over("marks-range-file", &[("a", b"one\ntwo\nthree\n")]);
    typed(&mut tui, "jmA");
    assert_eq!(typed(&mut tui, ":'A,'As/./X/<CR>"), Step::Changed);
    assert_eq!(
        lines(&mut tui)[..3],
        ["one", "Xwo", "three"],
        "the range named by A alone, both ends"
    );
}

#[test]
fn a_file_mark_refused_by_e37_leaves_the_cursor_kept() {
    // A6: `arrive` must not be set until `shown` says the ask went
    // out; E37 refuses it, so the next `:e` of the same Buffer must
    // land on the cursor `gg` kept, not hijacked onto A's own line.
    let (_rig, _host, mut tui) = over(
        "marks-e37-kept",
        &[("a", b"one\ntwo\nthree\n"), ("b", b"x\ny\n")],
    );
    typed(&mut tui, "jmAgg");
    assert_eq!(typed(&mut tui, ":e new<CR>"), Step::Changed);
    typed(&mut tui, "ix<Esc>");
    typed(&mut tui, "'A");
    assert_eq!(
        said(&mut tui),
        "E37: No write since last change (add ! to override)"
    );
    assert_eq!(typed(&mut tui, ":w<CR>"), Step::Changed);
    assert_eq!(typed(&mut tui, ":e a<CR>"), Step::Changed);
    typed(&mut tui, "x");
    assert_eq!(
        lines(&mut tui)[..3],
        ["ne", "two", "three"],
        "line 0, where gg left it, is what changed"
    );
}

#[test]
fn arriving_at_a_buffer_with_a_place_keeps_its_local_marks_and_undo() {
    // A6: `opened` restores the `Place` before it takes `arrive`, so
    // a file mark's swap back lands on a Buffer whose own undo list
    // and local marks are the ones it left with, not empty ones.
    let (_rig, _host, mut tui) = over(
        "marks-place-kept",
        &[("a", b"one\ntwo\nthree\n"), ("b", b"x\ny\n")],
    );
    typed(&mut tui, "xmb");
    assert_eq!(typed(&mut tui, ":w<CR>"), Step::Changed);
    typed(&mut tui, "mA");
    assert_eq!(typed(&mut tui, ":e b<CR>"), Step::Changed);
    assert_eq!(typed(&mut tui, "'A"), Step::Changed, "arrives back at a");
    assert_eq!(typed(&mut tui, "u"), Step::Changed, "a's own undo list");
    assert_eq!(lines(&mut tui)[..1], ["one"], "the text back");
    typed(&mut tui, "'b");
    assert_eq!(said(&mut tui), "", "b, a's own local mark, not E20");
}

#[test]
fn the_marks_cell_lists_what_is_set() {
    let (_rig, host, mut tui) = over("marks-cell", &[("a", b"one\ntwo\n")]);

    // `k` and not `gg`: a jump would set `'` too, and this counts
    // only the marks it names.
    typed(&mut tui, "jma");
    typed(&mut tui, "kmB");
    let listing = cat(&host, "marks");
    let rows: Vec<&str> = listing.lines().collect();
    assert_eq!(rows[0], "a 2 0", "local, line from 1, column from 0");
    assert!(rows[1].starts_with("B 1 0 "), "a file mark names its source");
}
