//! Buffer navigation, saving and closure through the Client's drafts.
mod common;

use std::cell::Cell;
use std::rc::Rc;

use common::cat;
use reovim_client::Host;
use reovim_client::dev::{Clip, Table, Want};
use reovim_client_api::{Key, NONE};
use reovim_client_region::{Region, drafted};
use reovim_client_tui::{Step, Tui};
use reovim_kernel_interface::{Error, Rd};
use reovim_plugin_vim::Editor;
use reovim_rig::{Rig, Seen, Stub, lines, opened, said, status, typed};
use reovim_sdk::walk::ROOT;
use reovim_sdk::{cell, ops};
use reovim_text_api::history;

const E37: &str = "E37: No write since last change (add ! to override)";
const W12: &str = "W12: changed in another Client and here; :e! takes theirs";

struct Scene {
    rig: Rig,
    host: Rc<Host>,
    editor: Rc<Editor>,
    tui: Tui,
    asks: Rc<Cell<usize>>,
}

impl Scene {
    fn new(name: &str, file: Option<&str>) -> Self {
        let rig = Rig::new(name);
        rig.file("a", b"abc\ndef\n");
        rig.file("b", b"ghi\njkl\n");
        rig.file("c", b"mno\npqr\n");
        std::fs::create_dir(rig.dir().join("sub")).unwrap();
        std::fs::create_dir(rig.dir().join("elsewhere")).unwrap();
        let host = rig.host();
        let editor = Editor::new();
        let held = editor.clone();
        let asks = Rc::new(Cell::new(0));
        let requests = asks.clone();
        let seen = Seen::default();
        let table: Table = Box::new(move |want| {
            requests.set(requests.get() + 1);
            match want {
                Want::Cell => Some((String::from("vim"), held.booted())),
                Want::Container | Want::Named("cli") => Some((
                    String::from("stub"),
                    Stub::typing(
                        &seen,
                        "other plugin",
                        Box::new(|ctx, region, _| region.backs(ctx)),
                    ),
                )),
                Want::Named(_) => None,
            }
        });
        let mut tui =
            opened(&host, table, Clip::none(), "", file, None).unwrap();
        tui.frame(80, 24).unwrap();
        let scene = Self { rig, host, editor, tui, asks };
        let initial = scene.draft("0");
        let source = ops::lookup(&scene.host.own(), initial, "source");
        if let Some(file) =
            file.filter(|file| scene.rig.dir().join(file).is_file())
        {
            let held = ops::bytes(&scene.host.own(), source.unwrap()).unwrap();
            let got: Rd = reovim_sdk::decode(&held).unwrap();
            assert_eq!(got, scene.rd(&format!("/module/fs/root/{file}")));
        } else {
            assert!(matches!(source, Err(Error::Gone | Error::Unsupported)));
        }
        scene
    }

    fn go(&mut self, keys: &str) -> Step {
        typed(&mut self.tui, keys)
    }

    fn rd(&self, path: &str) -> Rd {
        cell::resolve(&self.host.own(), ROOT, path).unwrap()
    }

    fn draft(&self, name: &str) -> Rd {
        let rd = self.rd(&format!("/self/dev/buffers/{name}"));
        assert_eq!(
            ops::info(&self.host.own(), rd).unwrap().name.as_deref(),
            Some(name)
        );
        rd
    }

    fn preload(&self, name: &str) -> Rd {
        let source = self.rd(&format!("/module/fs/root/{name}"));
        let ctx = self.host.own();
        let draft = drafted(&ctx, Some(source)).unwrap();
        let child = ops::lookup(&ctx, draft, "source").unwrap();
        let got: Rd =
            reovim_sdk::decode(&ops::bytes(&ctx, child).unwrap()).unwrap();
        assert_eq!(got, source);
        draft
    }

    fn server(&self, file: &str) -> Vec<u8> {
        let mut observer = self.rig.client();
        let source =
            observer.resolve(&format!("/module/fs/root/{file}")).unwrap();
        let buffer =
            observer.resolve(&format!("/module/text/{}", source.0)).unwrap();
        observer.read_all(buffer).unwrap()
    }

    fn modified(&self, draft: Rd, text: &[u8]) {
        let ctx = self.host.own();
        ops::put(&ctx, draft, text.to_vec()).unwrap();
        assert_eq!(ops::bytes(&ctx, draft).unwrap(), text);
        let bytes = ops::call_by_name(&ctx, draft, "tree", Vec::new()).unwrap();
        let tree: history::Tree = reovim_sdk::decode(&bytes).unwrap();
        assert_ne!(tree.head, tree.published);
    }

