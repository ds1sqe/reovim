//! `bind`, the leader, and the keymap and the options as cells, over
//! the tui and the rig.
//!
//! Every bound key is a case here and none is in the corpus: nvim
//! reads no rc under `--clean` and a mapping there would have to be
//! typed `:nnoremap`, which is not `bind`. What the corpus carries is
//! the unbound Alt alone, which nvim and we type alike.

mod common;

use std::rc::Rc;

use common::cat;
use reovim_client::Host;
use reovim_client::dev::{Clip, Table, Want};
use reovim_client_tui::{Step, Tui};
use reovim_kernel_interface::Rd;
use reovim_plugin_vim::bins::bound;
use reovim_plugin_vim_host::Editor;
use reovim_rig::{Rig, Seen, Stub, lines, listed, opened, said, status, typed};
use reovim_sdk::walk::ROOT;
use reovim_sdk::{cell, ops};

/// An editor over `note`, whose text is `text`, with one stub for
/// every region Vim hands on — the cli among them — and the Host its
/// cells are read through.
fn over(name: &str, text: &[u8]) -> (Rig, Tui, Rc<Host>, Seen) {
    let rig = Rig::new(name);
    rig.file("note", text);
    rig.file("other", b"elsewhere\n");
    let host = rig.host();
    let vim = Editor::new();
    let seen = Seen::default();
    let held = seen.clone();
    let table: Table = Box::new(move |want| match want {
        Want::Cell => Some((String::from("vim"), vim.booted())),
        _ => Some((
            String::from("cli"),
            Stub::typing(
                &held,
                "$ ",
                Box::new(|ctx, region, _| region.backs(ctx)),
            ),
        )),
    });
    let tui =
        opened(&host, table, Clip::none(), "", Some("note"), None).unwrap();
    (rig, tui, host, seen)
}

/// The three lines every scene here starts from.
fn three(test: &str) -> (Rig, Tui, Rc<Host>, Seen) {
    over(test, b"one\ntwo\nthree\n")
}

#[test]
fn a_bound_key_runs_its_line() {
    let (rig, mut tui, _host, _seen) = three("bind-runs");
    assert_eq!(typed(&mut tui, ":bind gw write<CR>"), Step::Changed);
    assert_eq!(said(&mut tui), "");
    typed(&mut tui, "x");
    assert_eq!(rig.disk("note"), b"one\ntwo\nthree\n", "nothing written yet");
    typed(&mut tui, "gw");
    assert_eq!(rig.disk("note"), b"ne\ntwo\nthree\n");

    // The line went through the `:` door and not over the `:` line:
    // the history holds what was typed and nothing of the binding.
    typed(&mut tui, ":<Up>");
    assert_eq!(said(&mut tui), ":bind gw write");
    typed(&mut tui, "<Esc>");

    // Nor is it `@:`'s, which is still the last line a person typed:
    // `@:` runs the `bind` again and writes nothing.
    typed(&mut tui, "x@:");
    assert_eq!(said(&mut tui), "");
    assert_eq!(rig.disk("note"), b"ne\ntwo\nthree\n", "`@:` is not write");
}

#[test]
fn bind_fills_two_fields_and_w_is_unchanged() {
    let (rig, mut tui, _host, _seen) = three("bind-two-fields");
    typed(&mut tui, ":bind <lt>Leader>w write my file<CR>");
    assert_eq!(said(&mut tui), "");
    typed(&mut tui, "x\\w");
    assert_eq!(rig.disk("my file"), b"ne\ntwo\nthree\n");

    // A name with one positional still takes the rest of the line
    // whole, spaces and all.
    typed(&mut tui, ":w other file<CR>");
    assert_eq!(rig.disk("other file"), b"ne\ntwo\nthree\n");
}

#[test]
fn bind_with_no_line_unbinds() {
    let (rig, mut tui, _host, _seen) = three("bind-unbind");
    typed(&mut tui, ":bind gw write<CR>");
    typed(&mut tui, "x");
    typed(&mut tui, ":bind gw<CR>");
    assert_eq!(said(&mut tui), "");
    typed(&mut tui, "gw");
    assert_eq!(rig.disk("note"), b"one\ntwo\nthree\n", "no binding ran");
    typed(&mut tui, ":bind gw<CR>");
    assert_eq!(said(&mut tui), "E31: No such mapping");
}

