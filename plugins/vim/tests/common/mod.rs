//! What every test of Vim starts from. Each test binary compiles
//! this on its own and uses what it needs.
#![allow(dead_code)]

use std::collections::VecDeque;
use std::rc::Rc;

use reovim_client::Host;
use reovim_client::dev::Table;
use reovim_plugin_vim::notation::keys;
use reovim_plugin_vim::{Edit, Editor, Effect, Stroke, Vim};
use reovim_sdk::walk::ROOT;
use reovim_sdk::{cell, ops};

pub fn edit(at: usize, len: usize, text: &str, join: bool) -> Effect {
    Effect::Edit(Edit { at, len, text: text.to_string(), join })
}

/// Every key of `text` in Vim notation; the last effect. What a
/// failed prefix let go runs again, as the Plugin's queue runs it.
pub fn typed(vim: &mut Vim, text: &str) -> Effect {
    let mut last = Effect::Nothing;
    for key in keys(text).unwrap() {
        let effect = vim.key(key);
        last = drained(vim, effect);
    }
    last
}

/// What `released()` holds, each through `past` and at the front, so
/// a key that lets go of more is drained before what follows it.
fn drained(vim: &mut Vim, mut last: Effect) -> Effect {
    let mut queue: VecDeque<Stroke> = vim.released().into();
    while let Some(stroke) = queue.pop_front() {
        last = vim.past(stroke);
        for stroke in vim.released().into_iter().rev() {
            queue.push_front(stroke);
        }
    }
    last
}

/// Every key of `text`; every edit they made, in order.
pub fn edits(vim: &mut Vim, text: &str) -> Vec<Edit> {
    let mut all = Vec::new();
    for key in keys(text).unwrap() {
        match vim.key(key) {
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
