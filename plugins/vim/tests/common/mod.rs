//! What every test of Vim starts from. Each test binary compiles
//! this on its own and uses what it needs.
#![allow(dead_code)]

use std::rc::Rc;

use reovim_client::Host;
use reovim_client::dev::Table;
use reovim_client_api::keyboard::Press;
use reovim_client_api::{NONE, origin, resolver::Input};
use reovim_plugin_vim::notation::keys;
use reovim_plugin_vim::{Edit, Editor, Effect, Key, Vim, keyed};
use reovim_sdk::walk::ROOT;
use reovim_sdk::{cell, ops};

/// The press this test stands for `n`: a clock of its own counting
/// up, and the rig's own source, which is no Client of a keyboard.
/// The engine keeps it whole and reads none of it.
const fn stamped(n: u64) -> Press {
    Press { key: 0, when: n, source: NONE }
}

pub fn edit(at: usize, len: usize, text: &str, join: bool) -> Effect {
    Effect::Edit(Edit { at, len, text: text.to_string(), join })
}

/// Every key of `text` in Vim notation, each an input of a press of
/// this run's own; the last effect. What a failed prefix let go runs
/// again, and what `.` answered runs as a repeat's, both as the
/// Plugin's queue runs them.
pub fn typed(vim: &mut Vim, text: &str) -> Effect {
    let mut last = Effect::Nothing;
    for (n, key) in keys(text).unwrap().into_iter().enumerate() {
        let effect = vim.key(input(key, origin::TYPED, n));
        last = ran(vim, effect);
    }
    last
}

/// One input of `key` in `origin`, its press the `n`th of this run.
fn input(key: Key, origin: u64, n: usize) -> Input {
    keyed(key, origin, &stamped(n as u64 + 1))
}

/// One key typed, for a test that hands the engine its inputs itself.
pub fn tapped(key: Key) -> Input {
    input(key, origin::TYPED, 0)
}

/// One key of `origin`, for a test that hands the engine its inputs
/// itself: `FED` a macro's, `REPEAT` a `.`'s.
pub fn as_of(key: Key, origin: u64) -> Input {
    input(key, origin, 0)
}

/// One key of `origin` that a failed prefix let go, as the queue
/// hands it back: whose it is, and that it is not looked up again.
pub fn let_go(key: Key, origin: u64) -> Input {
    Input { released: true, ..input(key, origin, 0) }
}

/// The queue's own two turns around one effect: the keys a failed
/// prefix let go, and the keys `.` answered, each through the door
/// the Plugin puts them through. A repeat is answered as the edits
/// its keys made, which is what one `.` did.
fn ran(vim: &mut Vim, effect: Effect) -> Effect {
    let last = drained(vim, effect);
    let Effect::Fed(keys, true) = last else { return last };
    let mut edits = Vec::new();
    for (n, key) in keys.into_iter().enumerate() {
        let effect = vim.key(input(key, origin::REPEAT, n));
        match drained(vim, effect) {
            Effect::Edit(edit) => edits.push(edit),
            Effect::Edits(more) => edits.extend(more),
            _ => {}
        }
    }
    if edits.is_empty() { Effect::Nothing } else { Effect::Edits(edits) }
}

/// What `released()` holds, each through `past` and at the front, so
/// a key that lets go of more is drained before what follows it.
fn drained(vim: &mut Vim, mut last: Effect) -> Effect {
    let mut released = vim.released();
    while !released.is_empty() {
        last = vim.key(released.remove(0));
        // A released key may let go of more — an unbound Alt among
        // them — which belong in front of what still stands.
        let mut more = vim.released();
        more.append(&mut released);
        released = more;
    }
    last
}

/// Every key of `text`; every edit they made, in order, a repeat's
/// keys run as the queue runs them.
pub fn edits(vim: &mut Vim, text: &str) -> Vec<Edit> {
    let mut all = Vec::new();
    for (n, key) in keys(text).unwrap().into_iter().enumerate() {
        let effect = vim.key(input(key, origin::TYPED, n));
        match ran(vim, effect) {
            Effect::Edit(edit) => all.push(edit),
            Effect::Edits(more) => all.extend(more),
            _ => {}
        }
    }
    all
}

/// A Vim on `text`, in normal mode at the start.
pub fn on(text: &str) -> Vim {
    let mut vim = Vim::default();
    vim.loaded(text.as_bytes());
    vim
}

/// A table over one `Editor`, given for every want alike, as the
/// app's own table does for `vim` (01.0).
pub fn one(vim: Rc<Editor>) -> Table {
    Box::new(move |_| Some((String::from("vim"), vim.booted())))
}

/// What the cell `path` under `/self/vim` holds, as `cat` prints it.
pub fn cat(host: &Rc<Host>, path: &str) -> String {
    let ctx = host.own();
    let rd = cell::resolve(&ctx, ROOT, &format!("/self/vim/{path}")).unwrap();
    String::from_utf8(ops::bytes(&ctx, rd).unwrap()).unwrap()
}