#[test]
fn a_bad_notation_is_said_and_nothing_bound() {
    let (_rig, mut tui, host, _seen) = three("bind-bad-notation");
    assert_eq!(
        cat(&host, "keymap"),
        "bind <Leader>y point > /self/dev/clipboard\nbind <M-x> cli\n"
    );
    typed(&mut tui, ":bind <lt>Nope> cli<CR>");
    assert_eq!(said(&mut tui), "E474: Invalid argument: <Nope>");
    assert_eq!(
        cat(&host, "keymap"),
        "bind <Leader>y point > /self/dev/clipboard\nbind <M-x> cli\n",
        "as it was"
    );
}

#[test]
fn help_bind_says_its_form() {
    let (_rig, mut tui, host, _seen) = three("bind-help");
    typed(&mut tui, ":help bind<CR>");
    assert_eq!(said(&mut tui), "bind [keys] [line]");
    // 25 full names and every abbreviation nvim takes for one of
    // them, plus `explore` and `cli`'s own — reovim's, nvim naming
    // neither — each an exact node of its own under `bin`.
    assert_eq!(listed(&host, "/self/vim/bin").len(), 105);

    // `b` is one of `buffer`'s own abbreviations, an exact name under
    // `bin` and not a prefix resolved at the call.
    typed(&mut tui, ":e other<CR>:b 0<CR>");
    assert_eq!(said(&mut tui), "");
    assert_eq!(lines(&mut tui)[..3], ["one", "two", "three"]);
}

#[test]
fn leader_before_and_after_a_set() {
    let (rig, mut tui, host, _seen) = three("bind-leader");
    typed(&mut tui, ":bind <lt>Leader>w write my file<CR>");
    typed(&mut tui, ":set leader=<lt>Space><CR>");
    assert_eq!(said(&mut tui), "");
    assert_eq!(cat(&host, "options/leader"), "<Space>\n");

    // The binding was made under `\` and answers the leader as it is
    // now: the leader is read at the key.
    typed(&mut tui, "x w");
    assert_eq!(rig.disk("my file"), b"ne\ntwo\nthree\n");

    typed(&mut tui, ":set leader=<lt>Nope><CR>");
    assert_eq!(said(&mut tui), "E474: Invalid argument: leader=<Nope>");
    assert_eq!(cat(&host, "options/leader"), "<Space>\n", "still the space");

    // A leader is a value, and a switch is not: `leader` with none,
    // `noleader`, and a value for a switch are each E474, while a
    // name that is no option at all is E518 with the whole argument.
    for (arg, message) in [
        ("leader", "E474: Invalid argument: leader"),
        ("noleader", "E474: Invalid argument: noleader"),
        ("leader=", "E474: Invalid argument: leader="),
        ("ic=1", "E474: Invalid argument: ic=1"),
        ("noic=1", "E474: Invalid argument: noic=1"),
        ("nosuch=1", "E518: Unknown option: nosuch=1"),
    ] {
        typed(&mut tui, &format!(":set {arg}<CR>"));
        assert_eq!(said(&mut tui), message, "{arg}");
    }
    assert_eq!(cat(&host, "options/leader"), "<Space>\n", "still the space");
}

#[test]
fn two_keys_that_read_the_same_answer_in_the_maps_order() {
    let (rig, mut tui, host, _seen) = three("bind-leader-order");
    typed(&mut tui, ":bind <lt>Leader>w write<CR>");
    typed(&mut tui, ":set leader=,<CR>");
    typed(&mut tui, ":bind \\w quit<CR>");
    typed(&mut tui, ":set leader=\\<CR>");
    assert_eq!(
        cat(&host, "keymap"),
        "bind <Leader>w write\n\
         bind <Leader>y point > /self/dev/clipboard\n\
         bind <M-x> cli\n\
         bind \\w quit\n"
    );

    // Both read `\w` now; the first in the map's order answers, so
    // the file is written and the editor is still here.
    typed(&mut tui, "x\\w");
    assert_eq!(rig.disk("note"), b"ne\ntwo\nthree\n");

    // One unbind takes both, and there is nothing left to take.
    typed(&mut tui, ":bind \\w<CR>");
    assert_eq!(said(&mut tui), "");
    assert_eq!(
        cat(&host, "keymap"),
        "bind <Leader>y point > /self/dev/clipboard\nbind <M-x> cli\n"
    );
    typed(&mut tui, ":bind \\w<CR>");
    assert_eq!(said(&mut tui), "E31: No such mapping");
}

