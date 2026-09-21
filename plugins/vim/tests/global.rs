//! Global commands through their declared names and the real frame.

mod common;

use std::rc::Rc;

use common::one;
use reovim_client::dev::Clip;
use reovim_client::{Host, SELF};
use reovim_client_tui::Tui;
use reovim_kernel_interface::{
    Calls, Driver, DriverId, Error, Flags, Module, Rd, Reply, Rights,
};
use reovim_plugin_vim::{Editor, notation};
use reovim_rig::{Rig, lines, listed, opened, said, status, typed};
use reovim_sdk::{cell, ops};

fn editor(host: &Rc<Host>, vim: &Rc<Editor>) -> Tui {
    let mut tui =
        opened(host, one(vim.clone()), Clip::none(), "", Some("note"), None)
            .unwrap();
    tui.frame(80, 24).unwrap();
    tui
}

fn revision(tui: &mut Tui) -> u64 {
    status(tui)
        .split_whitespace()
        .find_map(|word| {
            word.strip_prefix('r')?.trim_end_matches("[+]").parse().ok()
        })
        .expect("revision in status row")
}

#[test]
fn the_three_names_resolve_and_s_stays_substitute() {
    let rig = Rig::new("global-names");
    rig.file("note", b"a\n");
    let host = rig.host();
    let mut tui = editor(&host, &Editor::new());
    let names = listed(&host, "/self/vim/bin");
    assert_eq!(
        names,
        [
            "write",
            "wq",
            "quit",
            "xit",
            "cquit",
            "substitute",
            "set",
            "nohlsearch",
            "edit",
            "explore",
            "cli",
            "global",
            "vglobal",
            "delete",
            "buffer",
            "buffers",
            "bnext",
            "bprevious",
            "bdelete",
            "qall",
            "wall",
            "wqall",
            "bind",
            "point",
        ]
    );
    typed(&mut tui, ":help global<CR>");
    assert_eq!(said(&mut tui), "global [spec]");
    typed(&mut tui, ":help delete<CR>");
    assert_eq!(said(&mut tui), "delete [register]");
    typed(&mut tui, ":s/a/b/<CR>");
    assert_eq!(lines(&mut tui), ["b"]);
}

#[test]
fn a_global_is_one_revision_and_one_undo() {
    let rig = Rig::new("global-undo");
    rig.file("note", b"a\na\na\n");
    let mut tui = editor(&rig.host(), &Editor::new());
    let before = revision(&mut tui);
    typed(&mut tui, ":g/a/s//b/<CR>");
    assert_eq!(lines(&mut tui), ["b", "b", "b"]);
    assert_eq!(revision(&mut tui), before + 1);
    typed(&mut tui, "u");
    assert_eq!(lines(&mut tui), ["a", "a", "a"]);
    typed(&mut tui, "u");
    assert_eq!(said(&mut tui), "Already at oldest change");
}

#[test]
fn a_spent_pattern_leaves_the_text_and_the_history() {
    let rig = Rig::new("global-spent");
    let text = format!("{}\nc\n", "a".repeat(30));
    rig.file("note", text.as_bytes());
    let vim = Editor::new();
    let mut tui = editor(&rig.host(), &vim);
    let before = revision(&mut tui);
    typed(&mut tui, r":g/\(a*\)*b/d<CR>");
    assert!(said(&mut tui).starts_with("E363:"));
    assert_eq!(vim.engine().text(), text);
    assert_eq!(revision(&mut tui), before);
}

#[test]
fn a_global_inside_a_global_is_e147_and_changes_nothing() {
    let rig = Rig::new("global-recursive");
    rig.file("note", b"a\na\n");
    let mut tui = editor(&rig.host(), &Editor::new());
    typed(&mut tui, ":g/a/g/a/d<CR>");
    assert_eq!(said(&mut tui), "E147: Cannot do :global recursive");
    assert_eq!(lines(&mut tui), ["a", "a"]);
    typed(&mut tui, ":g/a/d<CR>");
    assert_eq!(lines(&mut tui), [""]);
}

#[test]
fn a_line_that_fails_ends_the_run() {
    let rig = Rig::new("global-line-fails");
    rig.file("note", b"b\na\na\n");
    let vim = Editor::new();
    let mut tui = editor(&rig.host(), &vim);
    typed(&mut tui, ":g/a/foo<CR>");
    assert_eq!(said(&mut tui), "E492: Not an editor command: foo");
    assert_eq!(vim.engine().position(), (1, 0));
    assert_eq!(lines(&mut tui), ["b", "a", "a"]);
}

