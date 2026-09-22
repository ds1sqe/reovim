//! The outcomes the engine declares and the replies that answer them.
//!
//! One numbering for every outcome that waits, a search's among them;
//! one door for every reply; every `Effect` the same thing as the
//! `Outcome` record it makes; and no handle and no path of the host's
//! left on the engine.

mod common;

use std::rc::Rc;

use common::{on, one, typed};
use reovim_client::Host;
use reovim_client::dev::Clip;
use reovim_client_api::resolver::{
    Command, Edit, Entry, Fed, Goto, Open, Outcome, Place, Reply, Search,
};
use reovim_client_api::{NONE, filled, kind, land, reply};
use reovim_client_tui::Tui;
use reovim_kernel_interface::Error;
use reovim_plugin_vim::{Effect, Quit, Switch, Vim, effect};
use reovim_plugin_vim_host::Editor;
use reovim_rig::{Rig, lines, opened, said};

/// A reply at rest: the `kind` it is, nothing filled, nothing
/// refused. Each test fills the one field its own answer carries.
const fn answer(continuation: u64, kind: u64) -> Reply {
    Reply {
        continuation,
        kind,
        text: String::new(),
        place: Place { cursor: 0, top: 0 },
        entries: Vec::new(),
        filled: filled::ABSENT,
        error: 0,
    }
}

/// The clipboard's own answer: what it held, for the put that waits.
fn pasted(continuation: u64, text: &str) -> Reply {
    Reply { text: String::from(text), ..answer(continuation, reply::PASTED) }
}

/// One read of a search's `matches`: the entries it found and how far
/// the fill got.
fn matched(continuation: u64, found: &[(u64, u64)], filled: u64) -> Reply {
    let entries = found.iter().map(|&(at, len)| Entry { at, len }).collect();
    Reply { entries, filled, ..answer(continuation, reply::MATCHED) }
}

/// What the host met, for the outcome it could not perform: `asked`
/// is the operation as the row names it.
fn refused(continuation: u64, asked: &str, error: u64) -> Reply {
    Reply {
        text: String::from(asked),
        error,
        ..answer(continuation, reply::REFUSED)
    }
}

/// The number the clipboard outcome of `keys` waits on.
fn wanting(vim: &mut Vim, keys: &str) -> u64 {
    match typed(vim, keys) {
        Effect::Clipboard(continuation) => continuation,
        other => panic!("a put waits for the clipboard, not {other:?}"),
    }
}

/// A put that waits is answered by its own number alone: another
/// number is an answer to something that is no longer waited for, and
/// the put stands until the number it asked with comes back.
#[test]
fn a_clipboard_put_waits_on_its_continuation_and_a_stale_reply_is_nothing() {
    let mut vim = on("abc\n");
    let continuation = wanting(&mut vim, "\"+p");
    assert_ne!(continuation, 0, "an outcome that waits is numbered");

    let stale = vim.reply(&pasted(continuation + 9, "X"));
    assert_eq!(stale, Effect::Nothing, "no wait of that number");
    assert_eq!(vim.text(), "abc\n", "and nothing was put");

    let edits = vec![Edit { at: 1, len: 0, text: "X".into(), join: false }];
    assert_eq!(vim.reply(&pasted(continuation, "X")), Effect::Edits(edits));
    assert_eq!(vim.text(), "aXbc\n");

    // The wait is over: the number that answered it answers nothing
    // a second time.
    assert_eq!(vim.reply(&pasted(continuation, "Y")), Effect::Nothing);
    assert_eq!(vim.text(), "aXbc\n");
}

/// The operation the host could not perform is the engine's to word:
/// it says what was refused, rings — which ends a replay — and lets
/// go of the prefix it was holding.
#[test]
fn a_refused_reply_is_the_engines_word() {
    let mut vim = on("abc\n");
    vim.bind("dx", "nohlsearch");
    assert_eq!(typed(&mut vim, "d"), Effect::Nothing, "a prefix is held");

    let said = vim.reply(&refused(0, "edit", error_no_space()));
    assert_eq!(said, Effect::Nothing);
    assert_eq!(vim.status(), "E: edit refused: no space");
    assert!(vim.rung(), "a refusal rings, so a replay ends here");

    // The cause is the engine's word too: the one a Buffer answers
    // an editor with is named, and any other is not guessed at.
    let other = reovim_kernel_interface::Error::Busy as u64;
    assert_eq!(vim.reply(&refused(0, "write", other)), Effect::Nothing);
    assert_eq!(vim.status(), "E: write refused");

    // Nothing of the prefix is held any more: the next key is itself.
    assert!(vim.released().is_empty(), "the held keys are dropped");
    assert_eq!(
        typed(&mut vim, "x"),
        Effect::Edits(vec![Edit {
            at: 0,
            len: 1,
            text: String::new(),
            join: false,
        }])
    );
}

