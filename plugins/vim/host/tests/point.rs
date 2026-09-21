//! `point`: where the cursor or the selection is, as `PATH:LOCATOR`,
//! the cell that reads it and the `<Leader>y` that puts it on the
//! clipboard.
//!
//! The grammar is text's own and the locator is the view's: a line,
//! a line and a column, or two of either with a `-` between them,
//! every number 1-based and every column in bytes, as nvim's `line()`
//! and `col()` say them.

mod common;

use std::fmt::Write as _;
use std::rc::Rc;

use common::cat;
use reovim_client::Host;
use reovim_client::dev::{Clip, Table, Want};
use reovim_client_tui::Tui;
use reovim_plugin_vim_host::Editor;
use reovim_rig::{Rig, listed, opened, said, typed};

/// Twenty lines of seven bytes each, so a column past the fourth is
/// on every one of them.
fn twenty() -> Vec<u8> {
    let written = |mut out: String, n: u32| {
        let _ = writeln!(out, "line-{n:02}");
        out
    };
    (1..=20).fold(String::new(), written).into_bytes()
}

/// An editor over `README.md`, with the clipboard `clip` makes of the
/// rig. Vim is the one Plugin here; no case opens a second region.
fn over(test: &str, clip: impl FnOnce(&Rig) -> Clip) -> (Rig, Tui, Rc<Host>) {
    let rig = Rig::new(test);
    rig.file("README.md", &twenty());
    let host = rig.host();
    let clip = clip(&rig);
    let vim = Editor::new();
    let table: Table = Box::new(move |want| match want {
        Want::Cell => Some((String::from("vim"), vim.booted())),
        _ => None,
    });
    let tui = opened(&host, table, clip, "", Some("README.md"), None).unwrap();
    (rig, tui, host)
}

/// The same, with no machine to yank to.
fn plain(test: &str) -> (Rig, Tui, Rc<Host>) {
    over(test, |_| Clip::none())
}

#[test]
fn the_cursor_is_path_line() {
    let (_rig, mut tui, host) = plain("point-cursor");
    typed(&mut tui, "11G:point<CR>");
    assert_eq!(said(&mut tui), "/README.md:11");

    // The cell is the same reading, for a script that never typed a
    // key: one method with arguments, one cell.
    assert_eq!(cat(&host, "point"), "/README.md:11\n");
    assert_eq!(listed(&host, "/self/vim/bin").len(), 25);
}

#[test]
fn col_and_relative_are_flags() {
    let (rig, mut tui, _host) = plain("point-flags");
    std::fs::create_dir_all(rig.dir().join("sub")).unwrap();
    typed(&mut tui, "11G4l");

    typed(&mut tui, ":point -c<CR>");
    assert_eq!(said(&mut tui), "/README.md:11:5");
    typed(&mut tui, ":point -col<CR>");
    assert_eq!(said(&mut tui), "/README.md:11:5", "the whole name too");
    typed(&mut tui, ":point -r<CR>");
    assert_eq!(said(&mut tui), "README.md:11");
    typed(&mut tui, ":point -c -r<CR>");
    assert_eq!(said(&mut tui), "README.md:11:5", "both");

    // `--relative` is from where the editor stands, which `:cd`
    // moves: a file above it is reached by `..`.
    typed(&mut tui, ":cd sub<CR>");
    typed(&mut tui, ":point -r<CR>");
    assert_eq!(said(&mut tui), "../README.md:11");
    typed(&mut tui, ":point<CR>");
    assert_eq!(said(&mut tui), "/README.md:11", "the absolute one stands");
}

#[test]
fn help_point_says_its_flags() {
    let (_rig, mut tui, _host) = plain("point-help");
    typed(&mut tui, ":help point<CR>");
    assert_eq!(said(&mut tui), "point [-c] [-r]");
}

#[test]
fn a_linewise_selection_is_from_to() {
    let (_rig, mut tui, host) = plain("point-linewise");
    typed(&mut tui, "12GV2j:point<CR>");
    assert_eq!(said(&mut tui), "/README.md:12-14");
    assert_eq!(cat(&host, "mode"), "normal\n");
}

#[test]
fn a_charwise_and_a_block_selection_are_corners() {
    let (_rig, mut tui, _host) = plain("point-corners");
    typed(&mut tui, "12G4lv3j2l:point<CR>");
    assert_eq!(said(&mut tui), "/README.md:12:5-15:7");

    // The corners in text order, whichever one the cursor was on.
    typed(&mut tui, "15G6l<C-v>3k2h:point<CR>");
    assert_eq!(said(&mut tui), "/README.md:12:5-15:7");
}

#[test]
fn a_typed_range_agrees() {
    let (_rig, mut tui, _host) = plain("point-typed-range");
    typed(&mut tui, "12G4lv3j2l<Esc>gv<Esc>");
    typed(&mut tui, ":'<lt>,'>point<CR>");
    assert_eq!(said(&mut tui), "/README.md:12:5-15:7");

    // The key and the line agree: what `'<,'>` names typed is what a
    // line run from the selection sees. A line with no address is
    // the cursor still.
    typed(&mut tui, "12G:point<CR>");
    assert_eq!(said(&mut tui), "/README.md:12", "no range, no selection");
}

#[test]
fn a_draft_has_no_file_name() {
    let rig = Rig::new("point-draft");
    let host = rig.host();
    let vim = Editor::new();
    let table: Table =
        Box::new(move |_| Some((String::from("vim"), vim.booted())));
    let mut tui = opened(&host, table, Clip::none(), "", None, None).unwrap();
    typed(&mut tui, ":point<CR>");
    assert_eq!(said(&mut tui), "E32: No file name");
    assert_eq!(cat(&host, "point"), "E32: No file name\n");
}

#[test]
fn leader_y_puts_the_point_on_the_clipboard() {
    let (rig, mut tui, host) = over("point-leader-y", Rig::clipping);
    typed(&mut tui, "11G\\y");
    assert_eq!(rig.clipboard(), b"/README.md:11");
    assert_eq!(said(&mut tui), "", "a redirect prints nothing");

    // The same key from visual: the binding leaves visual, the line
    // runs over `'<,'>`, and the mode is normal after it.
    typed(&mut tui, "12GV2j\\y");
    assert_eq!(rig.clipboard(), b"/README.md:12-14");
    assert_eq!(cat(&host, "mode"), "normal\n");

    // And the key from normal mode carries none of it: the range is
    // the selection the line came from, not the marks it left behind.
    typed(&mut tui, "11G\\y");
    assert_eq!(rig.clipboard(), b"/README.md:11");
}

#[test]
fn the_keymap_lists_the_default_and_bind_takes_it() {
    let (rig, mut tui, host) = over("point-keymap", Rig::clipping);
    assert_eq!(
        cat(&host, "keymap"),
        "bind <Leader>y point > /self/dev/clipboard\nbind <M-x> cli\n"
    );

    typed(&mut tui, ":bind <lt>Leader>y<CR>");
    assert_eq!(said(&mut tui), "");
    assert_eq!(cat(&host, "keymap"), "bind <M-x> cli\n");

    // `\` is itself again and `y` is the operator it always was.
    typed(&mut tui, "11G\\yy");
    assert_eq!(rig.clipboard(), b"", "nothing reached the machine");
    assert_eq!(cat(&host, "registers/\""), "line-11\n");
}
