//! Macro journeys through the real Client and draft authority.
mod common;

use common::{cat, one};
use reovim_client::dev::Clip;
use reovim_client_api::{Key, keyboard};
use reovim_kernel_interface::{Calls, Error, Rights};
use reovim_plugin_vim::Editor;
use reovim_rig::{Rig, lines, opened, said, typed};
use reovim_sdk::walk::ROOT;
use reovim_sdk::{cell, decode, ops};

#[test]
fn recording_and_append_are_visible_without_changing_unnamed() {
    let rig = Rig::new("macro-recording");
    rig.file("file", b"abcdef\n");
    let host = rig.host();
    let mut tui =
        opened(&host, one(Editor::new()), Clip::none(), "", Some("file"), None)
            .unwrap();
    typed(&mut tui, "qa");
    assert_eq!(cat(&host, "recording"), "a\n");
    assert!(said(&mut tui).contains("recording @a"));
    typed(&mut tui, "xq");
    assert_eq!(cat(&host, "recording"), "");
    assert_eq!(cat(&host, "registers/a"), "x");
    typed(&mut tui, "qAlq");
    assert_eq!(cat(&host, "registers/a"), "xl");
    assert_eq!(cat(&host, "registers/\""), "a");
}

#[test]
fn replay_never_writes_keyboard_events() {
    let rig = Rig::new("macro-stream");
    rig.file("file", b"abcdef\n");
    let host = rig.host();
    let mut tui =
        opened(&host, one(Editor::new()), Clip::none(), "", Some("file"), None)
            .unwrap();
    typed(&mut tui, "qaxq");
    let ctx = host.own();
    let rd = cell::resolve(&ctx, ROOT, "/self/dev/input/keyboard").unwrap();
    let h = ctx.open(rd, Rights::READ).unwrap();
    typed(&mut tui, "@a");
    for key in [Key::Char('@'), Key::Char('a')] {
        let bytes = ops::read(&ctx, h, 0, 4096).unwrap();
        let press: keyboard::Press = decode(&bytes).unwrap();
        assert_eq!(Key::of(press.key), Some(key));
    }
    assert_eq!(ops::read(&ctx, h, 0, 4096), Err(Error::Busy));
    assert_eq!(lines(&mut tui), ["cdef"]);
    ops::close(&ctx, h).unwrap();
}

#[test]
fn a_recursive_run_stops_and_the_next_typed_key_works() {
    let rig = Rig::new("macro-budget");
    rig.file("file", b"lh@a\nabcdef\n");
    let mut tui = opened(
        &rig.host(),
        one(Editor::new()),
        Clip::none(),
        "",
        Some("file"),
        None,
    )
    .unwrap();
    typed(&mut tui, "\"ayyj@a");
    assert_eq!(said(&mut tui), "@: stopped after 100000 keys");
    typed(&mut tui, "x");
    assert_eq!(lines(&mut tui), ["lh@a", "bcdef"]);
}

#[test]
fn nested_replay_writes_changed_bytes_to_disk() {
    let rig = Rig::new("macro-nested-save");
    rig.file("file", b":w\n@c\n@b\nold\n");
    let mut tui = opened(
        &rig.host(),
        one(Editor::new()),
        Clip::none(),
        "",
        Some("file"),
        None,
    )
    .unwrap();
    typed(&mut tui, "\"cyyj\"byyj\"ayyjA!<Esc>@a");
    assert_eq!(rig.disk("file"), b":w\n@c\n@b\nold!\n");
}