#[test]
fn the_five_option_cells() {
    let (_rig, mut tui, host, _seen) = three("bind-options");
    let booted = reovim_module_coreutils::boot("");
    host.load("coreutils", booted.module, booted.drivers).unwrap();

    assert_eq!(
        listed(&host, "/self/vim"),
        [
            "view",
            "mode",
            "pending",
            "registers",
            "marks",
            "jumps",
            "recording",
            "options",
            "keymap",
            "point",
            "search",
            "bin"
        ]
    );
    assert_eq!(
        listed(&host, "/self/vim/options"),
        ["ignorecase", "smartcase", "hlsearch", "vi", "leader"]
    );

    assert_eq!(cat(&host, "options/ignorecase"), "0\n");
    typed(&mut tui, ":set ic<CR>");
    assert_eq!(cat(&host, "options/ignorecase"), "1\n");
    assert_eq!(cat(&host, "options/smartcase"), "0\n");
    // `hlsearch` is off until it is asked for, which is the engine's
    // own default and what the cell is a reading of.
    assert_eq!(cat(&host, "options/hlsearch"), "0\n");
    typed(&mut tui, ":set hls<CR>");
    assert_eq!(cat(&host, "options/hlsearch"), "1\n");
    assert_eq!(cat(&host, "options/vi"), "0\n");
    assert_eq!(cat(&host, "options/leader"), "\\\n");
    typed(&mut tui, ":set leader=<lt>Space><CR>");
    assert_eq!(cat(&host, "options/leader"), "<Space>\n");

    // No cell is a writer: `set` is, and a redirect into one is
    // refused by the tree with the option left where it stood.
    typed(&mut tui, ":cat note > /self/vim/options/ignorecase<CR>");
    assert_ne!(said(&mut tui), "", "the redirect was refused");
    assert_eq!(cat(&host, "options/ignorecase"), "1\n");
}

#[test]
fn the_keymap_cell_is_a_script_that_makes_it() {
    let (_rig, mut tui, host, _seen) = three("bind-keymap-cell");
    typed(&mut tui, ":bind <lt>Leader>w write<CR>");
    typed(&mut tui, ":bind <lt>M-x><CR>");
    let script = cat(&host, "keymap");
    assert_eq!(
        script,
        "bind <Leader>w write\n\
         bind <Leader>y point > /self/dev/clipboard\n",
        "an unbound default is no line"
    );

    // The text run as lines makes the bindings it lists on a second
    // editor, and claims no more: that one has its own default.
    let second = Rig::new("bind-keymap-cell-again");
    second.file("note", b"one\ntwo\nthree\n");
    let host = second.host();
    let vim = Editor::with_init(script);
    let table: Table =
        Box::new(move |_| Some((String::from("vim"), vim.booted())));
    let mut tui =
        opened(&host, table, Clip::none(), "", Some("note"), None).unwrap();
    typed(&mut tui, "x\\w");
    assert_eq!(second.disk("note"), b"ne\ntwo\nthree\n");
    assert_eq!(
        cat(&host, "keymap"),
        "bind <Leader>w write\n\
         bind <Leader>y point > /self/dev/clipboard\n\
         bind <M-x> cli\n"
    );
}

#[test]
fn a_bound_alt_in_visual_runs_its_line_and_leaves_visual() {
    let (rig, mut tui, host, _seen) = three("bind-alt-visual");
    typed(&mut tui, ":bind <lt>M-w> write<CR>");
    typed(&mut tui, "x");
    typed(&mut tui, "v<M-w>");
    assert_eq!(rig.disk("note"), b"ne\ntwo\nthree\n");
    assert_eq!(cat(&host, "mode"), "normal\n", "visual was left first");
    // No `<Esc>` and no `w` were let go, so the cursor never moved.
    assert!(status(&mut tui).starts_with("note  1:1"), "{}", status(&mut tui));
}

