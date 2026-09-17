//! Tab on the `:` line, and the line editor under the three prompts,
//! over the tui and the rig.
//!
//! The corpus cannot judge any of this: nvim leaves `:w` as `:w` and
//! lists `wall`, `while`, `wincmd`, which is the one divergence this
//! minor makes on purpose. So the rig judges it by what happened: the
//! status row after Tab, the file on disk after the expanded line ran,
//! and what a walk through the history put back on the line.

use reovim_client::Plugin;
use reovim_client_tui::{Step, Table, Tui, Want};
use reovim_plugin_vim::Vim;
use reovim_plugin_vim::notation::keys;
use reovim_rig::Rig;

/// The one editor a test drives.
type Editor = Tui<Box<dyn Plugin>>;

/// Vim for a cell; nothing else is opened here.
fn table() -> Table<Box<dyn Plugin>> {
    Box::new(|want| match want {
        Want::Cell => Some(Box::new(Vim::default()) as Box<dyn Plugin>),
        _ => None,
    })
}

/// An editor over `d/note`, started in `d`, with `d/alpha` beside it.
fn over(test: &str) -> (Rig, Editor) {
    let rig = Rig::new(test);
    std::fs::create_dir_all(rig.dir().join("d")).unwrap();
    rig.file("d/alpha", b"");
    rig.file("d/note", b"one\ntwo\n");
    let tui = Tui::open(rig.client(), "d", Some("d/note"), None, table());
    (rig, tui.unwrap())
}

/// Every key of `text` in Vim notation; what the last one did.
fn typed(tui: &mut Editor, text: &str) -> Step {
    let mut step = Step::Same;
    for key in keys(text).unwrap() {
        step = tui.key(key).unwrap();
    }
    step
}

/// Vim's own word of the status row: what follows the name, the
/// position and the revision, which the tui writes before it.
fn said(tui: &mut Editor) -> String {
    let status = tui.frame(80, 24).unwrap().status;
    status.split_once("   ").unwrap().1.into()
}

/// `text` typed, and the status row it left, the prompt still open.
fn said_after(tui: &mut Editor, text: &str) -> String {
    typed(tui, text);
    said(tui)
}

/// `text` typed on a fresh prompt, and the status row it left.
fn after(tui: &mut Editor, text: &str) -> String {
    typed(tui, text);
    let said = said(tui);
    typed(tui, "<Esc>");
    said
}

#[test]
fn one_candidate_is_the_word_expanded_and_its_help() {
    let (_rig, mut tui) = over("tab-one");

    // `q` is `quit` in both tables, so it is one candidate: the whole
    // name and a space on the line, then the help. A word that takes
    // nothing says nothing where the argument form would stand.
    assert_eq!(after(&mut tui, ":q<Tab>"), ":quit   quit; ! leaves the draft");
    assert_eq!(
        after(&mut tui, ":wr<Tab>"),
        ":write   [PATH]  the buffer, to PATH if one is given"
    );
    assert_eq!(
        after(&mut tui, ":nohl<Tab>"),
        ":nohlsearch   stop marking the last search"
    );

    // A name no table has offers nothing, and the line stands as it
    // was typed.
    assert_eq!(after(&mut tui, ":zz<Tab>"), ":zz");
    assert_eq!(after(&mut tui, ":w!<Tab>"), ":w!");

    // An argument does not complete yet; Tab past the word is
    // nothing at all.
    assert_eq!(after(&mut tui, ":write al<Tab>"), ":write al");
}

#[test]
fn several_candidates_are_a_wildmenu_tab_and_ctrl_p_walk() {
    let (_rig, mut tui) = over("tab-menu");
    let menu = ["write", "wq", "watch"];
    let shown = |at: usize| {
        let current = format!("[{}]", menu[at]);
        let rest = menu
            .into_iter()
            .enumerate()
            .map(|(i, name)| if i == at { current.as_str() } else { name });
        let listed: Vec<&str> = rest.collect();
        format!(":{}   {}", menu[at], listed.join("  "))
    };

    // Vim's `write` and `wq` before the cli's `watch`; the cli's own
    // `write`, which Vim's shadows, is not a second entry.
    assert_eq!(said_after(&mut tui, ":w<Tab>"), shown(0));
    assert_eq!(said_after(&mut tui, "<Tab>"), shown(1));
    assert_eq!(said_after(&mut tui, "<Tab>"), shown(2));
    assert_eq!(said_after(&mut tui, "<Tab>"), shown(0), "past the last");
    assert_eq!(said_after(&mut tui, "<C-p>"), shown(2), "before the first");
    assert_eq!(said_after(&mut tui, "<C-p>"), shown(1));
    typed(&mut tui, "<Esc>");

    // Any other key leaves the menu with the word it stood on, and is
    // itself: the `x` lands after the space the expansion left.
    assert_eq!(said_after(&mut tui, ":w<Tab>x"), ":write x");
    // Ctrl-P is nothing again once the menu is gone.
    assert_eq!(said_after(&mut tui, "<C-p>"), ":write x");
    typed(&mut tui, "<Esc>");
}