    fn scratch(&mut self) {
        let ctx = self.host.own();
        Region::of(&ctx).unwrap().opens(&ctx, NONE, "").unwrap();
        self.tui.tick().unwrap();
        assert!(status(&mut self.tui).starts_with("[No Name]"));
    }
}

#[test]
fn b_takes_a_buffer_by_its_name_in_the_tree() {
    let mut s = Scene::new("buffers-names", Some("a"));
    s.go(":e b<CR>:b 0<CR>");
    assert_eq!(lines(&mut s.tui), ["abc", "def"]);
    s.go(":b 7<CR>");
    assert_eq!(said(&mut s.tui), "E86: Buffer 7 does not exist");
    s.go(":b #<CR>");
    assert_eq!(lines(&mut s.tui), ["ghi", "jkl"]);
}

#[test]
fn ctrl_6_is_ctrl_caret() {
    let mut s = Scene::new("buffers-control", Some("a"));
    s.go("jl:e b<CR>");
    s.tui.key(Key::Ctrl('6')).unwrap();
    assert_eq!(lines(&mut s.tui), ["abc", "def"]);
    assert_eq!(s.editor.engine().position(), (1, 1));
}

#[test]
fn a_stay_left_for_the_cli_keeps_the_alternate() {
    let mut s = Scene::new("buffers-cli", Some("a"));
    s.go(":e b<CR>:cli<CR>");
    assert_eq!(said(&mut s.tui), "other plugin");
    s.go(" ");
    assert_eq!(lines(&mut s.tui), ["ghi", "jkl"]);
    s.go("<C-^>");
    assert_eq!(lines(&mut s.tui), ["abc", "def"]);
}

#[test]
fn an_unsaved_buffer_is_left_and_found_as_it_was() {
    let mut s = Scene::new("buffers-hidden", Some("a"));
    s.go("x:e b<CR>");
    assert_eq!(lines(&mut s.tui), ["ghi", "jkl"]);
    s.go(":e a<CR>");
    assert_eq!(lines(&mut s.tui), ["bc", "def"]);
    s.go("u");
    assert_eq!(lines(&mut s.tui), ["abc", "def"]);
}

#[test]
fn an_unsaved_buffer_over_nothing_keeps_e37() {
    let mut s = Scene::new("buffers-scratch-guard", None);
    s.go("ix<Esc>:e b<CR>");
    assert_eq!(said(&mut s.tui), E37);
    assert_eq!(lines(&mut s.tui), ["x"]);
}

#[test]
fn explore_is_refused_while_any_buffer_is_unsaved() {
    let mut s = Scene::new("buffers-explore-guard", Some("a"));
    s.go("x:explore<CR>");
    assert_eq!(said(&mut s.tui), E37);
    s.go(":w<CR>:e b<CR>x:e a<CR>:explore<CR>");
    assert_eq!(
        said(&mut s.tui),
        "E162: No write since last change for buffer \"b\""
    );
    assert_eq!(lines(&mut s.tui), ["bc", "def"]);
    s.go(":explore!<CR>");
    assert_eq!(said(&mut s.tui), "other plugin");
}

#[test]
fn a_buffer_over_nothing_is_never_the_alternate() {
    let mut s = Scene::new("buffers-scratch-alt", Some("a"));
    s.go(":e b<CR>:e nofile<CR>");
    assert!(!s.rig.dir().join("nofile").exists());
    assert_eq!(lines(&mut s.tui), [""]);
    s.go("<C-^>");
    assert_eq!(lines(&mut s.tui), ["ghi", "jkl"]);
    s.go("<C-^>");
    assert_eq!(said(&mut s.tui), "E23: No alternate file");
}

#[test]
fn a_switch_onto_its_own_buffer_answers_nothing() {
    let mut s = Scene::new("buffers-same", Some("a"));
    s.go(":e b<CR>jlma");
    let before = s.asks.get();
    let jumps = cat(&s.host, "jumps");
    s.go(":b 1<CR>:e b<CR>");
    assert_eq!(s.asks.get(), before, "no new region selection");
    assert_eq!(cat(&s.host, "jumps"), jumps);
    assert_eq!(s.editor.engine().position(), (1, 1));
}