#[test]
fn a_bound_line_in_visual_finds_no_selection() {
    let (_rig, mut tui, host, _seen) = over("bind-visual-edit", b"abcdef\n");
    typed(&mut tui, ":bind gs %s/bcdef//<CR>");
    typed(&mut tui, "$vgs");
    assert_eq!(lines(&mut tui)[..1], ["a"]);
    assert_eq!(cat(&host, "mode"), "normal\n");

    // The selection is gone with visual, so nothing slices past the
    // end of a line the line itself edited.
    typed(&mut tui, "d");
    assert_eq!(lines(&mut tui)[..1], ["a"]);
    assert_eq!(cat(&host, "mode"), "normal\n");
}

#[test]
fn a_bound_line_from_visual_carries_the_range() {
    let (_rig, mut tui, host, _seen) = three("bind-visual-range");
    typed(&mut tui, ":bind gd delete<CR>");

    // A line of its own address keeps it, as `:'<,'>%s` does in nvim.
    typed(&mut tui, ":bind gp %point<CR>");
    typed(&mut tui, "Vjgp");
    assert_eq!(said(&mut tui), "/note:1-3", "the line's own `%` won");

    // nvim runs a `:` line from visual over `'<,'>`, typed or mapped:
    // the binding leaves visual and the line it runs sees the marks.
    typed(&mut tui, "ggVjgd");
    assert_eq!(lines(&mut tui)[..1], ["three"], "two lines went");
    assert_eq!(cat(&host, "mode"), "normal\n");
}

#[test]
fn alt_x_opens_the_cli_buffer() {
    let (_rig, mut tui, _host, seen) = three("bind-alt-x");
    assert_eq!(typed(&mut tui, "<M-x>"), Step::Changed);
    assert_eq!(said(&mut tui), "$ ", "the cli was asked for");
    assert_eq!(seen.borrow().len(), 1);

    // The map is read in normal and visual alone, so an insert's
    // `<M-x>` is the Esc that ends it and then the char.
    typed(&mut tui, " ");
    typed(&mut tui, "i<M-x>");
    assert_eq!(lines(&mut tui)[..1], ["ne"]);
    assert_eq!(seen.borrow().len(), 1, "no second ask");
}

#[test]
fn a_bound_alt_behind_a_count_or_a_register_runs_alone() {
    let (_rig, mut tui, host, seen) = three("bind-alt-count");

    // A count before it is dropped and no part of the line.
    assert_eq!(typed(&mut tui, "3<M-x>"), Step::Changed);
    assert_eq!(said(&mut tui), "$ ");
    typed(&mut tui, " ");
    assert_eq!(cat(&host, "pending"), "");
    assert_eq!(lines(&mut tui)[..3], ["one", "two", "three"], "text whole");

    // A register before it, the same.
    assert_eq!(typed(&mut tui, "\"a<M-x>"), Step::Changed);
    assert_eq!(said(&mut tui), "$ ");
    typed(&mut tui, " ");
    assert_eq!(cat(&host, "pending"), "");
    assert_eq!(seen.borrow().len(), 2);

    // An operator waiting is not a count: the Alt is `<Esc>` then the
    // char, which drops the operator and takes one char.
    typed(&mut tui, "d<M-x>");
    assert_eq!(lines(&mut tui)[..1], ["ne"]);
    assert_eq!(seen.borrow().len(), 2, "no third ask");
}

#[test]
fn a_count_or_a_register_before_a_bound_key_is_not_the_bindings() {
    let (rig, mut tui, host, _seen) = three("bind-counted");
    typed(&mut tui, ":bind \\w write<CR>");
    typed(&mut tui, "x");

    // nvim hands its count to the right side and runs the mapping
    // under the register; here neither key is the binding's, so the
    // keys are themselves and the file is not written.
    typed(&mut tui, "3\\w");
    assert_eq!(rig.disk("note"), b"one\ntwo\nthree\n", "`3\\w` is not `\\w`");
    typed(&mut tui, "\"a\\w");
    assert_eq!(rig.disk("note"), b"one\ntwo\nthree\n", "`\"a\\w` is not `\\w`");

    // A bound Alt is the one exception, and what stood before it is
    // dropped: the `3` does not reach the `x` after it, and the `"a`
    // does not take the yank.
    typed(&mut tui, ":bind <lt>M-n> nohlsearch<CR>");
    typed(&mut tui, "gg3<M-n>x");
    assert_eq!(lines(&mut tui)[..1], ["e"], "one char, not three");
    assert_eq!(cat(&host, "pending"), "");
    typed(&mut tui, "\"b<M-n>yy");
    assert_eq!(cat(&host, "registers/b"), "", "the register was dropped");
    assert_eq!(cat(&host, "registers/\""), "e\n");
}

