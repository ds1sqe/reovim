//! What a case over an editor does: open the tui the app opens, type
//! at it in Vim's notation or in keys, and read the frame back.
//!
//! What stays beside the case is its own table, the Plugin it tests.

use std::rc::Rc;

use reovim_client::dev::{self, Clip, Table};
use reovim_client::{Error, Host};
use reovim_client_api::Key;
use reovim_client_tui::{Frame, Step, Tui};
use reovim_plugin_vim::notation::keys;
use reovim_sdk::walk::ROOT;
use reovim_sdk::{cell, ops};

/// The tui over `host`, started in `dir` over `file` at `line`, the
/// Plugins it opens being what `table` gives and its machine `clip`.
///
/// # Errors
/// What the boot of `/self/dev` or the first read of its cells said.
pub fn opened(
    host: &Rc<Host>,
    table: Table,
    clip: Clip,
    dir: &str,
    file: Option<&str>,
    line: Option<u64>,
) -> Result<Tui, Error> {
    let dev = dev::boot(host, table, clip, dir, file, line)?;
    Tui::open(host.clone(), dev)
}

/// Every key of `text` in Vim notation; what the last one did.
///
/// # Panics
/// When the notation names no key, or the editor refuses one.
pub fn typed(tui: &mut Tui, text: &str) -> Step {
    let mut step = Step::Same;
    for key in keys(text).unwrap() {
        step = tui.key(key).unwrap();
    }
    step
}

/// Those keys, in order, and what the last one did.
///
/// # Panics
/// When the editor refuses one.
pub fn pressed(tui: &mut Tui, keys: &[Key]) -> Step {
    let mut step = Step::Same;
    for key in keys {
        step = tui.key(*key).unwrap();
    }
    step
}

/// The frame at the size the app always asks for.
///
/// # Panics
/// When the region refuses to draw.
pub fn frame(tui: &mut Tui) -> Frame {
    tui.frame(80, 24).unwrap()
}

/// The rows that frame shows.
///
/// # Panics
/// As [`frame`].
pub fn lines(tui: &mut Tui) -> Vec<String> {
    frame(tui).lines
}

/// The status row of that frame.
///
/// # Panics
/// As [`frame`].
pub fn status(tui: &mut Tui) -> String {
    frame(tui).status
}

/// What the Plugin itself put in the status row: what follows the name,
/// the position and the revision, which the tui writes before it.
///
/// # Panics
/// As [`frame`], and when the row holds none of that.
pub fn said(tui: &mut Tui) -> String {
    status(tui).split_once("   ").unwrap().1.into()
}

/// The names `ls` lists under `path`, in the order the tree holds them.
///
/// # Panics
/// When `path` does not resolve, or a child under it has no name.
pub fn listed(host: &Rc<Host>, path: &str) -> Vec<String> {
    let ctx = host.own();
    let rd = cell::resolve(&ctx, ROOT, path).unwrap();
    ops::children(&ctx, rd)
        .unwrap()
        .into_iter()
        .map(|child| ops::info(&ctx, child).unwrap().name.unwrap())
        .collect()
}