#[test]
fn first_edit_after_motion_and_dot_belongs_to_the_run() {
    let rig = Rig::new("macro-undo-dot");
    rig.file("file", b"jx.\nabcdef\nghijkl\n");
    let mut tui = opened(
        &rig.host(),
        one(Editor::new()),
        Clip::none(),
        "",
        Some("file"),
        None,
    )
    .unwrap();
    typed(&mut tui, "\"ayyjx@a");
    assert_eq!(lines(&mut tui), ["jx.", "bcdef", "ijkl"]);
    typed(&mut tui, "u");
    assert_eq!(lines(&mut tui), ["jx.", "bcdef", "ghijkl"]);
    typed(&mut tui, "u");
    assert_eq!(lines(&mut tui), ["jx.", "abcdef", "ghijkl"]);
}

#[test]
fn ex_driver_edits_keep_the_macro_revision() {
    let rig = Rig::new("macro-ex-undo");
    rig.file("file", b":s/a/x/\n:s/b/y/\naabb\n");
    let mut tui = opened(
        &rig.host(),
        one(Editor::new()),
        Clip::none(),
        "",
        Some("file"),
        None,
    )
    .unwrap();
    typed(&mut tui, "\"a2yyjj@a");
    assert_eq!(lines(&mut tui), [":s/a/x/", ":s/b/y/", "xayb"]);
    typed(&mut tui, "u");
    assert_eq!(lines(&mut tui), [":s/a/x/", ":s/b/y/", "aabb"]);
}

#[test]
fn clipboard_completion_keeps_the_macro_revision() {
    let rig = Rig::new("macro-clip-undo");
    rig.file("file", b"x\"+px\nabcdef\n");
    rig.clip(b"Q");
    let mut tui = opened(
        &rig.host(),
        one(Editor::new()),
        rig.clipping(),
        "",
        Some("file"),
        None,
    )
    .unwrap();
    typed(&mut tui, "\"ayyj@a");
    assert_eq!(lines(&mut tui), ["x\"+px", "bcdef"]);
    typed(&mut tui, "u");
    assert_eq!(lines(&mut tui), ["x\"+px", "abcdef"]);
}

#[test]
fn a_run_crosses_a_buffer_and_starts_its_own_undo_step() {
    let rig = Rig::new("macro-swap-undo");
    rig.file("file", b"x:e! other\nx\n");
    rig.file("other", b"abc\n");
    let mut tui = opened(
        &rig.host(),
        one(Editor::new()),
        Clip::none(),
        "",
        Some("file"),
        None,
    )
    .unwrap();
    typed(&mut tui, "\"a2yy@a");
    tui.tick().unwrap();
    assert_eq!(lines(&mut tui), ["bc"]);
    typed(&mut tui, "u");
    assert_eq!(lines(&mut tui), ["abc"]);
    typed(&mut tui, "u");
    assert_eq!(said(&mut tui), "Already at oldest change");
}

#[test]
fn switching_at_the_budget_keeps_the_diagnostic_and_drops_the_tail() {
    let rig = Rig::new("macro-swap-budget");
    // 99,991 no-op zeros followed by nine keys in ':e other\r'.
    let macro_text = format!("{}:e other\nx", "0".repeat(99_991));
    rig.file("file", format!("{macro_text}\n").as_bytes());
    rig.file("other", b"abc\n");
    let mut tui = opened(
        &rig.host(),
        one(Editor::new()),
        Clip::none(),
        "",
        Some("file"),
        None,
    )
    .unwrap();
    typed(&mut tui, "\"a2yy@a");
    tui.tick().unwrap();
    assert_eq!(lines(&mut tui), ["abc"]);
    assert_eq!(said(&mut tui), "@: stopped after 100000 keys");
    typed(&mut tui, "x");
    assert_eq!(lines(&mut tui), ["bc"]);
}

