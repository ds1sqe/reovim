//! What the tui hands Vim when it starts it, through the tui on the
//! rig: a name that is not on disk — what the status says on the
//! first frame, what `:q` leaves, what the first write makes — and
//! the line `+N` begins at. The corpus cannot judge these: nvim
//! records no message, a case's file is always there, and a case
//! always begins at the first line.

use std::rc::Rc;

use reovim_client::Host;
use reovim_client::dev::{self, Table};
use reovim_client_tui::{Step, Tui};
use reovim_plugin_vim::Editor;
use reovim_plugin_vim::notation::keys;
use reovim_rig::Rig;

/// What a write says when the file it would make is there already.
const E13: &str = "E13: File exists (add ! to override)";
/// What a write says when the host would not make the file.
const E212: &str = "E212: Can't open file for writing";

/// The editor `file` opens in, started in `dir`, at `line`: the tui
/// and the Vim the table gave it.
fn editor(
    rig: &Rig,
    dir: &str,
    file: Option<&str>,
    line: Option<u64>,
) -> (Tui, Rc<Editor>) {
    let vim = Editor::new();
    let one = vim.clone();
    let table: Table =
        Box::new(move |_| Some((String::from("vim"), one.booted())));
    let host: Rc<Host> = Rc::new(rig.host());
    let dev = dev::boot(&host, table, dir, file, line).unwrap();
    (Tui::open(host, dev).unwrap(), vim)
}

/// `file`, a name under the root, in an editor of its own.
fn open(rig: &Rig, file: &str) -> Tui {
    editor(rig, "", Some(file), None).0
}

/// Every key of `text` in Vim notation; what the last one did.
fn typed(tui: &mut Tui, text: &str) -> Step {
    let mut step = Step::Same;
    for key in keys(text).unwrap() {
        step = tui.key(key).unwrap();
    }
    step
}

/// The status row at the size the app asks for.
fn status(tui: &mut Tui) -> String {
    tui.frame(80, 24).unwrap().status
}

#[test]
fn a_new_file_is_announced_and_made_by_its_first_write() {
    let rig = Rig::new("new-file");
    let mut tui = open(&rig, "memo.md");
    assert_eq!(status(&mut tui), "memo.md  1:1  r1   \"memo.md\" [New]");

    // `:q` leaves nothing behind: nothing was opened, nothing made.
    assert_eq!(typed(&mut tui, ":q<CR>"), Step::Quit(0));
    assert!(!rig.dir().join("memo.md").exists(), ":q made a file");

    // `:w` on an untouched new Buffer writes it empty, as Vim does.
    let mut tui = open(&rig, "memo.md");
    assert_eq!(typed(&mut tui, ":w<CR>"), Step::Changed);
    assert_eq!(rig.disk("memo.md"), b"", "an empty file");
    assert_eq!(status(&mut tui), "memo.md  1:1  r1   ");
    assert_eq!(typed(&mut tui, ":q<CR>"), Step::Quit(0));

    // And a file already there is opened, not announced.
    let mut tui = open(&rig, "memo.md");
    assert_eq!(status(&mut tui), "memo.md  1:1  r1   ");
}

#[test]
fn typing_then_wq_makes_the_file_and_leaves() {
    let rig = Rig::new("new-file-wq");
    let mut tui = open(&rig, "memo.md");
    assert_eq!(typed(&mut tui, "ihi<Esc>"), Step::Changed);
    assert!(!rig.dir().join("memo.md").exists(), "not until the write");
    assert_eq!(typed(&mut tui, ":wq<CR>"), Step::Quit(0));
    assert_eq!(rig.disk("memo.md"), b"hi\n", "fixendofline writes the \\n");
}

#[test]
fn an_untouched_new_buffer_writes_nothing_and_leaves() {
    let rig = Rig::new("new-file-zz");
    let mut tui = open(&rig, "memo.md");
    // `:x` and `ZZ` write only what has changed; nothing has.
    assert_eq!(typed(&mut tui, "ZZ"), Step::Quit(0));
    assert!(!rig.dir().join("memo.md").exists(), "ZZ made a file");
}

