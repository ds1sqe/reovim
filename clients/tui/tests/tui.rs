//! The tui on a Kernel with fs and text, beside a second Client, over
//! a stub Plugin.
//!
//! The stub hands back the effects a test scripts and records what the
//! tui told it, so each test judges only what the tui owns: the Buffer
//! and the disk, history, the watch, the quit codes, the notices and
//! the clipboard rd. What a key means is the Plugin's, and the nvim
//! corpus in plugins/vim judges that.

use std::cell::RefCell;
use std::collections::VecDeque;
use std::rc::Rc;

use reovim_client::{
    Client, Edit, Effect, Fault, Frame, Key, Plugin, Quit, Rd,
};
use reovim_client_tui::{Step, Tui};
use reovim_rig::{Limits, Rig};

/// What the stub hands back and what the tui told it.
#[derive(Default)]
struct Script {
    /// The text as the Plugin holds it: what `loaded` gave it, with
    /// the edits it handed back already in it.
    text: String,
    /// One effect per key, in order; `pasted` takes the next one too.
    effects: VecDeque<Effect>,
    /// Every `loaded`, every `notice`, every `pasted`.
    loads: Vec<String>,
    notices: Vec<String>,
    pastes: Vec<Option<String>>,
    /// What a yank left for the clipboard, once.
    clip: Option<String>,
    /// The last word before the next save.
    saving: Option<Edit>,
    position: (usize, usize),
}

impl Script {
    /// The edit the Plugin has already made to its own copy; the tui
    /// sends it on to the Buffer.
    fn apply(&mut self, edit: &Edit) {
        self.text.replace_range(edit.at..edit.at + edit.len, &edit.text);
    }

    /// Those edits, in order.
    fn apply_each(&mut self, edits: &[Edit]) {
        for edit in edits {
            self.apply(edit);
        }
    }

    /// The next scripted effect, its edits already in the text.
    fn next(&mut self) -> Effect {
        let effect = self.effects.pop_front().unwrap_or(Effect::Nothing);
        match &effect {
            Effect::Edit(edit) => self.apply(edit),
            Effect::Edits(edits) => self.apply_each(edits),
            _ => {}
        }
        effect
    }
}

/// A Plugin of scripted effects. The handle is shared, so a test
/// writes the script and reads what was recorded while the tui holds
/// the Plugin itself.
#[derive(Clone, Default)]
struct Stub(Rc<RefCell<Script>>);

impl Stub {
    /// The effect for the next key, or the next `pasted`.
    fn give(&self, effect: Effect) {
        self.0.borrow_mut().effects.push_back(effect);
    }

    /// What `take_clipboard` hands over on the next key.
    fn yanks(&self, text: &str) {
        self.0.borrow_mut().clip = Some(String::from(text));
    }

    /// What `saving` asks for before the next save, as Vim's
    /// fixendofline asks for the newline a text ends without.
    fn on_save(&self, at: usize, text: &str) {
        self.0.borrow_mut().saving = Some(one(at, 0, text, false));
    }

    fn loads(&self) -> Vec<String> {
        self.0.borrow().loads.clone()
    }

    fn notices(&self) -> Vec<String> {
        self.0.borrow().notices.clone()
    }

    fn pastes(&self) -> Vec<Option<String>> {
        self.0.borrow().pastes.clone()
    }
}

impl Plugin for Stub {
    fn loaded(&mut self, bytes: &[u8]) -> bool {
        let text = String::from_utf8_lossy(bytes).into_owned();
        let script = &mut *self.0.borrow_mut();
        script.loads.push(text.clone());
        let changed = script.text != text;
        script.text = text;
        changed
    }

    fn key(&mut self, _key: Key) -> Effect {
        self.0.borrow_mut().next()
    }

    fn pasted(&mut self, text: Option<String>) -> Effect {
        let script = &mut *self.0.borrow_mut();
        script.pastes.push(text);
        script.next()
    }

    fn take_clipboard(&mut self) -> Option<String> {
        self.0.borrow_mut().clip.take()
    }