#[test]
fn a_bell_drops_replay_but_preserves_typed_keys_in_one_frame() {
    let rig = Rig::new("macro-typed-bell");
    rig.file("file", b"hx\nabcdef\n");
    let host = rig.host();
    let mut tui =
        opened(&host, one(Editor::new()), Clip::none(), "", Some("file"), None)
            .unwrap();
    typed(&mut tui, "\"ayiwj");
    let ctx = host.own();
    let keyboard =
        cell::resolve(&ctx, ROOT, "/self/dev/input/keyboard").unwrap();
    let bytes =
        "@ax".chars().flat_map(|c| Key::Char(c).code().to_le_bytes()).collect();
    ops::put(&ctx, keyboard, bytes).unwrap();
    tui.tick().unwrap();
    assert_eq!(lines(&mut tui), ["hx", "bcdef"]);
}

#[test]
fn stale_draft_stops_the_remaining_macro_edits() {
    let rig = Rig::new("macro-stale");
    rig.file("file", b"abcdef\n");
    let host = rig.host();
    let mut first =
        opened(&host, one(Editor::new()), Clip::none(), "", Some("file"), None)
            .unwrap();
    typed(&mut first, "qaxq");
    let mut second = opened(
        &rig.host(),
        one(Editor::new()),
        Clip::none(),
        "",
        Some("file"),
        None,
    )
    .unwrap();
    typed(&mut second, "A!<Esc>");
    assert_eq!(lines(&mut second), ["bcdef!"]);
    assert_eq!(lines(&mut first), ["bcdef"]);
    let mut observer = rig.client();
    let source = observer.resolve("/module/fs/root/file").unwrap();
    let buffer =
        observer.resolve(&format!("/module/text/{}", source.0)).unwrap();
    assert_eq!(observer.read_all(buffer).unwrap(), b"bcdef!\n");
    // The typed edit is pending when the macro's first edit flushes it.
    let ctx = host.own();
    let keyboard =
        cell::resolve(&ctx, ROOT, "/self/dev/input/keyboard").unwrap();
    let bytes = "x3@a"
        .chars()
        .flat_map(|c| Key::Char(c).code().to_le_bytes())
        .collect();
    ops::put(&ctx, keyboard, bytes).unwrap();
    first.tick().unwrap();
    assert_eq!(lines(&mut first), ["cdef"]);
    assert_eq!(
        said(&mut first),
        "W12: changed in another Client and here; :e! takes theirs"
    );
}

#[test]
fn stale_replay_reaches_its_first_edit_then_drops_its_tail() {
    let rig = Rig::new("macro-stale-motion");
    rig.file("file", b"abcdef\n");
    let host = rig.host();
    let editor = Editor::new();
    let mut first = opened(
        &host,
        one(editor.clone()),
        Clip::none(),
        "",
        Some("file"),
        None,
    )
    .unwrap();
    typed(&mut first, "qalxq");
    let mut second = opened(
        &rig.host(),
        one(Editor::new()),
        Clip::none(),
        "",
        Some("file"),
        None,
    )
    .unwrap();
    typed(&mut second, "A!<Esc>");
    assert_eq!(lines(&mut first), ["acdef"]);
    assert_eq!(lines(&mut second), ["acdef!"]);
    let mut observer = rig.client();
    let source = observer.resolve("/module/fs/root/file").unwrap();
    let buffer =
        observer.resolve(&format!("/module/text/{}", source.0)).unwrap();
    assert_eq!(observer.read_all(buffer).unwrap(), b"acdef!\n");
    let ctx = host.own();
    let keyboard =
        cell::resolve(&ctx, ROOT, "/self/dev/input/keyboard").unwrap();
    let bytes = "x3@a"
        .chars()
        .flat_map(|c| Key::Char(c).code().to_le_bytes())
        .collect();
    ops::put(&ctx, keyboard, bytes).unwrap();
    first.tick().unwrap();
    assert_eq!(lines(&mut first), ["adef"]);
    assert_eq!(
        said(&mut first),
        "W12: changed in another Client and here; :e! takes theirs"
    );
    assert_eq!(
        editor.engine().position(),
        (0, 2),
        "the first fed l ran; later l keys were discarded"
    );
    assert_eq!(observer.read_all(buffer).unwrap(), b"acdef!\n");
}