/// `Error::NoSpace` as the Kernel numbers it, which a `REFUSED` reply
/// carries.
const fn error_no_space() -> u64 {
    reovim_kernel_interface::Error::NoSpace as u64
}

/// A search waits on the same numbering every other outcome waits on,
/// and answers to a number that is not its own move nothing.
#[test]
fn a_search_reply_carries_the_wait_it_answers() {
    let mut vim = on("ab\ncd\n");
    let Effect::Search(want) = typed(&mut vim, "/cd<CR>") else {
        panic!("the key asks for the list it cannot answer from")
    };
    assert_ne!(want.epoch, 0, "the wait is numbered");
    assert_eq!(vim.position(), (0, 0), "nothing moved while it waits");

    let stale = matched(want.epoch + 9, &[(0, 2)], filled::WHOLE);
    assert_eq!(vim.reply(&stale), Effect::Nothing, "a list that is gone");
    assert_eq!(vim.position(), (0, 0));

    let its_own = matched(want.epoch, &[(3, 2)], filled::WHOLE);
    assert_eq!(vim.reply(&its_own), Effect::Moved);
    assert_eq!(vim.position(), (1, 0));

    // One counting, not two: the next outcome that waits takes the
    // number after the search's, whatever kind of wait it is.
    let continuation = wanting(&mut vim, "\"+p");
    assert!(continuation > want.epoch, "one counter for every wait");
}

/// Every `Effect` the engine makes is the `Outcome` record of its own
/// kind, and that record is the same `Effect` read back: what crosses
/// the boundary is declared data and loses nothing.
#[test]
fn every_effect_round_trips_through_outcome() {
    let edit = Edit { at: 3, len: 1, text: "x".into(), join: true };
    let command = Command { line: "write --address 1,9".into() };
    let open = Open { path: "note".into(), bang: true };
    let goto = Goto { over: NONE, line: 4, column: 2, land: land::EXACT };
    let search = Search {
        pattern: "wo.d".into(),
        ignorecase: true,
        smartcase: false,
        from: 30,
        epoch: 4,
    };
    let fed = Fed { keys: vec![u64::from('x')], repeat: true };
    let every = [
        (Effect::Nothing, kind::NOTHING),
        (Effect::Moved, kind::MOVED),
        (Effect::Edits(vec![edit]), kind::EDITS),
        (Effect::Undo, kind::UNDO),
        (Effect::Redo, kind::REDO),
        (Effect::Save(true), kind::SAVE),
        (Effect::SaveAs("note".into(), true), kind::SAVE_AS),
        (Effect::Quit(Quit::All(true)), kind::QUIT),
        (Effect::Switch(Switch::Named("2".into())), kind::SWITCH),
        (Effect::Close("2".into(), true), kind::CLOSE),
        (Effect::List, kind::LIST),
        (Effect::SaveAll, kind::SAVE_ALL),
        (Effect::Explore(open.clone()), kind::EXPLORE),
        (Effect::Open(open), kind::OPEN),
        (Effect::Reload(true), kind::RELOAD),
        (Effect::Command(command), kind::COMMAND),
        (Effect::Cli(true), kind::CLI),
        (Effect::Complete("w".into()), kind::COMPLETE),
        (Effect::Clipboard(7), kind::CLIPBOARD),
        (Effect::Goto(goto), kind::GOTO),
        (Effect::Search(search), kind::SEARCH),
        (Effect::Fed(fed), kind::FED),
    ];
    assert_eq!(every.len(), 22, "one kind for every effect there is");
    for (made, kind) in every {
        let outcome = made.outcome();
        assert_eq!(outcome.kind, kind, "{made:?}");
        assert_eq!(effect(&outcome), Some(made.clone()), "{made:?}");
    }
    // Every way out of the six journeys, each read back as itself.
    let mut vim = on("abc\n");
    for keys in ["x", "3dw", ":write\r", "\"+p", "u", "<C-r>"] {
        let made = typed(&mut vim, keys);
        assert_eq!(effect(&made.outcome()), Some(made), "{keys}");
    }
}

/// A kind no `Outcome` declares is nothing the engine can perform.
#[test]
fn an_outcome_of_no_kind_is_no_effect() {
    let mut outcome: Outcome = Effect::Nothing.outcome();
    outcome.kind = 22;
    assert_eq!(effect(&outcome), None);
}