    fn saving(&mut self) -> Option<Edit> {
        let script = &mut *self.0.borrow_mut();
        let edit = script.saving.take()?;
        script.apply(&edit);
        Some(edit)
    }

    fn goto_line(&mut self, n: usize) {
        self.0.borrow_mut().position = (n.saturating_sub(1), 0);
    }

    fn position(&self) -> (usize, usize) {
        self.0.borrow().position
    }

    fn notice(&mut self, message: &str) {
        self.0.borrow_mut().notices.push(String::from(message));
    }

    /// The text in lines, a trailing newline ending the last one; the
    /// status row is the tui's alone.
    fn frame(&mut self, _cols: u16, _rows: u16) -> Frame {
        let script = self.0.borrow();
        let text = script.text.strip_suffix('\n').unwrap_or(&script.text);
        let cell = |n: usize| u16::try_from(n).unwrap_or(u16::MAX);
        Frame {
            lines: text.split('\n').map(String::from).collect(),
            cursor: (cell(script.position.0), cell(script.position.1)),
            ..Frame::default()
        }
    }
}

/// `text` in place of `len` bytes at `at`, `join` saying it belongs to
/// the head revision.
fn one(at: usize, len: usize, text: &str, join: bool) -> Edit {
    Edit { at, len, text: String::from(text), join }
}

/// That edit alone, as a new revision.
fn edit(at: usize, len: usize, text: &str) -> Effect {
    Effect::Edit(one(at, len, text, false))
}

/// That edit alone, into the head revision, as an insert joins.
fn amend(at: usize, len: usize, text: &str) -> Effect {
    Effect::Edit(one(at, len, text, true))
}

/// One key, `effect` what the stub hands back for it. The key itself
/// is the stub's to ignore: what a key means is the Plugin's.
fn press(tui: &mut Tui<Stub>, effect: Effect) -> Result<Step, Fault> {
    tui.plugin().give(effect);
    tui.key(Key::Char('.'))
}

fn buffer_of(client: &mut Client, tui: &Tui<Stub>) -> Rd {
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
    let mut tui =
        Tui::open(rig.client(), "", Some("note"), Stub::default()).unwrap();
    let mut other = rig.client();
    let buffer = buffer_of(&mut other, &tui);
    assert_eq!(tui.plugin().loads(), ["first\n"], "the draft at open");

    assert_eq!(press(&mut tui, Effect::Nothing), Ok(Step::Same));
    assert_eq!(press(&mut tui, Effect::Moved), Ok(Step::Changed), "a move");
    assert_eq!(press(&mut tui, edit(0, 0, "x")), Ok(Step::Changed));
    assert_eq!(press(&mut tui, amend(1, 0, "y")), Ok(Step::Changed));
    assert_eq!(press(&mut tui, amend(1, 1, "")), Ok(Step::Changed));
    assert_eq!(other.read_all(buffer).unwrap(), b"xfirst\n");
    let status = tui.frame(80, 24).status;
    assert!(status.starts_with("note  1:1  r2[+]"), "{status}");
    assert_eq!(press(&mut tui, edit(6, 0, "!")), Ok(Step::Changed));
    assert_eq!(other.read_all(buffer).unwrap(), b"xfirst!\n");
    assert_eq!(rig.disk("note"), b"first\n", "disk waits for save");
    let frame = tui.frame(80, 24);
    assert_eq!(frame.lines, vec!["xfirst!"]);
    assert!(frame.status.starts_with("note  1:1  r3[+]"), "{}", frame.status);

    assert_eq!(tui.tick(), Ok(Step::Same), "its own edits change nothing");
    assert_eq!(press(&mut tui, Effect::Save), Ok(Step::Changed));
    assert_eq!(rig.disk("note"), b"xfirst!\n");
    assert_eq!(tui.frame(80, 24).status, "note  1:1  r3   ");
    assert_eq!(press(&mut tui, Effect::Quit(Quit::Ask)), Ok(Step::Quit(0)));
}