#[test]
fn q_with_another_buffer_unsaved_says_e162_and_goes_there() {
    for quit in [":q", ":q!", ":x", ":wq", "ZZ"] {
        let mut s = Scene::new(&format!("buffers-quit-{quit}"), Some("a"));
        s.go("x:e b<CR>");
        assert_eq!(lines(&mut s.tui), ["ghi", "jkl"]);
        let keys =
            if quit == "ZZ" { quit.to_string() } else { format!("{quit}<CR>") };
        assert_ne!(s.go(&keys), Step::Quit(0));
        assert_eq!(lines(&mut s.tui), ["bc", "def"]);
        assert_eq!(
            said(&mut s.tui),
            "E162: No write since last change for buffer \"a\""
        );
    }
}

#[test]
fn wqa_writes_every_buffer_and_ends_a_last_line() {
    let mut s = Scene::new("buffers-write-all", Some("a"));
    let other = s.preload("b");
    s.modified(other, b"written without newline");
    s.go("x");
    assert_eq!(s.go(":wqa<CR>"), Step::Quit(0));
    assert_eq!(s.rig.disk("a"), b"bc\ndef\n");
    assert_eq!(s.rig.disk("b"), b"written without newline\n");
}

#[test]
fn bd_closes_the_rd_and_the_list_is_one_shorter() {
    let mut s = Scene::new("buffers-close", Some("a"));
    let a = s.draft("0");
    let b = s.preload("b");
    s.go(":bd 7<CR>");
    assert_eq!(said(&mut s.tui), "E516: No buffers were deleted");
    assert!(ops::info(&s.host.own(), a).is_ok());
    s.go(":bd<CR>");
    s.tui.tick().unwrap();
    assert_eq!(ops::info(&s.host.own(), a), Err(Error::Invalid));
    assert!(ops::info(&s.host.own(), b).is_ok());
    assert_eq!(lines(&mut s.tui), ["ghi", "jkl"]);
}

#[test]
fn bd_of_an_unsaved_buffer_is_e89_and_bang_closes_it() {
    let mut s = Scene::new("buffers-close-dirty", Some("a"));
    let a = s.draft("0");
    s.go("x:bd<CR>");
    assert_eq!(
        said(&mut s.tui),
        "E89: No write since last change for buffer 0 \
             (add ! to override)"
    );
    s.go(":e b<CR>:bd 0<CR>");
    assert_eq!(
        said(&mut s.tui),
        "E89: No write since last change for buffer 0 \
             (add ! to override)"
    );
    s.go(":bd! 0<CR>");
    assert_eq!(ops::info(&s.host.own(), a), Err(Error::Invalid));
    assert_eq!(lines(&mut s.tui), ["ghi", "jkl"]);
    let b = s.draft("1");
    s.go("x:bd!<CR>");
    s.tui.tick().unwrap();
    assert_eq!(ops::info(&s.host.own(), b), Err(Error::Invalid));
    assert_eq!(lines(&mut s.tui), [""]);
}

#[test]
fn bd_of_the_last_buffer_leaves_a_vim_over_nothing() {
    let mut s = Scene::new("buffers-close-last", Some("a"));
    let a = s.draft("0");
    s.go(":bd<CR>");
    s.tui.tick().unwrap();
    assert_eq!(ops::info(&s.host.own(), a), Err(Error::Invalid));
    assert_eq!(lines(&mut s.tui), [""]);
    assert!(status(&mut s.tui).starts_with("[No Name]"));
    s.go("ix<Esc>");
    assert_eq!(lines(&mut s.tui), ["x"]);
}

#[test]
fn bd_drops_the_place_and_the_jumps_of_what_it_closed() {
    let mut s = Scene::new("buffers-forget", Some("a"));
    s.go(":e b<CR>jlma");
    let b = s.draft("1");
    assert_eq!(b, s.preload("b"));
    assert!(cat(&s.host, "marks").lines().any(|row| row == "a 2 1"));
    s.go(":e c<CR>");
    assert!(cat(&s.host, "jumps").lines().any(|row| row.ends_with("/b")));
    s.go(":bd 1<CR>");
    assert_eq!(ops::info(&s.host.own(), b), Err(Error::Invalid));
    assert!(!cat(&s.host, "jumps").lines().any(|row| row.ends_with("/b")));
    s.go("<C-^>");
    assert_eq!(lines(&mut s.tui), ["ghi", "jkl"]);
    assert_eq!(s.editor.engine().position(), (0, 0));
    assert!(!cat(&s.host, "marks").lines().any(|row| row.starts_with("a ")));
    s.go("jlma");
    assert!(cat(&s.host, "marks").lines().any(|row| row == "a 2 1"));
    s.go(":bd<CR>");
    s.tui.tick().unwrap();
    assert_eq!(lines(&mut s.tui), ["mno", "pqr"]);
    assert!(!cat(&s.host, "jumps").lines().any(|row| row.ends_with("/b")));
    s.go("<C-^>");
    assert_eq!(lines(&mut s.tui), ["ghi", "jkl"]);
    assert_eq!(s.editor.engine().position(), (0, 0));
    assert!(!cat(&s.host, "marks").lines().any(|row| row.starts_with("a ")));
}