#[test]
fn a_bound_alt_a_prefix_let_go_runs_alone() {
    let (_rig, mut tui, _host, seen) = three("bind-alt-past");
    typed(&mut tui, ":bind ,w write<CR>");
    assert_eq!(typed(&mut tui, ",<M-x>"), Step::Changed);
    assert_eq!(said(&mut tui), "$ ");
    typed(&mut tui, " ");
    assert_eq!(seen.borrow().len(), 1);
    assert_eq!(lines(&mut tui)[..3], ["one", "two", "three"], "text whole");
}

#[test]
fn a_released_alt_is_drained_to_its_end() {
    let (_rig, mut tui, _host, _seen) =
        over("bind-alt-drain", b"abc\ndef\nghi\n");
    typed(&mut tui, ":bind \\w write<CR>");
    typed(&mut tui, "\\<M-j>x");
    assert_eq!(lines(&mut tui)[..3], ["abc", "ef", "ghi"]);
}

#[test]
fn a_macro_runs_a_binding() {
    let (rig, mut tui, _host, _seen) = three("bind-macro");
    typed(&mut tui, ":bind \\w write<CR>");
    typed(&mut tui, "xqa\\wq");
    assert_eq!(rig.disk("note"), b"ne\ntwo\nthree\n");
    typed(&mut tui, "x@a");
    assert_eq!(rig.disk("note"), b"e\ntwo\nthree\n");
}

#[test]
fn a_macro_over_a_bound_prefix_is_one_run() {
    let (_rig, mut tui, _host, _seen) =
        over("bind-macro-prefix", b"1\n2\n3\n4\n5\n6\n7\n8\n");
    typed(&mut tui, ":bind dx write<CR>");
    typed(&mut tui, "qaddjddq");
    assert_eq!(lines(&mut tui)[..6], ["2", "4", "5", "6", "7", "8"]);
    typed(&mut tui, "@a");
    assert_eq!(lines(&mut tui)[..4], ["2", "5", "7", "8"]);

    // One `u` restores the whole run: every edit of it joined the one
    // revision, the keys a failed prefix let go among them.
    typed(&mut tui, "u");
    assert_eq!(lines(&mut tui)[..6], ["2", "4", "5", "6", "7", "8"]);
}

#[test]
fn a_recursive_macro_over_a_bound_prefix_ends_at_the_budget() {
    let (_rig, mut tui, _host, _seen) = over("bind-macro-budget", b"\n");
    typed(&mut tui, ":bind dx write<CR>");
    typed(&mut tui, "qaq");
    typed(&mut tui, "qadl@aq");
    typed(&mut tui, "@a");
    assert_eq!(said(&mut tui), "@: stopped after 100000 keys");
}

#[test]
fn exact_with_longer_in_a_macro_waits_for_a_typed_key() {
    let (rig, mut tui, _host, _seen) = three("bind-macro-longer");
    typed(&mut tui, ":bind \\w write<CR>");
    typed(&mut tui, "qa\\wq");
    assert_eq!(rig.disk("note"), b"one\ntwo\nthree\n");
    typed(&mut tui, ":bind \\wz quit<CR>");
    typed(&mut tui, "x");

    // The fed keys end at a Longer, and nothing settles them: no
    // timeout reaches a Plugin.
    typed(&mut tui, "@a");
    assert_eq!(rig.disk("note"), b"one\ntwo\nthree\n", "held, not run");

    // The next typed key settles it, and is let go after the line.
    typed(&mut tui, "j");
    assert_eq!(rig.disk("note"), b"ne\ntwo\nthree\n");
}

#[test]
fn a_bell_drops_released_macro_keys_but_keeps_typed_keys() {
    let (_rig, mut tui, _host, _seen) = over("bind-bell", b"abc\nh\n");
    typed(&mut tui, ":bind hz nohlsearch<CR>");

    // `hx` typed at column 0: `h` rings, and the typed `x` it let go
    // is kept and takes its char.
    typed(&mut tui, "qahxq");
    assert_eq!(lines(&mut tui)[..1], ["bc"]);
    typed(&mut tui, "u");
    assert_eq!(lines(&mut tui)[..1], ["abc"]);

    // Fed, the same two keys are the macro's: the bell the `h` rang
    // drops the `x` with the rest of that run.
    typed(&mut tui, "@a");
    assert_eq!(lines(&mut tui)[..1], ["abc"]);

    // A key typed after a cancelled run is not dropped with it.
    typed(&mut tui, "@ax");
    assert_eq!(lines(&mut tui)[..1], ["bc"]);
    typed(&mut tui, "u");

    // A prefix a macro left half held, finished by a typed key: each
    // goes back in the origin it was taken with, so the fed `h` rings
    // and the typed `x` after it still stands.
    typed(&mut tui, "j\"bylgg@bx");
    assert_eq!(lines(&mut tui)[..1], ["bc"]);
}