#[test]
fn the_quit_commands_map_onto_the_three_outcomes() {
    let rig = Rig::new("quit");
    rig.file("note", b"first\n");
    let mut tui =
        Tui::open(rig.client(), "", Some("note"), Stub::default()).unwrap();
    let ask = Effect::Quit(Quit::Ask);
    assert_eq!(press(&mut tui, ask.clone()), Ok(Step::Quit(0)), "none unsaved");

    assert_eq!(press(&mut tui, edit(0, 0, "x")), Ok(Step::Changed));
    assert_eq!(press(&mut tui, ask), Ok(Step::Changed), "E37");
    let e37 = "E37: No write since last change (add ! to override)";
    assert_eq!(tui.plugin().notices(), [e37], "the tui's own word");
    let force = Effect::Quit(Quit::Force);
    assert_eq!(press(&mut tui, force.clone()), Ok(Step::Quit(0)), "discarded");
    assert_eq!(rig.disk("note"), b"first\n");

    let write = Effect::Quit(Quit::Write);
    assert_eq!(press(&mut tui, edit(0, 0, "y")), Ok(Step::Changed));
    assert_eq!(press(&mut tui, write.clone()), Ok(Step::Quit(0)), "written");
    assert_eq!(rig.disk("note"), b"yxfirst\n");
    let abort = Effect::Quit(Quit::Abort);
    assert_eq!(press(&mut tui, abort), Ok(Step::Quit(1)), "use nothing");
    assert_eq!(press(&mut tui, edit(0, 0, "z")), Ok(Step::Changed));
    assert_eq!(press(&mut tui, write), Ok(Step::Quit(0)));
    assert_eq!(rig.disk("note"), b"zyxfirst\n");
    assert_eq!(press(&mut tui, force), Ok(Step::Quit(0)));
    assert_eq!(tui.plugin().notices(), [e37], "no other word since");
}

#[test]
fn undo_and_redo_walk_the_revisions() {
    let rig = Rig::new("undo");
    rig.file("note", b"first\n");
    let mut tui =
        Tui::open(rig.client(), "", Some("note"), Stub::default()).unwrap();
    let mut other = rig.client();
    let buffer = buffer_of(&mut other, &tui);

    assert_eq!(press(&mut tui, edit(0, 0, "x")), Ok(Step::Changed));
    assert_eq!(press(&mut tui, Effect::Save), Ok(Step::Changed));
    assert_eq!(press(&mut tui, edit(0, 0, "y")), Ok(Step::Changed));
    assert!(tui.frame(80, 24).status.contains("r3[+]"));
    assert_eq!(press(&mut tui, Effect::Undo), Ok(Step::Changed));
    let frame = tui.frame(80, 24);
    assert_eq!(frame.lines, vec!["xfirst"], "the earlier text, loaded again");
    assert!(frame.status.contains("1:1  r2   "), "{}", frame.status);
    assert_eq!(tui.tick(), Ok(Step::Same));
    assert_eq!(press(&mut tui, Effect::Redo), Ok(Step::Changed));
    assert_eq!(tui.frame(80, 24).lines, vec!["yxfirst"]);
    assert_eq!(press(&mut tui, Effect::Undo), Ok(Step::Changed));
    assert_eq!(press(&mut tui, Effect::Undo), Ok(Step::Changed));
    assert_eq!(tui.plugin().loads().last().unwrap(), "first\n", "the oldest");
    assert_eq!(press(&mut tui, Effect::Undo), Ok(Step::Changed), "a message");
    assert_eq!(tui.plugin().notices(), ["Already at oldest change"]);

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
    let mut tui =
        Tui::open(rig.client(), "", Some("note"), Stub::default()).unwrap();
    let mut other = rig.client();
    let buffer = buffer_of(&mut other, &tui);

    assert_eq!(tui.tick(), Ok(Step::Same));
    other.write_all(buffer, b"other\nlines".to_vec()).unwrap();
    assert_eq!(tui.tick(), Ok(Step::Changed));
    assert_eq!(tui.tick(), Ok(Step::Same));
    assert_eq!(tui.plugin().loads(), ["first\n", "other\nlines"]);
    let frame = tui.frame(80, 24);
    assert_eq!(frame.lines, vec!["other", "lines"]);
    assert!(frame.status.contains("[+]"), "{}", frame.status);
}