/// A marked line's command that asks the region away ends the run
/// as one that fails does: the lines after it would run over a
/// Buffer this stay no longer shows. The cursor stands where the run
/// stopped, the first marked line, not the last.
#[test]
fn a_line_that_leaves_ends_the_run() {
    let rig = Rig::new("global-line-leaves");
    rig.file("note", b"a\nb\na\n");
    let vim = Editor::new();
    let mut tui = editor(&rig.host(), &vim);
    typed(&mut tui, "G");
    assert_eq!(vim.engine().position(), (2, 0));
    typed(&mut tui, ":g/a/cli<CR>");
    assert_eq!(vim.engine().position(), (0, 0));
}

#[test]
fn a_global_in_a_macro_joins_its_run() {
    let rig = Rig::new("global-macro");
    rig.file("note", b"x:g/a/s//b/\na a\na a\n");
    let mut tui = editor(&rig.host(), &Editor::new());
    typed(&mut tui, "\"ayyj@a");
    assert_eq!(lines(&mut tui), ["x:g/b/s//b/", " b", "b a"]);
    typed(&mut tui, "u");
    assert_eq!(lines(&mut tui), ["x:g/a/s//b/", "a a", "a a"]);
}

/// Reads add exactly one Host call apiece before reaching the view.
struct Depth;

impl Module for Depth {
    fn start(&self, ctx: &dyn Calls, home: Rd) -> Result<(), Error> {
        for name in ["1", "2", "3"] {
            let rd = ctx.register(DriverId(0), home, Some(name))?;
            ctx.report(rd, Flags::READ, &[])?;
        }
        Ok(())
    }
}

impl Driver for Depth {
    fn open(&self, _: &dyn Calls, rd: Rd, _: Rights) -> Reply<u64> {
        Reply::ok(rd.0)
    }

    fn read(
        &self,
        ctx: &dyn Calls,
        h: u64,
        at: u64,
        _: usize,
    ) -> Reply<Vec<u8>> {
        if at != 0 {
            return Reply::ok(Vec::new());
        }
        Self::forward(ctx, Rd(h)).into()
    }
}

impl Depth {
    fn forward(ctx: &dyn Calls, rd: Rd) -> Result<Vec<u8>, Error> {
        let name = ops::info(ctx, rd)?.name.ok_or(Error::Invalid)?;
        let path = match name.as_str() {
            "1" => "dev/windows/0/view",
            "2" => "depth/1",
            "3" => "depth/2",
            _ => return Err(Error::Invalid),
        };
        ops::bytes(ctx, cell::resolve(ctx, SELF, path)?)
    }
}

fn queued(host: &Rc<Host>, text: &str) {
    let ctx = host.own();
    let rd = cell::resolve(&ctx, SELF, "dev/input/keyboard").unwrap();
    let bytes = notation::keys(text)
        .unwrap()
        .iter()
        .flat_map(|key| key.code().to_le_bytes())
        .collect();
    ops::put(&ctx, rd, bytes).unwrap();
}

#[test]
fn a_typed_global_reaches_six_of_eight() {
    let rig = Rig::new("global-depth");
    rig.file("note", "a\n".repeat(300).as_bytes());
    let host = rig.host();
    let vim = Editor::new();
    let _tui = editor(&host, &vim);
    let depth = Rc::new(Depth);
    host.load("depth", depth.clone(), vec![depth]).unwrap();
    queued(&host, ":g/a/s//b/<CR>");
    let ctx = host.own();
    let rd = cell::resolve(&ctx, SELF, "depth/2").unwrap();
    ops::bytes(&ctx, rd).unwrap();
    assert_eq!(vim.engine().text(), "b\n".repeat(300));
    let draft = cell::resolve(&ctx, SELF, "dev/buffers/0").unwrap();
    assert_eq!(ops::bytes(&ctx, draft).unwrap(), "b\n".repeat(300).as_bytes());
}

#[test]
fn a_refused_edit_reloads_the_draft_and_ends_the_global() {
    let rig = Rig::new("global-depth-refused");
    rig.file("note", "a\n".repeat(300).as_bytes());
    let host = rig.host();
    let vim = Editor::new();
    let mut tui = editor(&host, &vim);
    let depth = Rc::new(Depth);
    host.load("depth", depth.clone(), vec![depth]).unwrap();
    queued(&host, ":g/a/s//b/<CR>");
    let ctx = host.own();
    let rd = cell::resolve(&ctx, SELF, "depth/3").unwrap();
    let _ = ops::bytes(&ctx, rd);
    let draft = cell::resolve(&ctx, SELF, "dev/buffers/0").unwrap();
    let actual = ops::bytes(&ctx, draft).unwrap();
    assert!(vim.engine().status().contains("busy"));
    let expected = format!("{}{}", "b\n".repeat(256), "a\n".repeat(44));
    assert_eq!(actual, expected.as_bytes());
    assert_eq!(vim.engine().text().as_bytes(), actual);
    typed(&mut tui, ":g/a/s//c/<CR>");
    assert_eq!(
        vim.engine().text(),
        format!("{}{}", "b\n".repeat(256), "c\n".repeat(44))
    );
}