#[test]
fn bd_bang_of_a_stale_draft_closes_it() {
    let mut s = Scene::new("buffers-close-stale", Some("a"));
    let a = s.draft("0");
    s.go("A!");
    let mut peer = opened(
        &s.rig.host(),
        common::one(Editor::new()),
        Clip::none(),
        "",
        Some("a"),
        None,
    )
    .unwrap();
    typed(&mut peer, "Oremote<Esc>");
    assert_eq!(lines(&mut peer), ["remote", "abc", "def"]);
    assert_eq!(s.server("a"), b"remote\nabc\ndef\n");
    s.go("<Esc>");
    assert_eq!(said(&mut s.tui), W12);
    assert_eq!(lines(&mut s.tui), ["abc!", "def"]);
    s.go(":bd<CR>");
    assert_eq!(
        said(&mut s.tui),
        "E89: No write since last change for buffer 0 \
             (add ! to override)"
    );
    s.go(":bd!<CR>");
    s.tui.tick().unwrap();
    assert_eq!(ops::info(&s.host.own(), a), Err(Error::Invalid));
    assert_eq!(lines(&mut s.tui), [""]);
    assert_eq!(lines(&mut peer), ["remote", "abc", "def"]);
}

#[test]
fn buffers_lists_a_line_a_buffer_and_the_row_joins_them() {
    let mut s = Scene::new("buffers-list", Some("a"));
    s.go(":e b<CR>x:buffers<CR>");
    assert_eq!(said(&mut s.tui), "0 #  a  1 %+ b");
    let ctx = s.host.own();
    let rd = s.rd("/self/vim/bin/buffers");
    let out = ops::call_by_name(&ctx, rd, "run", Vec::new()).unwrap();
    let output: Rd = reovim_sdk::decode(&out).unwrap();
    assert_eq!(ops::bytes(&ctx, output).unwrap(), b"0 #  a\n1 %+ b\n");
}

#[test]
fn missing_edit_anchors_its_name_before_a_later_cd() {
    let mut s = Scene::new("buffers-missing-cwd", Some("a"));
    s.go(":cd sub<CR>:e draft<CR>");
    assert_eq!(lines(&mut s.tui), [""]);
    assert!(!s.rig.dir().join("sub/draft").exists());
    s.go("ix<Esc>:cd ../elsewhere<CR>:w<CR>");
    assert_eq!(s.rig.disk("sub/draft"), b"x\n");
    assert!(!s.rig.dir().join("elsewhere/draft").exists());
}

#[test]
fn missing_edit_normalizes_absolute_dot_and_missing_parent_paths() {
    for (path, target) in [
        ("/absolute", "absolute"),
        ("sub/../relative", "relative"),
        ("sub/./missing/deep", "sub/missing/deep"),
    ] {
        let mut s = Scene::new("buffers-missing-normalized", Some("a"));
        s.go(&format!(":e {path}<CR>"));
        assert_eq!(lines(&mut s.tui), [""]);
        assert!(!s.rig.dir().join(target).exists());
        assert!(!s.rig.dir().join("sub/missing").exists());
        s.go("ix<Esc>:w<CR>");
        assert_eq!(s.rig.disk(target), b"x\n");
    }
}

#[test]
fn missing_edit_refuses_mirror_escape_alias_and_file_as_parent() {
    let mut s = Scene::new("buffers-invalid-missing", Some("a"));
    for path in ["../escape", "/../escape", "a/new", "rd:999999999/new"] {
        s.go(&format!(":e {path}<CR>"));
        assert_eq!(lines(&mut s.tui), ["abc", "def"]);
        assert!(status(&mut s.tui).starts_with("a  "));
        assert_ne!(said(&mut s.tui), "");
        assert!(!s.rig.dir().join("escape").exists());
        assert!(!s.rig.dir().join("rd:999999999").exists());
    }
}