#[test]
fn a_missing_file_opens_empty_and_appears_on_save() {
    let rig = Rig::new("missing");
    let mut tui =
        Tui::open(rig.client(), "", Some("none"), Stub::default()).unwrap();
    assert_eq!(tui.frame(80, 24).lines, vec![""]);
    assert_eq!(press(&mut tui, edit(0, 0, "x")), Ok(Step::Changed));
    assert!(!rig.dir().join("none").exists(), "nothing on disk yet");
    tui.plugin().on_save(1, "\n");
    assert_eq!(press(&mut tui, Effect::Save), Ok(Step::Changed));
    assert_eq!(rig.disk("none"), b"x\n", "the Plugin's last word in it");
}

#[test]
fn a_tui_behind_a_full_ring_reads_everything_again() {
    let rig =
        Rig::with("overflow", Limits { watch_ring: 2, ..Limits::default() });
    rig.file("note", b"first\n");
    let mut tui =
        Tui::open(rig.client(), "", Some("note"), Stub::default()).unwrap();
    let mut other = rig.client();
    let buffer = buffer_of(&mut other, &tui);
    for text in [b"one".as_slice(), b"two", b"three"] {
        other.write_all(buffer, text.to_vec()).unwrap();
    }
    assert_eq!(tui.tick(), Ok(Step::Changed));
    assert_eq!(tui.frame(80, 24).lines, vec!["three"]);
    assert!(tui.frame(80, 24).status.contains("r4[+]"));
}

#[test]
fn many_edits_are_one_revision() {
    let rig = Rig::new("many");
    rig.file("note", b"x\nx\nx\n");
    let mut tui =
        Tui::open(rig.client(), "", Some("note"), Stub::default()).unwrap();
    // A substitute over the three lines: the first edit a revision, the
    // rest joined into it.
    let substitute = Effect::Edits(vec![
        one(0, 1, "y", false),
        one(2, 1, "y", true),
        one(4, 1, "y", true),
    ]);
    assert_eq!(press(&mut tui, substitute), Ok(Step::Changed));
    let frame = tui.frame(80, 24);
    assert_eq!(frame.lines, vec!["y", "y", "y"]);
    assert!(frame.status.contains("r2[+]"), "{}", frame.status);
    assert_eq!(press(&mut tui, Effect::Undo), Ok(Step::Changed));
    let frame = tui.frame(80, 24);
    assert_eq!(frame.lines, vec!["x", "x", "x"]);
    assert!(frame.status.contains("1:1  r1   "), "{}", frame.status);

    let block = Effect::Edits(vec![
        one(0, 1, "z", false),
        one(2, 1, "z", true),
        one(4, 1, "z", true),
    ]);
    assert_eq!(press(&mut tui, block), Ok(Step::Changed));
    assert_eq!(tui.frame(80, 24).lines, vec!["z", "z", "z"]);
    assert_eq!(press(&mut tui, Effect::Undo), Ok(Step::Changed), "one back");
    assert_eq!(tui.frame(80, 24).lines, vec!["x", "x", "x"]);
}

#[test]
fn the_clipboard_is_an_rd_of_its_module() {
    let rig = Rig::new("clip");
    rig.file("note", b"one\n");
    let mut tui =
        Tui::open(rig.client(), "", Some("note"), Stub::default()).unwrap();
    tui.plugin().yanks("one\n");
    assert_eq!(press(&mut tui, Effect::Moved), Ok(Step::Changed));
    assert_eq!(rig.clipboard(), b"one\n", "written through the rd");

    rig.clip(b"two\n");
    tui.plugin().give(Effect::Clipboard);
    tui.plugin().give(edit(4, 0, "two\n"));
    assert_eq!(tui.key(Key::Char('p')), Ok(Step::Changed));
    assert_eq!(tui.plugin().pastes(), [Some(String::from("two\n"))]);
    assert_eq!(tui.frame(80, 24).lines, vec!["one", "two"]);

    tui.plugin().yanks("two");
    assert_eq!(press(&mut tui, Effect::Moved), Ok(Step::Changed));
    assert_eq!(rig.clipboard(), b"two");
    tui.goto_line(2);
    let frame = tui.frame(80, 24);
    assert_eq!(frame.cursor, (1, 0));
    assert!(frame.status.starts_with("note  2:1  "), "{}", frame.status);
}