#[test]
fn an_exact_prefix_with_longer_runs_on_mismatch() {
    let (rig, mut tui, host, _seen) = three("bind-longer-mismatch");
    typed(&mut tui, ":bind ,w write<CR>");
    typed(&mut tui, ":bind ,wab quit<CR>");
    typed(&mut tui, "x");
    typed(&mut tui, ",wax");
    assert_eq!(rig.disk("note"), b"ne\ntwo\nthree\n", "`,w` ran");
    assert_eq!(cat(&host, "mode"), "insert\n");
    assert_eq!(lines(&mut tui)[..1], ["nxe"], "`a` then `x`, in order");
}

#[test]
fn a_budget_ending_at_a_prefix_is_not_finished_by_a_typed_key() {
    let (_rig, mut tui, _host, _seen) =
        over("bind-budget-prefix", b"alpha beta gamma\n");
    typed(&mut tui, ":bind ,w nohlsearch<CR>");

    // Nine fed keys a turn, and 100000 is one past a whole number of
    // them, so the run is cut with the `,` held and the `w` it wants
    // never fed.
    typed(&mut tui, "qa,w<Esc><Esc><Esc><Esc><Esc>@aq");
    typed(&mut tui, "@a");
    assert_eq!(said(&mut tui), "@: stopped after 100000 keys");

    // The typed key after it is itself: a macro that ended inside a
    // prefix cannot be finished by a person's next key.
    typed(&mut tui, "wx");
    assert_eq!(lines(&mut tui)[..1], ["alpha eta gamma"]);
}

#[test]
fn keys_with_a_blank_and_a_line_with_a_newline_are_refused() {
    let (_rig, _tui, host, _seen) = three("bind-blanks");
    let ctx = host.own();
    let rd = cell::resolve(&ctx, ROOT, "/self/vim/bin/bind").unwrap();
    let ran = |keys: &str, line: &str| {
        let args =
            bound::Args { keys: String::from(keys), line: String::from(line) };
        let out = ops::call_by_name(&ctx, rd, "run", reovim_sdk::encode(&args));
        let cell: Rd = reovim_sdk::decode(&out.unwrap()).unwrap();
        String::from_utf8(ops::bytes(&ctx, cell).unwrap()).unwrap()
    };

    // The cell is a script read back at blanks and at line ends, so
    // neither may be hidden inside a binding.
    assert_eq!(ran("", "write"), "E471: Argument required");
    assert_eq!(ran("a b", "write"), "E474: Invalid argument: a b");
    assert_eq!(
        ran("<Space>a", "write\nquit"),
        "E474: Invalid argument: write\nquit"
    );
    assert_eq!(
        cat(&host, "keymap"),
        "bind <Leader>y point > /self/dev/clipboard\nbind <M-x> cli\n",
        "as it was"
    );
    assert_eq!(ran("<Space>b", "write"), "");
    assert_eq!(
        cat(&host, "keymap"),
        "bind <Leader>y point > /self/dev/clipboard\n\
         bind <M-x> cli\n\
         bind <Space>b write\n"
    );
}

#[test]
fn a_bind_line_of_init_works() {
    let rig = Rig::new("bind-init");
    rig.file("note", b"one\ntwo\nthree\n");
    let host = rig.host();
    let vim = Editor::with_init(String::from(
        "set leader=<Space>\nbind <Leader>w write\n",
    ));
    let table: Table =
        Box::new(move |_| Some((String::from("vim"), vim.booted())));
    let mut tui =
        opened(&host, table, Clip::none(), "", Some("note"), None).unwrap();
    typed(&mut tui, "x w");
    assert_eq!(rig.disk("note"), b"ne\ntwo\nthree\n");
}
