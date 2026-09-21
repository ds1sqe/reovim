//! Register policy and window motion through public engine/Client paths.
mod common;

use std::fmt::Write as _;

use common::{cat, on, one, typed as engine_keys};
use reovim_client::dev::Clip;
use reovim_client_api::{Key, window};
use reovim_plugin_vim::Editor;
use reovim_rig::{Rig, lines, listed, opened, said, typed};
use reovim_sdk::walk::ROOT;
use reovim_sdk::{cell, decode, encode, ops};

#[test]
fn nine_deletes_shift_and_the_tenth_drops_the_first() {
    let text: String = (0..12).fold(String::new(), |mut text, n| {
        writeln!(text, "line{n}").unwrap();
        text
    });
    let mut vim = on(&text);
    for _ in 0..10 {
        engine_keys(&mut vim, "dd");
    }
    for register in 1..=9 {
        let name = char::from_digit(register, 10).unwrap();
        assert_eq!(vim.stored(name), format!("line{}\n", 10 - register));
    }
    assert_eq!(vim.stored('0'), "");
    assert_eq!(vim.text(), "line10\nline11\n");
}

#[test]
fn a_small_delete_leaves_one_and_a_big_one_leaves_the_small() {
    let mut vim = on("first\none two\nlast\n");
    engine_keys(&mut vim, "yydddw");
    assert_eq!(vim.stored('0'), "first\n");
    assert_eq!(vim.stored('1'), "first\n");
    assert_eq!(vim.stored('-'), "one ");
    engine_keys(&mut vim, "dd");
    assert_eq!(vim.stored('1'), "two\n");
    assert_eq!(vim.stored('2'), "first\n");
    assert_eq!(vim.stored('-'), "one ");
    assert_eq!(vim.stored('0'), "first\n");
}

/// A search is the search Module's, so this case is over a Client
/// that has loaded it, where the engine's own cases are not.
#[test]
fn a_delete_by_search_or_star_writes_both() {
    for (n, keys) in ["d*", "2wd#", "/one<CR>ggdn"].iter().enumerate() {
        let rig = Rig::new(&format!("registers-search-{n}"));
        rig.file("file", b"one two one three\nlast\n");
        let host = rig.searching(None);
        let mut tui = opened(
            &host,
            one(Editor::new()),
            Clip::none(),
            "",
            Some("file"),
            None,
        )
        .unwrap();
        typed(&mut tui, keys);
        assert_eq!(cat(&host, "registers/1"), "one two ", "{keys}");
        assert_eq!(cat(&host, "registers/-"), "one two ", "{keys}");
        assert_eq!(lines(&mut tui), ["one three", "last"], "{keys}");
    }
}

#[test]
fn named_digit_writes_precede_shifting_and_yanks_do_not_shift() {
    let mut vim = on("one\ntwo\nthree\n");
    engine_keys(&mut vim, "dd\"1dd");
    assert_eq!(vim.stored('1'), "two\n");
    assert_eq!(vim.stored('2'), "two\n");
    let mut vim = on("one\ntwo\nthree\n");
    engine_keys(&mut vim, "dd\"add\"Add");
    assert_eq!(vim.stored('a'), "two\nthree\n");
    assert_eq!(vim.stored('1'), "three\n");
    assert_eq!(vim.stored('2'), "two\n");
    let mut vim = on("one\ntwo\nthree\n");
    engine_keys(&mut vim, "dd\"1yy");
    assert_eq!(vim.stored('1'), "two\n");
    assert_eq!(vim.stored('2'), "");
    assert_eq!(vim.stored('0'), "");
    engine_keys(&mut vim, "\"1p");
    assert_eq!(vim.text(), "two\ntwo\nthree\n");
}

#[test]
fn black_hole_and_named_small_deletes_preserve_history() {
    let mut vim = on("first\none two\nlast\n");
    engine_keys(&mut vim, "yydd\"adw\"_dd");
    assert_eq!(vim.stored('0'), "first\n");
    assert_eq!(vim.stored('1'), "first\n");
    assert_eq!(vim.stored('-'), "");
    assert_eq!(vim.stored('a'), "one ");
    assert_eq!(vim.stored('"'), "one ");
}

#[test]
fn explicit_clipboard_selection_is_named_for_automatic_registers() {
    for name in ['+', '*'] {
        let mut vim = on("first\none two\nlast\n");
        engine_keys(&mut vim, "yydd");
        engine_keys(&mut vim, &format!("\"{name}yy"));
        assert_eq!(vim.clipped(), Some(String::from("one two\n")));
        assert_eq!(vim.stored('0'), "first\n");
        engine_keys(&mut vim, &format!("\"{name}dw"));
        assert_eq!(vim.clipped(), Some(String::from("one ")));
        assert_eq!(vim.stored('-'), "");
        assert_eq!(vim.stored('1'), "first\n");
        engine_keys(&mut vim, &format!("\"{name}dd"));
        assert_eq!(vim.clipped(), Some(String::from("two\n")));
        assert_eq!(vim.stored('1'), "two\n");
        assert_eq!(vim.stored('2'), "first\n");
        assert_eq!(vim.stored('-'), "");
    }
}

#[test]
fn the_thirty_eight_cells_have_their_distinct_contents() {
    let rig = Rig::new("register-cells");
    rig.file("file", b"one\ntwo\n");
    let host = rig.host();
    let mut tui =
        opened(&host, one(Editor::new()), Clip::none(), "", Some("file"), None)
            .unwrap();
    typed(&mut tui, "yyjdd");
    let expected: Vec<String> = std::iter::once('"')
        .chain('0'..='9')
        .chain('a'..='z')
        .chain(std::iter::once('-'))
        .map(|c| c.to_string())
        .collect();
    assert_eq!(listed(&host, "/self/vim/registers"), expected);
    assert_eq!(cat(&host, "registers/0"), "one\n");
    assert_eq!(cat(&host, "registers/1"), "two\n");
    assert_eq!(cat(&host, "registers/-"), "");
}

