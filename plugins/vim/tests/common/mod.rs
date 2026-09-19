//! What every test of Vim starts from. Each test binary compiles
//! this on its own and uses what it needs.
#![allow(dead_code)]

use std::rc::Rc;

use reovim_client::Host;
use reovim_client::dev::Table;
use reovim_plugin_vim::notation::keys;
use reovim_plugin_vim::{Edit, Editor, Effect, Vim};
use reovim_sdk::walk::ROOT;
use reovim_sdk::{cell, ops};

pub fn edit(at: usize, len: usize, text: &str, join: bool) -> Effect {
    Effect::Edit(Edit { at, len, text: text.to_string(), join })
}

/// Every key of `text` in Vim notation; the last effect.
pub fn typed(vim: &mut Vim, text: &str) -> Effect {
    let mut last = Effect::Nothing;
    for key in keys(text).unwrap() {
        last = vim.key(key);
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