#[test]
fn bd_closes_current_unnamed_and_named_missing_drafts() {
    for file in [None, Some("missing")] {
        let mut s = Scene::new("buffers-close-source-less", file);
        let draft = s.draft("0");
        assert!(matches!(
            ops::lookup(&s.host.own(), draft, "source"),
            Err(Error::Gone | Error::Unsupported)
        ));
        s.go("ix<Esc>:bd<CR>");
        assert_eq!(
            said(&mut s.tui),
            "E89: No write since last change for buffer 0 \
             (add ! to override)"
        );
        s.go(":bd!<CR>");
        s.tui.tick().unwrap();
        assert_eq!(ops::info(&s.host.own(), draft), Err(Error::Invalid));
        assert_eq!(lines(&mut s.tui), [""]);
        assert!(!s.rig.dir().join("missing").exists());
        let new = s.draft("1");
        s.go(":bd<CR>");
        s.tui.tick().unwrap();
        assert_eq!(ops::info(&s.host.own(), new), Err(Error::Invalid));
        assert_eq!(lines(&mut s.tui), [""]);
    }
}

#[test]
fn write_all_skips_unchanged_unnamed_and_missing_targets() {
    for file in [None, Some("missing")] {
        let mut s = Scene::new("buffers-write-unchanged", file);
        s.go(":wall<CR>");
        assert_eq!(said(&mut s.tui), "");
        assert!(!s.rig.dir().join("missing").exists());
        assert_eq!(s.go(":wqa<CR>"), Step::Quit(0));
        assert!(!s.rig.dir().join("missing").exists());
    }
}

#[test]
fn write_all_continues_after_unnamed_failure_in_either_draft_order() {
    for named_first in [false, true] {
        let file = named_first.then_some("a");
        let mut s = Scene::new("buffers-write-mixed", file);
        let good = s.preload("a");
        s.modified(good, b"saved");
        if named_first {
            s.scratch();
        }
        s.go("ix<Esc>");
        let bad = if named_first { "1" } else { "0" };
        assert_ne!(s.go(":wqa<CR>"), Step::Quit(0));
        assert_eq!(
            said(&mut s.tui),
            format!("E141: No file name for buffer {bad}")
        );
        assert_eq!(s.rig.disk("a"), b"saved\n");
        assert_eq!(lines(&mut s.tui), ["x"]);
    }
}

#[test]
fn write_all_does_not_sweep_departed_scratch_drafts() {
    let mut s = Scene::new("buffers-write-hidden-scratch", None);
    let hidden = s.draft("0");
    s.go("ix<Esc>");
    s.scratch();
    let good = s.preload("b");
    s.modified(good, b"saved");
    s.go(":wall<CR>");
    assert_eq!(said(&mut s.tui), "");
    assert_eq!(s.rig.disk("b"), b"saved\n");
    assert_eq!(ops::bytes(&s.host.own(), hidden).unwrap(), b"x");
}

#[test]
fn write_all_continues_after_its_own_stale_draft() {
    let mut s = Scene::new("buffers-write-own-stale", Some("a"));
    let b = s.preload("b");
    s.modified(b, b"saved");
    s.go("A!");
    let mut peer = opened(
        &s.rig.host(),
        common::one(Editor::new()),
        Clip::none(),
        "",
        Some("a"),
        None,
    )
    .unwrap();
    typed(&mut peer, "Oremote<Esc>");
    assert_eq!(lines(&mut peer), ["remote", "abc", "def"]);
    assert_eq!(s.server("a"), b"remote\nabc\ndef\n");
    s.go("<Esc>");
    assert_eq!(said(&mut s.tui), W12);
    assert_eq!(lines(&mut s.tui), ["abc!", "def"]);
    assert_ne!(s.go(":wqa<CR>"), Step::Quit(0));
    assert!(said(&mut s.tui).starts_with("W12:"));
    assert_eq!(s.rig.disk("b"), b"saved\n");
    assert_eq!(s.rig.disk("a"), b"abc\ndef\n");
}

