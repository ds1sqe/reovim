//! Captured startup lines use the same command path as typed lines.

mod common;

use std::rc::Rc;

use common::one;
use reovim_client::Host;
use reovim_client::dev::{Clip, Table, Want};
use reovim_client_tui::Tui;
use reovim_plugin_vim::Editor;
use reovim_rig::{Rig, Seen, Stub, lines, opened, said, typed};
use reovim_sdk::walk::ROOT;
use reovim_sdk::{cell, log, ops};

fn rig(name: &str) -> Rig {
    let rig = Rig::new(name);
    rig.file("a", b"one\nTwo\n");
    rig.file("b", b"other\n");
    rig.file("c", b"one\nTwo\none\n");
    rig.file("out", b"");
    rig
}

fn editor(host: &Rc<Host>, init: &str, file: &str) -> Tui {
    opened(
        host,
        one(Editor::with_init(init.into())),
        Clip::none(),
        "",
        Some(file),
        None,
    )
    .unwrap()
}

fn entries(host: &Rc<Host>) -> Vec<(u8, String)> {
    let ctx = host.own();
    let rd = cell::resolve(&ctx, ROOT, "/sys/log").unwrap();
    log::parse(&ops::bytes(&ctx, rd).unwrap())
        .iter()
        .filter(|entry| entry.text().starts_with("init:"))
        .map(|entry| (entry.level, entry.text()))
        .collect()
}

#[test]
fn the_lines_run_once_across_a_second_start() {
    let rig = rig("init-once");
    let mut tui = editor(&rig.host(), "cat a >> out\n", "a");
    assert_eq!(rig.disk("out"), b"one\nTwo\n");
    typed(&mut tui, ":e b<CR>");
    tui.tick().unwrap();
    assert_eq!(lines(&mut tui), ["other"]);
    assert_eq!(rig.disk("out"), b"one\nTwo\n");
}

#[test]
fn blank_and_hash_lines_are_skipped() {
    let rig = rig("init-blank");
    let host = rig.host();
    let mut tui = editor(&host, "\n# x\n  \n", "a");
    assert!(entries(&host).is_empty());
    assert_eq!(lines(&mut tui), ["one", "Two"]);
}

#[test]
fn a_failing_line_is_logged_and_the_next_runs() {
    let rig = rig("init-failed");
    let host = rig.host();
    let mut tui = editor(&host, "set ic\nnosuch\npwd\ncat a > out\n", "a");
    assert_eq!(rig.disk("out"), b"one\nTwo\n");
    assert_eq!(
        entries(&host),
        [
            (4, "init:2: E492: Not an editor command: nosuch".into()),
            (6, "init:3: /module/fs/root".into()),
        ]
    );
    assert_eq!(said(&mut tui), "");
}

#[test]
fn a_failed_line_rings_nothing_into_the_next() {
    let rig = rig("init-bell");
    let mut tui = editor(&rig.host(), "nosuch\ng/one/d\n", "c");
    assert_eq!(lines(&mut tui), ["Two"]);
    typed(&mut tui, "x");
    assert_eq!(lines(&mut tui), ["wo"]);
}

#[test]
fn set_runs_as_typed() {
    let rig = rig("init-set");
    let mut tui = editor(&rig.host(), "set ic scs\n", "a");
    typed(&mut tui, "/two<CR>x");
    assert_eq!(lines(&mut tui), ["one", "wo"]);
}

#[test]
fn what_start_said_outlives_the_run() {
    let rig = rig("init-start-said");
    let mut tui = editor(&rig.host(), "nosuch\npwd\n", "new");
    assert_eq!(said(&mut tui), "\"new\" [New]");
}

#[test]
fn a_directory_first_runs_it_at_the_first_vim() {
    let rig = rig("init-directory");
    let host = rig.host();
    let ctx = host.own();
    let a = cell::resolve(&ctx, ROOT, "/module/fs/root/a").unwrap();
    let vim = Editor::with_init("cat a > out\ne b\n".into());
    let seen = Seen::default();
    let table: Table = Box::new(move |want| match want {
        Want::Container => Some((
            "picker".into(),
            Stub::typing(
                &seen,
                "listing",
                Box::new(move |ctx, region, _| region.opens(ctx, a, "")),
            ),
        )),
        Want::Cell => Some(("vim".into(), vim.booted())),
        Want::Named(_) => None,
    });
    let mut tui =
        opened(&host, table, Clip::none(), "", Some("."), None).unwrap();
    tui.frame(80, 24).unwrap();
    assert_eq!(rig.disk("out"), b"");
    typed(&mut tui, "l");
    assert_eq!(rig.disk("out"), b"one\nTwo\n");
    assert_eq!(lines(&mut tui), ["one", "Two"]);
    tui.tick().unwrap();
    assert_eq!(lines(&mut tui), ["other"]);
}

#[test]
fn a_line_that_leaves_ends_the_run() {
    for command in ["e b", "cli", "q", "explore"] {
        let rig = rig(&format!("init-leaves-{}", command.replace(' ', "-")));
        let host = rig.host();
        let vim = Editor::with_init(format!("{command}\ncat a > out\n"));
        let seen = Seen::default();
        let table: Table = Box::new(move |want| match want {
            Want::Cell => Some(("vim".into(), vim.booted())),
            _ => Some(("stub".into(), Stub::booted(&seen, "left"))),
        });
        let _tui =
            opened(&host, table, Clip::none(), "", Some("a"), None).unwrap();
        assert_eq!(rig.disk("out"), b"", "{command}");
        assert_eq!(
            entries(&host),
            [(4, "init:1: left; the lines after it were not run".into())]
        );
    }
}

#[test]
fn a_failed_set_keeps_the_effects_of_its_valid_options() {
    let rig = rig("init-partial-set");
    let host = rig.host();
    let mut tui = editor(&host, "set ic nosuch\n", "a");
    typed(&mut tui, "/two<CR>x");
    assert_eq!(lines(&mut tui), ["one", "wo"]);
    assert_eq!(
        entries(&host),
        [(4, "init:1: E518: Unknown option: nosuch".into())]
    );
}

#[test]
fn adjacent_global_lines_are_independent_undo_steps() {
    let rig = rig("init-global-steps");
    let mut tui = editor(&rig.host(), "g/one/s//a/\ng/a/s//b/\n", "c");
    assert_eq!(lines(&mut tui), ["b", "Two", "b"]);
    typed(&mut tui, "u");
    assert_eq!(lines(&mut tui), ["a", "Two", "a"]);
    typed(&mut tui, "u");
    assert_eq!(lines(&mut tui), ["one", "Two", "one"]);
}
