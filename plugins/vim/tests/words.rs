//! The `:` line over the tui and the rig: Vim's own list of words,
//! every one of them by its full name and by the beginning the order
//! computes for it, and a line that is none of them, which goes to the
//! shell.
//!
//! The corpus judges the short forms it uses and nothing else, so
//! what the full names and the shell's lines do is judged here, by
//! what happened: the step the editor ended in, the status row, the
//! text, the file on disk, the search marks, and what a listing was
//! started over.

use std::cell::RefCell;
use std::rc::Rc;

use reovim_client::{
    BLOCK, Ctx, Event, Fault, Hash, Plugin, Rd, Rights, Start, grid,
};
use reovim_client_tui::{Step, Table, Tui, Want};
use reovim_plugin_vim::notation::keys;
use reovim_plugin_vim::{Vim, WORDS, Word, resolve};
use reovim_rig::Rig;
use reovim_text_api::open as text_open;

/// The one editor a test drives, over whichever Plugin the table gives.
type Editor = Tui<Box<dyn Plugin>>;

/// What each start of the table's Container Plugin was over, in order.
type Listed = Rc<RefCell<Vec<Option<Rd>>>>;

/// The Container Plugin of the table, as explore.rs has it: it records
/// what it was started over and shows a scratch Buffer of its own, so
/// the region has text and a word no Vim would give it.
struct Stub(Listed);

impl Plugin for Stub {
    fn start(&mut self, ctx: Ctx<'_>, at: Start) -> Result<(), Fault> {
        let Ctx { tree, caps } = ctx;
        self.0.borrow_mut().push(at.over);
        let opener = tree.resolve("/module/text/open")?;
        let opening = tree.open(opener, Rights::WRITE)?;
        let buffer = text_open::Call(tree, opening).new()?;
        tree.close(opening)?;
        let Some(region) = caps.open(grid::hash()) else { return Ok(()) };
        let view = grid::View {
            over: buffer.0,
            top: 0,
            cursor: 0,
            shape: BLOCK,
            spans: Vec::new(),
            status: String::from("-- LISTING --"),
        };
        grid::Call(caps, region).show(view)
    }

    fn input(&mut self, _: Ctx<'_>, _: Hash, _: &[u8]) -> Result<(), Fault> {
        Ok(())
    }

    fn changed(&mut self, _: Ctx<'_>, _: &[Event]) -> Result<(), Fault> {
        Ok(())
    }
}

/// The app's table in the small: Vim for a cell, the stub for a
/// Container.
fn table(seen: &Listed) -> Table<Box<dyn Plugin>> {
    let seen = seen.clone();
    Box::new(move |want| match want {
        Want::Cell => Some(Box::new(Vim::default()) as Box<dyn Plugin>),
        Want::Container => Some(Box::new(Stub(seen.clone()))),
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

/// Every key of `text` in Vim notation; what the last one did.
fn typed(tui: &mut Editor, text: &str) -> Step {
    let mut step = Step::Same;
    for key in keys(text).unwrap() {
        step = tui.key(key).unwrap();
    }
    step
}

/// What `text` did to an editor of its own, in one line: the step it
/// ended in, the status row, the text shown, what `note` holds on
/// disk, how many search marks are on the frame, and how many
/// listings were started.
fn done(name: &str, keys: &str) -> String {
    let rig = work(name);
    let seen = Listed::default();
    let mut tui =
        Tui::open(rig.client(), "d", Some("d/note"), None, table(&seen))
            .unwrap();
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
    format!(
        "{step:?} [{status}] {:?} disk={disk:?} marks={} listed={}",
        frame.lines,
        frame.marks.len(),
        seen.borrow().len()
    )
}

/// One word of Vim's: a line that uses it by its full name, the same
/// line by its abbreviation, and what both do.
struct Case {
    full: &'static str,
    short: &'static str,
    done: &'static str,
}

#[test]
fn every_word_runs_by_its_full_name_and_by_its_abbreviation() {
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

/// The shortest beginning of `word` that resolves to it: its
/// abbreviation, which the order of the list computes and nothing
/// lists by hand.
fn short(word: &Word) -> &'static str {
    let name = word.name;
    let resolves = |n: &usize| resolve(&name[..*n]).is_some_and(|w| w == word);
    &name[..(1..=name.len()).find(resolves).unwrap_or(name.len())]
}

#[test]
fn the_order_computes_the_short_form_of_every_word() {
    let short: Vec<&str> = WORDS.iter().map(short).collect();
    // `cquit` is `c` and not nvim's `cq`: no `change` stands before it
    // in this list, and no name under PATH stands behind it. `cli`
    // stands behind `cquit`, so it is `cl`.
    let all = ["w", "wq", "q", "x", "c", "s", "se", "n", "e", "ex", "cl"];
    assert_eq!(short, all);
    // Nothing under PATH is reached by a beginning of its name: `wa`
    // is no word of Vim's and the line goes to the shell whole, where
    // `watch` is `watch`.
    assert_eq!(resolve("wa"), None);
    assert_eq!(resolve("l"), None);
}

#[test]
fn a_line_of_the_shells_runs_where_the_buffer_is() {
    let rig = work("words-cli");
    let seen = Listed::default();
    let mut tui =
        Tui::open(rig.client(), "d", Some("d/note"), None, table(&seen))
            .unwrap();
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
    let seen = Listed::default();
    let mut tui =
        Tui::open(rig.client(), "d", Some("d/note"), None, table(&seen))
            .unwrap();
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
fn a_file_that_is_not_there_is_e345_and_the_region_stays() {
    let rig = work("words-e345");
    let seen = Listed::default();
    let mut tui =
        Tui::open(rig.client(), "d", Some("d/note"), None, table(&seen))
            .unwrap();
    assert_eq!(typed(&mut tui, ":edit nothere<CR>"), Step::Changed);
    let frame = tui.frame(80, 24).unwrap();
    let said = "E345: Can't find file \"nothere\" in path";
    assert_eq!(frame.status, format!("note  1:1  r1   {said}"));
    assert_eq!(frame.lines, ["one", "two"], "the file is still it");
}

#[test]
fn a_line_that_is_only_a_range_is_that_line() {
    let rig = work("words-range");
    let seen = Listed::default();
    let mut tui =
        Tui::open(rig.client(), "d", Some("d/note"), None, table(&seen))
            .unwrap();
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