#[test]
fn write_all_continues_past_another_stale_draft() {
    let mut s = Scene::new("buffers-write-other-stale", Some("a"));
    let b = s.preload("b");
    let c = s.preload("c");
    s.modified(b, b"pending");
    s.modified(c, b"saved");
    let mut peer = opened(
        &s.rig.host(),
        common::one(Editor::new()),
        Clip::none(),
        "",
        Some("b"),
        None,
    )
    .unwrap();
    typed(&mut peer, "A!<Esc>");
    assert_eq!(lines(&mut peer), ["ghi!", "jkl"]);
    assert_eq!(s.server("b"), b"ghi!\njkl\n");
    assert_eq!(ops::bytes(&s.host.own(), b).unwrap(), b"pending");
    s.go("x");
    assert_ne!(s.go(":wqa<CR>"), Step::Quit(0));
    assert_eq!(
        said(&mut s.tui),
        "W12: buffer 1 changed in another Client and here"
    );
    assert_eq!(s.rig.disk("a"), b"bc\ndef\n");
    assert_eq!(s.rig.disk("c"), b"saved\n");
    assert_eq!(s.rig.disk("b"), b"ghi\njkl\n");
}

#[test]
fn fixture_scratch_and_stale_drafts_have_the_required_lifecycle() {
    let mut s = Scene::new("buffers-fixture-proof", Some("a"));
    s.go("A!");
    let mut peer = opened(
        &s.rig.host(),
        common::one(Editor::new()),
        Clip::none(),
        "",
        Some("a"),
        None,
    )
    .unwrap();
    typed(&mut peer, "Oremote<Esc>");
    assert_eq!(lines(&mut peer), ["remote", "abc", "def"]);
    assert_eq!(s.server("a"), b"remote\nabc\ndef\n");
    s.go("<Esc>");
    assert_eq!(said(&mut s.tui), W12);
    assert_eq!(lines(&mut s.tui), ["abc!", "def"]);
    let mut observer = s.rig.client();
    let source = observer.resolve("/module/fs/root/a").unwrap();
    let buffer =
        observer.resolve(&format!("/module/text/{}", source.0)).unwrap();
    assert_eq!(observer.read_all(buffer).unwrap(), b"remote\nabc\ndef\n");
    s.scratch();
    let scratch = s.draft("1");
    assert!(matches!(
        ops::lookup(&s.host.own(), scratch, "source"),
        Err(Error::Gone | Error::Unsupported)
    ));
    assert_eq!(lines(&mut s.tui), [""]);
    s.go("ix<Esc>");
    assert_eq!(lines(&mut s.tui), ["x"]);
    assert_eq!(observer.read_all(buffer).unwrap(), b"remote\nabc\ndef\n");
}

#[test]
fn full_ask_fixture_matches_the_existing_declared_api() {
    let mut s = Scene::new("buffers-ask-fixture", Some("a"));
    let ctx = s.host.own();
    let region = Region::of(&ctx).unwrap();
    let target = s.rd("/self/dev/open");
    let ask = reovim_client_api::open::Ask {
        target: NONE,
        plugin: String::new(),
        name: String::from("sub/new"),
        line: 2,
    };
    ops::call_by_name(&ctx, target, "open", reovim_sdk::encode(&ask)).unwrap();
    s.tui.tick().unwrap();
    let delivered = region.ask(&ctx).unwrap();
    assert_eq!(delivered.target, NONE);
    assert_eq!(delivered.name, "sub/new");
    assert_eq!(delivered.line, 2);
    assert_eq!(lines(&mut s.tui), [""]);
    assert!(!s.rig.dir().join("sub/new").exists());
}

#[test]
fn write_all_saves_a_changed_named_missing_current_draft() {
    let mut s = Scene::new("buffers-write-new-current", Some("missing"));
    assert!(!s.rig.dir().join("missing").exists());
    s.go("ix<Esc>");
    assert_eq!(s.go(":wqa<CR>"), Step::Quit(0));
    assert_eq!(s.rig.disk("missing"), b"x\n");
}

#[test]
fn a_source_less_insert_keeps_its_bytes_without_implicit_newline() {
    let mut s = Scene::new("buffers-scratch-bytes", None);
    let draft = s.draft("0");
    s.go("ix<Esc>");
    assert_eq!(ops::bytes(&s.host.own(), draft).unwrap(), b"x");
}

#[test]
fn repeated_leading_slashes_still_mean_the_mirror_root() {
    let mut s = Scene::new("buffers-repeated-root", Some("a"));
    s.go(":e //absolute<CR>");
    assert_eq!(lines(&mut s.tui), [""]);
    assert!(!s.rig.dir().join("absolute").exists());
    s.go("ix<Esc>:w<CR>");
    assert_eq!(s.rig.disk("absolute"), b"x\n");
}