#[test]
fn the_expanded_line_runs_the_word_it_names() {
    let (rig, mut tui) = over("tab-runs");

    // `:w` Tab Enter is `:write`, which writes the draft.
    assert_eq!(typed(&mut tui, "ix<Esc>:w<Tab><CR>"), Step::Changed);
    assert_eq!(rig.disk("d/note"), b"xone\ntwo\n");

    // The wildmenu walked to another word runs that one: `watch` is
    // the cli's, and a target that is not there is the cli's own
    // sentence, not E492.
    typed(&mut tui, ":w<Tab><Tab><Tab>nothere<CR>");
    assert_eq!(said(&mut tui), "watch nothere: no such rd (nothere)");

    // `:q` Tab Enter quits, the draft being written.
    assert_eq!(typed(&mut tui, ":q<Tab><CR>"), Step::Quit(0));
}

#[test]
fn the_colon_line_and_the_searches_keep_a_history_each() {
    let (_rig, mut tui) = over("tab-history");

    // Up on the `:` line walks what was submitted at it.
    typed(&mut tui, ":pwd<CR>");
    assert_eq!(said(&mut tui), "/module/fs/root/d");
    assert_eq!(said_after(&mut tui, ":<Up>"), ":pwd");
    typed(&mut tui, "<CR>");
    assert_eq!(said(&mut tui), "/module/fs/root/d");

    // What was typed before the first Up is what every Up looks for at
    // the start of a line: `:l` walks to `ls`, past `pwd`.
    typed(&mut tui, ":ls<CR>");
    assert_eq!(said_after(&mut tui, ":l<Up>"), ":ls");
    assert_eq!(said_after(&mut tui, "<Down>"), ":l", "back to what was typed");
    typed(&mut tui, "<Esc>");

    // `/` and `?` walk their own, which the `:` lines are none of.
    typed(&mut tui, "/two<CR>");
    assert_eq!(said_after(&mut tui, "/<Up>"), "/two");
    typed(&mut tui, "<Esc>");
    assert_eq!(said_after(&mut tui, "?<Up>"), "?two", "one search history");
    typed(&mut tui, "<Esc>");
    assert_eq!(said_after(&mut tui, ":<Up>"), ":ls", "not the search's");
    typed(&mut tui, "<Esc>");
}

#[test]
fn the_prompt_takes_the_keys_the_cli_line_takes() {
    let (_rig, mut tui) = over("tab-keys");

    // The cursor moves and a char lands where it stands.
    assert_eq!(said_after(&mut tui, ":abc<Left><Left>X"), ":aXbc");
    assert_eq!(said_after(&mut tui, "<Home>Y"), ":YaXbc");
    assert_eq!(said_after(&mut tui, "<End>Z"), ":YaXbcZ");
    assert_eq!(said_after(&mut tui, "<C-w>"), ":");
    typed(&mut tui, "<Esc>");
    assert_eq!(said_after(&mut tui, ":ls /tmp<C-u>"), ":");
    typed(&mut tui, "<Esc>");

    // Backspace on an empty line leaves the prompt, as it always did,
    // and so do Esc and Ctrl-C.
    assert_eq!(said_after(&mut tui, ":x<BS><BS>"), "");
    assert_eq!(said_after(&mut tui, ":x<C-c>"), "");
    assert_eq!(said_after(&mut tui, ":x<Esc>"), "");

    // A prompt opens over an empty line, whatever the one before it
    // was left with.
    assert_eq!(said_after(&mut tui, ":abc<Esc>:"), ":");
    typed(&mut tui, "<Esc>");
}
