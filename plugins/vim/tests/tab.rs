//! Tab on the `:` line, and the line editor under the three prompts,
//! over the tui and the rig.
//!
//! The corpus cannot judge the listing: nvim leaves `:w` as `:w` and
//! lists `wall`, `while`, `wincmd`, which is the divergence this is
//! on purpose. So the rig judges it by what happened: the status row
//! after Tab, the file on disk after the expanded line ran, and what
//! a walk through the history put back on the line. What a completed
//! path ends as is nvim's own, and the corpus judges that.

use reovim_client::dev::{Clip, Table, Want};
use reovim_client_tui::{Step, Tui};
use reovim_plugin_vim::DIVERGENCES;
use reovim_rig::{Rig, opened, said, typed};

/// The one editor a test drives.
type Editor = Tui;

/// Vim for a cell; nothing else is opened here.
fn table() -> Table {
    Box::new(|want| match want {
        Want::Cell => Some((String::from("vim"), reovim_plugin_vim::boot())),
        _ => None,
    })
}

/// An editor over `d/note`, started in `d`, with `d/alpha` and the
/// directory `d/sub` beside it, which the argument completes from.
fn over(test: &str) -> (Rig, Editor) {
    let rig = Rig::new(test);
    std::fs::create_dir_all(rig.dir().join("d/sub")).unwrap();
    rig.file("d/alpha", b"");
    rig.file("d/note", b"one\ntwo\n");
    rig.file("d/sub/deep", b"");
    let host = rig.host();
    let tui = opened(&host, table(), Clip::none(), "d", Some("d/note"), None)
        .unwrap();
    (rig, tui)
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
fn one_candidate_is_the_name_expanded_and_its_usage_line() {
    let (_rig, mut tui) = over("tab-one");

    // `q` is `quit` and nothing else under PATH, so it is one
    // candidate: the whole name and a space on the line, then the
    // usage line its declaration gives. A name that takes nothing is
    // that name alone.
    assert_eq!(after(&mut tui, ":q<Tab>"), ":quit   quit");
    assert_eq!(after(&mut tui, ":wr<Tab>"), ":write   write [path]");
    assert_eq!(after(&mut tui, ":nohl<Tab>"), ":nohlsearch   nohlsearch");

    // A name PATH has not offers nothing, and the line stands as it
    // was typed.
    assert_eq!(after(&mut tui, ":zz<Tab>"), ":zz");
    assert_eq!(after(&mut tui, ":w!<Tab>"), ":w!");

    // Past the name the argument completes, and the name is written
    // out with it: one is simply put on the line.
    assert_eq!(after(&mut tui, ":write al<Tab>"), ":write alpha ");
}

#[test]
fn a_path_argument_completes_from_the_tree() {
    let (_rig, mut tui) = over("tab-paths");

    // A name under the directory the editor works in, the word
    // expanded with it. A file gets the space the next word begins
    // after; a Container gets the `/` the next Tab goes on under.
    assert_eq!(after(&mut tui, ":w al<Tab>x"), ":write alpha x");
    assert_eq!(after(&mut tui, ":e s<Tab>"), ":edit sub/");
    assert_eq!(after(&mut tui, ":e sub/<Tab>"), ":edit sub/deep ");
    assert_eq!(after(&mut tui, ":e su<Tab><Tab>"), ":edit sub/deep ");

    // Several names are the wildmenu, in name order, which Tab walks
    // as it walks the words.
    assert_eq!(
        said_after(&mut tui, ":exp <Tab>"),
        ":explore alpha   [alpha]  note  sub/"
    );
    assert_eq!(
        said_after(&mut tui, "<Tab>"),
        ":explore note   alpha  [note]  sub/"
    );
    typed(&mut tui, "<Esc>");

    // A name with nothing under it offers nothing, and the line
    // stands as the expansion left it.
    assert_eq!(after(&mut tui, ":w nowhere/x<Tab>"), ":write nowhere/x");
    assert_eq!(after(&mut tui, ":w zz<Tab>"), ":write zz");

    // A leading `/` is the root of what the editor was started over,
    // where `:w /NAME` writes, and not the root of the tree.
    assert_eq!(after(&mut tui, ":w /d<Tab>"), ":write /d/");
    assert_eq!(after(&mut tui, ":w /d/al<Tab>"), ":write /d/alpha ");
}

#[test]
fn set_completes_the_options_and_their_no_forms() {
    let (_rig, mut tui) = over("tab-set");

    assert_eq!(after(&mut tui, ":se hl<Tab>"), ":set hlsearch ");
    assert_eq!(after(&mut tui, ":set i<Tab>"), ":set ignorecase ");
    assert_eq!(
        said_after(&mut tui, ":set no<Tab>"),
        ":set noignorecase   [noignorecase]  nosmartcase  nohlsearch  novi"
    );
    typed(&mut tui, "<Esc>");
    assert_eq!(after(&mut tui, ":set zz<Tab>"), ":set zz");

    // The completed option is the one that runs: E518 would say so.
    typed(&mut tui, ":set hl<Tab><CR>");
    assert_eq!(said(&mut tui), "");
}

#[test]
fn a_cli_word_completes_as_the_cli_completes_it() {
    let (_rig, mut tui) = over("tab-cli");

    // The cli's own completion over Vim's Shell, which stands at the
    // mirror of the buffer's directory: a name under it, a Container
    // with its `/`, and the flags a word takes.
    assert_eq!(after(&mut tui, ":ls al<Tab>"), ":ls alpha ");
    assert_eq!(after(&mut tui, ":cat s<Tab>"), ":cat sub/");
    assert_eq!(after(&mut tui, ":ls -a<Tab>"), ":ls -a ");
    assert_eq!(after(&mut tui, ":ls /mod<Tab>"), ":ls /module/");

    // A name of Vim's that takes no path takes nothing from the tree.
    assert_eq!(after(&mut tui, ":q a<Tab>"), ":quit a");
}

#[test]
fn cd_moves_the_directory_a_name_resolves_under() {
    let (rig, mut tui) = over("tab-cd");

    // `:cd` moves the Shell the shell's lines stand on, and with it
    // the directory Vim's own names resolve under, as nvim's does.
    typed(&mut tui, ":cd sub<CR>");
    assert_eq!(after(&mut tui, ":w d<Tab>"), ":write deep ");
    typed(&mut tui, ":w x<CR>");
    assert_eq!(rig.disk("d/sub/x"), b"one\ntwo\n");

    // A cwd outside the mirror is no directory of the editor's, and
    // leaves it where it was.
    typed(&mut tui, ":cd /sys<CR>");
    typed(&mut tui, ":w y<CR>");
    assert_eq!(rig.disk("d/sub/y"), b"one\ntwo\n");
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

    // Vim's own `write` and `wq`, whose bin is first under PATH,
    // before `watch`, which is under `/bin`.
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
fn the_expanded_line_runs_the_name_it_holds() {
    let (rig, mut tui) = over("tab-runs");

    // `:w` Tab Enter is `:write`, which writes the draft.
    assert_eq!(typed(&mut tui, "ix<Esc>:w<Tab><CR>"), Step::Changed);
    assert_eq!(rig.disk("d/note"), b"xone\ntwo\n");

    // The wildmenu walked to another name runs that one: `watch` is a
    // Driver under `/bin`, and a target that is not there is what the
    // shell says of the run that failed, not E492.
    typed(&mut tui, ":w<Tab><Tab><Tab>nothere<CR>");
    assert_eq!(said(&mut tui), "watch nothere: gone");

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

#[test]
fn under_vi_one_candidate_is_listed_as_several_are() {
    let (_rig, mut tui) = over("tab-vi");

    // The default: the one name expanded and the usage line it
    // declares, which is the one row of `DIVERGENCES`.
    assert_eq!(after(&mut tui, ":wr<Tab>"), ":write   write [path]");

    // `vi` is an option like the search's, and Tab writes its name
    // out: `:set v` has no other option to be.
    assert_eq!(after(&mut tui, ":set v<Tab>"), ":set vi ");
    typed(&mut tui, ":set vi<CR>");

    // Under it, one candidate is the wildmenu the several are: the
    // line takes the name, the status row holds it in brackets, and
    // there is no usage line. Tab and Ctrl-P walk the one candidate,
    // which is itself.
    assert_eq!(said_after(&mut tui, ":wr<Tab>"), ":write   [write]");
    assert_eq!(said_after(&mut tui, "<Tab>"), ":write   [write]");
    assert_eq!(said_after(&mut tui, "<C-p>"), ":write   [write]");
    typed(&mut tui, "<Esc>");
    assert_eq!(after(&mut tui, ":nohl<Tab>"), ":nohlsearch   [nohlsearch]");

    // Everything else on the line is the same code under both: the
    // menu of several, and a path argument from the tree.
    assert_eq!(said_after(&mut tui, ":w<Tab>"), ":write   [write]  wq  watch");
    typed(&mut tui, "<Esc>");
    assert_eq!(after(&mut tui, ":w al<Tab>"), ":write alpha ");

    // `:set novi` is the way back, through the same option.
    typed(&mut tui, ":set novi<CR>");
    assert_eq!(after(&mut tui, ":wr<Tab>"), ":write   write [path]");
    assert_eq!(said_after(&mut tui, ":w<Tab>"), ":write   [write]  wq  watch");
    typed(&mut tui, "<Esc>");
}

#[test]
fn the_divergences_are_one_row() {
    // The one list of what reovim's default does of its own, and the
    // rows the tests above are of. A divergence added without its row,
    // or a row without its branch on `:set vi`, is this count to
    // change on purpose.
    assert_eq!(DIVERGENCES.len(), 1);
    assert_eq!(DIVERGENCES[0].keys, "Tab on the : line");
}
