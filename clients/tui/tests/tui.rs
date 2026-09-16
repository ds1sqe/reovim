//! The tui on a Kernel with fs and text, beside a second Client.

use reovim_client::{Client, Fault, Key, Rd};
use reovim_client_tui::{Step, Tui};
use reovim_client_vim::notation::keys;
use reovim_rig::{Limits, Rig};

/// `text` in Vim notation, key by key; the last step.
fn typed(tui: &mut Tui, text: &str) -> Result<Step, Fault> {
    let mut last = Ok(Step::Same);
    for key in keys(text).unwrap() {
        last = tui.key(key);
    }
    last
}

fn buffer_of(client: &mut Client, tui: &Tui) -> Rd {
    let buffer = tui.buffer();
    let name = client.name(buffer).unwrap();
    assert_eq!(client.resolve(&format!("/module/text/{name}")), Ok(buffer));
    // The tui was the first to connect: session 1, its one client.
    let clients = client.resolve("/session/1/client").unwrap();
    let first = client.children(clients).unwrap()[0];
    let capability = client.lookup(first, "capability").unwrap();
    assert_eq!(client.read_all(capability).unwrap(), b"tui");
    buffer
}

#[test]
fn keys_change_the_draft_and_save_reaches_disk() {
    let rig = Rig::new("keys");
    rig.file("note", b"first\n");
    let mut tui = Tui::open(rig.client(), "note").unwrap();
    let mut other = rig.client();
    let buffer = buffer_of(&mut other, &tui);

    assert_eq!(tui.key(Key::Tab), Ok(Step::Same));
    assert_eq!(tui.key(Key::Char('i')), Ok(Step::Changed), "the mode");
    assert_eq!(typed(&mut tui, "xy<BS>"), Ok(Step::Changed));
    assert_eq!(other.read_all(buffer).unwrap(), b"xfirst\n");
    assert!(tui.frame(80, 24).status.contains("r2[+]   -- INSERT --"));
    assert_eq!(tui.key(Key::End), Ok(Step::Changed), "ends the insert");
    assert_eq!(tui.key(Key::Char('!')), Ok(Step::Changed));
    assert_eq!(other.read_all(buffer).unwrap(), b"xfirst!\n");
    assert_eq!(rig.disk("note"), b"first\n", "disk waits for save");
    let frame = tui.frame(80, 24);
    assert_eq!(frame.lines, vec!["xfirst!"]);
    assert_eq!(frame.cursor, (0, 7));
    assert!(frame.status.starts_with("note  1:8  r3[+]"), "{}", frame.status);

    assert_eq!(tui.tick(), Ok(Step::Same), "its own edits change nothing");
    assert_eq!(tui.key(Key::Esc), Ok(Step::Changed));
    assert_eq!(typed(&mut tui, ":w<CR>"), Ok(Step::Changed));
    assert_eq!(rig.disk("note"), b"xfirst!\n");
    assert_eq!(tui.frame(80, 24).status, "note  1:7  r3   ");
    assert_eq!(typed(&mut tui, ":q<CR>"), Ok(Step::Quit(0)));
}

#[test]
fn the_quit_commands_map_onto_the_three_outcomes() {
    let rig = Rig::new("quit");
    rig.file("note", b"first\n");
    let mut tui = Tui::open(rig.client(), "note").unwrap();
    assert_eq!(typed(&mut tui, ":q<CR>"), Ok(Step::Quit(0)), "nothing unsaved");

    assert_eq!(typed(&mut tui, "ix<Esc>:q<CR>"), Ok(Step::Changed), "E37");
    let status = tui.frame(80, 24).status;
    assert!(status.contains("E37: No write since last change"), "{status}");
    assert_eq!(tui.key(Key::Char('j')), Ok(Step::Changed), "the message goes");
    assert!(!tui.frame(80, 24).status.contains("E37"));
    assert_eq!(typed(&mut tui, ":q!<CR>"), Ok(Step::Quit(0)), "discarded");
    assert_eq!(rig.disk("note"), b"first\n");

    assert_eq!(typed(&mut tui, "iy<Esc>:wq<CR>"), Ok(Step::Quit(0)));
    assert_eq!(rig.disk("note"), b"yxfirst\n");
    assert_eq!(typed(&mut tui, ":xyz<CR>"), Ok(Step::Changed));
    let status = tui.frame(80, 24).status;
    assert!(status.contains("E492: Not an editor command: xyz"), "{status}");
    assert_eq!(typed(&mut tui, ":cq<CR>"), Ok(Step::Quit(1)));
    assert_eq!(typed(&mut tui, "iz<Esc>ZZ"), Ok(Step::Quit(0)));
    assert_eq!(rig.disk("note"), b"zyxfirst\n");
    assert_eq!(typed(&mut tui, "ZQ"), Ok(Step::Quit(0)));
}