#[test]
fn ex_dash_selects_small_register_but_numeric_counts_stay_refused() {
    let rig = Rig::new("register-ex-dash");
    rig.file("file", b"one\ntwo\nthree\n");
    let host = rig.host();
    let editor = Editor::new();
    let mut tui = opened(
        &host,
        one(editor.clone()),
        Clip::none(),
        "",
        Some("file"),
        None,
    )
    .unwrap();
    typed(&mut tui, ":d -<CR>");
    assert_eq!(editor.engine().text(), "two\nthree\n");
    assert_eq!(cat(&host, "registers/-"), "one\n");
    assert_eq!(cat(&host, "registers/1"), "one\n");
    for arg in ["1", "ab", "?", "aa"] {
        typed(&mut tui, &format!(":d {arg}<CR>"));
        assert!(said(&mut tui).starts_with("E488:"), "{arg}");
        assert_eq!(editor.engine().text(), "two\nthree\n");
        assert_eq!(cat(&host, "registers/-"), "one\n");
    }
}

#[test]
fn g_and_h_in_one_frame_read_the_resized_window() {
    for (height, rows, keys, expected) in [
        (24, 23, "GH", 7),
        (24, 9, "GH", 21),
        (24, 9, "GM", 25),
        (24, 9, "GL", 29),
        (10, 23, "GH", 7),
    ] {
        let rig = Rig::new(&format!("window-{rows}-{keys}"));
        let text: String = (1..=30).fold(String::new(), |mut text, n| {
            writeln!(text, "{n}").unwrap();
            text
        });
        rig.file("file", text.as_bytes());
        let host = rig.host();
        let editor = Editor::new();
        let mut tui = opened(
            &host,
            one(editor.clone()),
            Clip::none(),
            "",
            Some("file"),
            None,
        )
        .unwrap();
        tui.frame(80, height).unwrap();
        let ctx = host.own();
        let size =
            cell::resolve(&ctx, ROOT, "/self/dev/windows/0/size").unwrap();
        let before: window::Size =
            decode(&ops::bytes(&ctx, size).unwrap()).unwrap();
        assert_eq!(before.rows, u64::from(height - 1));
        ops::put(&ctx, size, encode(&window::Size { cols: 80, rows })).unwrap();
        let resized: window::Size =
            decode(&ops::bytes(&ctx, size).unwrap()).unwrap();
        assert_eq!(resized.rows, rows);
        let keyboard =
            cell::resolve(&ctx, ROOT, "/self/dev/input/keyboard").unwrap();
        let bytes = keys
            .chars()
            .flat_map(|c| Key::Char(c).code().to_le_bytes())
            .collect();
        ops::put(&ctx, keyboard, bytes).unwrap();
        tui.tick().unwrap();
        assert_eq!(editor.engine().position().0, expected, "{rows} {keys}");
    }
}

#[test]
fn invalid_ex_delete_arguments_preserve_text_and_registers() {
    let rig = Rig::new("register-ex-invalid");
    rig.file("file", b"one\ntwo\n");
    let host = rig.host();
    let editor = Editor::new();
    let mut tui = opened(
        &host,
        one(editor.clone()),
        Clip::none(),
        "",
        Some("file"),
        None,
    )
    .unwrap();
    typed(&mut tui, "\"ayy");
    for arg in ["1", "ab", "?", "aa"] {
        typed(&mut tui, &format!(":d {arg}<CR>"));
        assert!(said(&mut tui).starts_with("E488:"), "{arg}");
        assert_eq!(editor.engine().text(), "one\ntwo\n");
        assert_eq!(cat(&host, "registers/a"), "one\n");
    }
}

#[test]
fn visual_charwise_paste_leaves_the_cursor_after_its_inserted_text() {
    let mut vim = on("one two\nthree\n");
    engine_keys(&mut vim, "yiwwviwp");
    assert_eq!(vim.text(), "one one\nthree\n");
    assert_eq!(vim.position(), (0, 6));
    assert_eq!(vim.stored('-'), "two");
    engine_keys(&mut vim, "\"-p");
    assert_eq!(vim.text(), "one onetwo\nthree\n");
    assert_eq!(vim.position(), (0, 9));
}

fn seeded_unnamed_write() -> reovim_plugin_vim::Vim {
    let mut vim = on("zero\nsmall\nsource\n");
    engine_keys(&mut vim, "\"0yyj\"-yyj\"ayy");
    assert_eq!(vim.stored('0'), "zero\n");
    assert_eq!(vim.stored('-'), "small\n");
    assert_eq!(vim.stored('a'), "source\n");
    vim
}

#[test]
fn explicit_unnamed_yank_writes_zero() {
    let mut vim = seeded_unnamed_write();
    engine_keys(&mut vim, "\"\"yy");
    assert_eq!(vim.stored('0'), "source\n");
    assert_eq!(vim.stored('-'), "small\n");
    assert_eq!(vim.stored('a'), "source\n");
}

#[test]
fn explicit_unnamed_delete_writes_zero_and_preserves_small() {
    let mut vim = seeded_unnamed_write();
    engine_keys(&mut vim, "\"\"x");
    assert_eq!(vim.stored('0'), "s");
    assert_eq!(vim.stored('-'), "small\n");
    assert_eq!(vim.stored('a'), "source\n");
    assert_eq!(vim.stored('"'), "s");
}