#[test]
fn a_buffer_over_nothing_has_no_file_to_write() {
    let rig = Rig::new("unnamed");
    let mut tui = Tui::open(rig.client(), "", None, Stub::default()).unwrap();
    assert_eq!(tui.frame(80, 24).status, "[No Name]  1:1  r1   ");
    let write = Effect::Quit(Quit::Write);
    assert_eq!(press(&mut tui, write.clone()), Ok(Step::Quit(0)), "no write");
    assert_eq!(press(&mut tui, edit(0, 0, "hi")), Ok(Step::Changed));
    assert_eq!(press(&mut tui, Effect::Save), Ok(Step::Changed));
    assert_eq!(press(&mut tui, write), Ok(Step::Changed), "E32 again");
    assert_eq!(press(&mut tui, Effect::Quit(Quit::Ask)), Ok(Step::Changed));
    assert_eq!(tui.tick(), Ok(Step::Same));
    let e32 = "E32: No file name";
    let e37 = "E37: No write since last change (add ! to override)";
    assert_eq!(tui.plugin().notices(), [e32, e32, e37]);
    assert_eq!(press(&mut tui, Effect::Quit(Quit::Force)), Ok(Step::Quit(0)));
    let written = std::fs::read_dir(rig.dir()).unwrap().count();
    assert_eq!(written, 0, "nothing written");
}

#[test]
fn w_name_writes_a_copy_and_names_a_buffer_over_nothing() {
    let rig = Rig::new("saveas");
    rig.file("note", b"one\n");
    let mut tui =
        Tui::open(rig.client(), "", Some("note"), Stub::default()).unwrap();
    assert_eq!(press(&mut tui, edit(0, 0, "x")), Ok(Step::Changed));
    let copy = Effect::SaveAs(String::from("copy"));
    assert_eq!(press(&mut tui, copy), Ok(Step::Changed));
    assert_eq!(rig.disk("copy"), b"xone\n");
    assert_eq!(rig.disk("note"), b"one\n", "the file itself waits");
    let status = tui.frame(80, 24).status;
    assert!(status.starts_with("note  1:1  r2[+]"), "{status}");
    let own = Effect::SaveAs(String::from("note"));
    assert_eq!(press(&mut tui, own), Ok(Step::Changed), "its own name saves");
    assert_eq!(rig.disk("note"), b"xone\n");
    assert!(tui.frame(80, 24).status.contains("r2   "));

    let mut tui = Tui::open(rig.client(), "", None, Stub::default()).unwrap();
    assert_eq!(press(&mut tui, edit(0, 0, "hi")), Ok(Step::Changed));
    tui.plugin().on_save(2, "\n");
    let fresh = Effect::SaveAs(String::from("fresh"));
    assert_eq!(press(&mut tui, fresh), Ok(Step::Changed));
    assert_eq!(rig.disk("fresh"), b"hi\n");
    let status = tui.frame(80, 24).status;
    assert!(status.starts_with("fresh  1:1  r2   "), "{status}");
    // Before the newline the last save left: the text ends in one, so
    // `saving` has nothing to add this time.
    assert_eq!(press(&mut tui, edit(2, 0, "x")), Ok(Step::Changed));
    assert_eq!(press(&mut tui, Effect::Quit(Quit::Write)), Ok(Step::Quit(0)));
    assert_eq!(rig.disk("fresh"), b"hix\n");
}