#[test]
fn undo_and_redo_walk_the_revisions() {
    let rig = Rig::new("undo");
    rig.file("note", b"first\n");
    let mut tui = Tui::open(rig.client(), "note").unwrap();
    let mut other = rig.client();
    let buffer = buffer_of(&mut other, &tui);

    assert_eq!(typed(&mut tui, "ix<Esc>"), Ok(Step::Changed));
    assert_eq!(typed(&mut tui, ":w<CR>"), Ok(Step::Changed));
    assert_eq!(typed(&mut tui, "iy<Esc>"), Ok(Step::Changed));
    assert!(tui.frame(80, 24).status.contains("r3[+]"));
    assert_eq!(tui.key(Key::Char('u')), Ok(Step::Changed));
    let frame = tui.frame(80, 24);
    assert_eq!(frame.lines, vec!["xfirst"]);
    assert!(frame.status.contains("1:1  r2   "), "{}", frame.status);
    assert_eq!(tui.tick(), Ok(Step::Same));
    assert_eq!(tui.key(Key::Ctrl('r')), Ok(Step::Changed));
    assert_eq!(tui.frame(80, 24).lines, vec!["yxfirst"]);
    assert_eq!(tui.key(Key::Char('u')), Ok(Step::Changed));
    assert_eq!(tui.key(Key::Char('u')), Ok(Step::Changed));
    assert_eq!(tui.key(Key::Char('u')), Ok(Step::Changed), "a message");
    assert!(tui.frame(80, 24).status.contains("Already at oldest change"));

    other.call_by_name(buffer, "redo", Vec::new()).unwrap();
    assert_eq!(tui.tick(), Ok(Step::Changed));
    let frame = tui.frame(80, 24);
    assert_eq!(frame.lines, vec!["xfirst"]);
    assert!(frame.status.contains("r2   "), "{}", frame.status);
    other.call_by_name(buffer, "redo", Vec::new()).unwrap();
    other.call_by_name(buffer, "save", Vec::new()).unwrap();
    assert_eq!(tui.tick(), Ok(Step::Changed), "a save by another shows");
    assert!(tui.frame(80, 24).status.contains("r3   "));
}

#[test]
fn a_write_from_another_client_shows_after_tick() {
    let rig = Rig::new("modify");
    rig.file("note", b"first\n");
    let mut tui = Tui::open(rig.client(), "note").unwrap();
    let mut other = rig.client();
    let buffer = buffer_of(&mut other, &tui);

    assert_eq!(tui.tick(), Ok(Step::Same));
    other.write_all(buffer, b"other\nlines".to_vec()).unwrap();
    assert_eq!(tui.tick(), Ok(Step::Changed));
    assert_eq!(tui.tick(), Ok(Step::Same));
    let frame = tui.frame(80, 24);
    assert_eq!(frame.lines, vec!["other", "lines"]);
    assert!(frame.status.contains("[+]"), "{}", frame.status);
    assert_eq!(tui.key(Key::Down), Ok(Step::Changed));
    assert_eq!(tui.frame(80, 24).cursor, (1, 0));
}

#[test]
fn a_missing_file_opens_empty_and_appears_on_save() {
    let rig = Rig::new("missing");
    let mut tui = Tui::open(rig.client(), "none").unwrap();
    assert_eq!(tui.frame(80, 24).lines, vec![""]);
    assert_eq!(typed(&mut tui, "ix<Esc>"), Ok(Step::Changed));
    assert!(!rig.dir().join("none").exists(), "nothing on disk yet");
    assert_eq!(typed(&mut tui, ":w<CR>"), Ok(Step::Changed));
    assert_eq!(rig.disk("none"), b"x\n", "the newline Vim adds");
}

#[test]
fn a_tui_behind_a_full_ring_reads_everything_again() {
    let rig =
        Rig::with("overflow", Limits { watch_ring: 2, ..Limits::default() });
    rig.file("note", b"first\n");
    let mut tui = Tui::open(rig.client(), "note").unwrap();
    let mut other = rig.client();
    let buffer = buffer_of(&mut other, &tui);
    for text in [b"one".as_slice(), b"two", b"three"] {
        other.write_all(buffer, text.to_vec()).unwrap();
    }
    assert_eq!(tui.tick(), Ok(Step::Changed));
    assert_eq!(tui.frame(80, 24).lines, vec!["three"]);
    assert!(tui.frame(80, 24).status.contains("r4[+]"));
}