/// The engine holds no handle and no path of the host's: the shell,
/// the root, the working directory, the clipboard's rd, the draft's
/// handle, the file a new Buffer was opened for and the greeting the
/// next stay shows all belong to the Client, which is where every
/// reader of them already was.
#[test]
fn the_engine_holds_no_path_or_handle() {
    let shown = format!("{:#?}", Vim::default());
    let gone = ["shell", "root", "cwd", "clip", "dir", "name", "h", "greeting"];
    for field in gone {
        let named = format!("\n    {field}: ");
        assert!(!shown.contains(&named), "`{field}` is still the engine's");
    }
    // The fields the engine keeps are the rds it hands back and keys
    // its own tables by, which is what settles the rest are not.
    for field in ["source", "buffer", "alt", "left", "places", "files"] {
        let named = format!("\n    {field}: ");
        assert!(shown.contains(&named), "`{field}` is the engine's own");
    }
}

/// An editor over `note`, which holds `text`, with `other` beside it
/// for a swap; `clip` is the machine it was given. For what only a
/// stay has.
fn over(
    name: &str,
    text: &[u8],
    clip: Clip,
) -> (Rig, Tui, Rc<Host>, Rc<Editor>) {
    let rig = Rig::new(name);
    rig.file("note", text);
    rig.file("other", b"three\nfour\n");
    let host = rig.host();
    let editor = Editor::new();
    let table = one(Rc::clone(&editor));
    let tui = opened(&host, table, clip, "", Some("note"), None).unwrap();
    (rig, tui, host, editor)
}

/// A machine whose clipboard answers neither text nor "no tool": a
/// read the Client is not allowed to make, which is every failure the
/// host cannot read as an empty one.
fn denied() -> Clip {
    Clip::new(Box::new(|| Err(Error::Denied)), Box::new(|_| Err(Error::Denied)))
}

/// A stay's own first list is numbered like every other wait, so no
/// outcome ever waits on 0: a text no edit has touched — an empty
/// file, whose loading is no change — has a number of its own.
#[test]
fn a_stays_first_list_is_numbered_like_every_other_wait() {
    let (_rig, _tui, _host, editor) =
        over("reply-first-list", b"", Clip::none());
    assert_ne!(editor.engine().epoch(), 0, "the stay took a number");
}

/// A clipboard read that fails is an end of the read like any other,
/// so it answers the put that asked for it. Not "no cell" and not "no
/// tool", which the host already reads as an empty clipboard: a read
/// the machine refused outright.
///
/// The put is what waits, so leaving without an answer would leave it
/// waiting for the rest of the stay — `State.waiting` never back at 0,
/// the frame's sync skipped, and nothing typed afterwards reaching the
/// server. The reply is the one way the wait ends.
#[test]
fn a_refused_clipboard_read_answers_the_put() {
    let (rig, mut tui, _host, editor) =
        over("reply-clip-denied", b"one\ntwo\n", denied());
    reovim_rig::typed(&mut tui, "\"+p");

    assert_eq!(editor.engine().state().waiting, 0, "nothing waits");
    assert_eq!(said(&mut tui), "E353: Nothing in register +");
    assert_eq!(lines(&mut tui), ["one", "two"], "and nothing was put");

    // The stay goes on: the key after it edits, and the edit goes
    // down, which is what the skipped sync would have held back.
    reovim_rig::typed(&mut tui, "x:w<CR>");
    assert_eq!(lines(&mut tui), ["ne", "two"]);
    assert_eq!(rig.disk("note"), b"ne\ntwo\n", "the edit reached the file");
}

/// The numbering is the engine's for its whole life, not a stay's: a
/// swap leaves the counter alone, so the list the second stay opens
/// with is numbered past every wait the first one made. A number that
/// went back would let a reply of the stay before answer a wait of
/// this one.
#[test]
fn a_stay_swap_keeps_the_numbering() {
    let (_rig, mut tui, _host, editor) =
        over("reply-stay-swap", b"one\ntwo\n", Clip::none());

    // The first stay's own waits: the list it opened with, and the
    // number the edit after it drew.
    reovim_rig::typed(&mut tui, "x:w<CR>");
    let first = editor.engine().epoch();
    assert!(first > 1, "the first stay numbered more than its opening");

    reovim_rig::typed(&mut tui, ":e other<CR>");
    assert_eq!(lines(&mut tui), ["three", "four"], "the second stay");
    let second = editor.engine().epoch();
    assert!(
        second > first,
        "the second stay opened on {second}, the first stay's last {first}"
    );
}