#[test]
fn a_missing_directory_is_made_on_the_way_to_the_file() {
    let rig = Rig::new("new-file-dir");
    let mut tui = open(&rig, "a/b/x");
    // The status says what it will be called; the word, the path.
    assert_eq!(status(&mut tui), "x  1:1  r1   \"a/b/x\" [New DIRECTORY]");
    assert_eq!(typed(&mut tui, ":q<CR>"), Step::Quit(0));
    assert!(!rig.dir().join("a").exists(), ":q made a directory");

    let mut tui = open(&rig, "a/b/x");
    assert_eq!(typed(&mut tui, "iy<Esc>:w<CR>"), Step::Changed);
    assert!(rig.dir().join("a/b").is_dir(), "the directories on the way");
    assert_eq!(rig.disk("a/b/x"), b"y\n");
    assert_eq!(status(&mut tui), "x  1:1  r3   ", "the Buffer is that file");
}

#[test]
fn a_file_that_appeared_meanwhile_is_e13_until_the_bang() {
    let rig = Rig::new("new-file-e13");
    let mut tui = open(&rig, "memo.md");
    assert_eq!(typed(&mut tui, "ihi<Esc>"), Step::Changed);
    // Somebody else writes that name between the open and the write.
    rig.file("memo.md", b"theirs\n");

    assert_eq!(typed(&mut tui, ":w<CR>"), Step::Changed);
    assert!(status(&mut tui).ends_with(E13), "{}", status(&mut tui));
    assert_eq!(rig.disk("memo.md"), b"theirs\n", "theirs is untouched");

    assert_eq!(typed(&mut tui, ":w!<CR>"), Step::Changed);
    assert_eq!(rig.disk("memo.md"), b"hi\n", "the bang writes over it");
    assert_eq!(status(&mut tui), "memo.md  1:2  r3   ");
}

#[test]
fn the_new_name_is_said_as_the_person_gave_it() {
    let rig = Rig::new("new-file-said");
    std::fs::create_dir(rig.dir().join("d")).unwrap();
    // The Client started in `d`, so `d/memo.md` was typed `memo.md`.
    let mut tui = editor(&rig, "d", Some("d/memo.md"), None).0;
    assert_eq!(status(&mut tui), "memo.md  1:1  r1   \"memo.md\" [New]");
    assert_eq!(typed(&mut tui, "iz<Esc>:w<CR>"), Step::Changed);
    assert_eq!(rig.disk("d/memo.md"), b"z\n");
}

#[test]
fn the_line_of_the_command_line_is_where_vim_begins() {
    let rig = Rig::new("plus-line");
    rig.file("note", b"one\n  two\nthree\n");
    let at = |line| {
        let (_tui, vim) = editor(&rig, "", Some("note"), line);
        vim.engine().position()
    };
    // Counted from 1, on the line's first non-blank, as `+N` lands.
    assert_eq!(at(None), (0, 0), "the first line without one");
    assert_eq!(at(Some(1)), (0, 0));
    assert_eq!(at(Some(2)), (1, 2), "past the indent");
    assert_eq!(at(Some(3)), (2, 0));
    assert_eq!(at(Some(9)), (2, 0), "clamped to the last line");
}

#[test]
fn a_file_where_a_directory_is_wanted_is_e212() {
    let rig = Rig::new("new-file-e212");
    rig.file("note", b"first\n");
    // `note` is a file, so nothing can be made below it.
    let mut tui = open(&rig, "note/deep");
    assert_eq!(
        status(&mut tui),
        "deep  1:1  r1   \"note/deep\" [New DIRECTORY]"
    );
    assert_eq!(typed(&mut tui, ":w<CR>"), Step::Changed);
    assert!(status(&mut tui).ends_with(E212), "{}", status(&mut tui));
    assert_eq!(rig.disk("note"), b"first\n", "the file is untouched");
}

#[test]
fn a_buffer_with_no_name_at_all_still_has_no_file_to_write() {
    let rig = Rig::new("new-file-none");
    let mut tui = editor(&rig, "", None, None).0;
    assert_eq!(status(&mut tui), "[No Name]  1:1  r1   ");
    assert_eq!(typed(&mut tui, "ihi<Esc>:w<CR>"), Step::Changed);
    assert!(status(&mut tui).ends_with("E32: No file name"));

    // `:w NAME` makes it, as it always did, and names the Buffer.
    assert_eq!(typed(&mut tui, ":w note<CR>"), Step::Changed);
    assert_eq!(rig.disk("note"), b"hi\n");
    assert_eq!(status(&mut tui), "note  1:2  r3   ");
}
