//! Vim's search through the search Module: the list it reads over its
//! own draft, the marks `hlsearch` draws from it, the words it says
//! when there is no list, and the cell that names the view it holds.
//!
//! Every case here runs the Plugin through the tui on a rig whose
//! Client has loaded `search` under `/self`, as the app's does.

mod common;

use std::cell::Cell;
use std::rc::Rc;

use common::{cat, one};
use reovim_client::dev::Clip;
use reovim_client::{Host, Rd, Rvm, SELF, bin};
use reovim_client_api::notation::keys;
use reovim_client_api::{Key, MATCH, NONE, window};
use reovim_client_tui::{Step, Tui};
use reovim_kernel_interface::{
    Calls, Driver, Error, Hash, Module, Reply, Rights,
};
use reovim_module_search::Search;
use reovim_plugin_vim::Editor;
use reovim_plugin_vim::bins::subst;
use reovim_rig::{Rig, lines, listed, opened, said, typed};
use reovim_sdk::walk::ROOT;
use reovim_sdk::{cell, decode, encode, ops};

/// An editor over `text`, on a Client that loaded the search Module.
fn over(name: &str, text: &[u8]) -> (Rig, Rc<Host>, Rc<Editor>, Tui) {
    let rig = Rig::new(name);
    rig.file("file", text);
    let host = rig.searching(None);
    let vim = Editor::new();
    let tui =
        opened(&host, one(vim.clone()), Clip::none(), "", Some("file"), None)
            .unwrap();
    (rig, host, vim, tui)
}

/// The `hlsearch` spans of the frame the read itself draws, off the
/// decoded View.
fn marks(host: &Rc<Host>) -> Vec<(u64, u64)> {
    let ctx = host.own();
    let rd = cell::resolve(&ctx, ROOT, "/self/vim/view").unwrap();
    let view: window::View = decode(&ops::bytes(&ctx, rd).unwrap()).unwrap();
    view.spans
        .iter()
        .filter(|span| span.style == MATCH)
        .map(|span| (span.from, span.to))
        .collect()
}

/// What `/self/vim/search` names: the rd of the view Vim holds.
fn held(host: &Rc<Host>) -> Vec<u8> {
    let ctx = host.own();
    let rd = cell::resolve(&ctx, ROOT, "/self/vim/search").unwrap();
    ops::bytes(&ctx, rd).unwrap()
}

#[test]
fn hlsearch_marks_every_match_until_noh() {
    let (_rig, host, _vim, mut tui) = over("search-hlsearch", b"ab\nb\n");
    typed(&mut tui, "/b<CR>");
    assert_eq!(marks(&host), Vec::new(), "off by default");
    typed(&mut tui, ":set hls<CR>");
    assert_eq!(marks(&host), [(1, 2), (3, 4)], "every match");
    typed(&mut tui, ":nohlsearch<CR>");
    assert_eq!(marks(&host), Vec::new());
    typed(&mut tui, "n");
    assert_eq!(marks(&host).len(), 2, "back with the next search");
    typed(&mut tui, ":set nohlsearch<CR>");
    assert_eq!(marks(&host), Vec::new());
    typed(&mut tui, ":set ic hls<CR>/B<CR>");
    assert_eq!(marks(&host).len(), 2, "ignorecase");
    typed(&mut tui, ":set noic<CR>/B<CR>");
    assert_eq!(said(&mut tui), "E486: Pattern not found: B");
}

#[test]
fn the_lookup_passes_the_entries_before_the_cursor_on_its_line() {
    // The list is begun at the start of the cursor's own line, so the
    // entry at 0 is read before the cursor and must be passed.
    let (_rig, _host, vim, mut tui) = over("search-before", b"aXa\nb\n");
    typed(&mut tui, "/a<CR>");
    assert_eq!(vim.engine().position(), (0, 2), "the one after the cursor");
    typed(&mut tui, "n");
    assert_eq!(vim.engine().position(), (0, 0), "then around to the top");
}

#[test]
fn each_edit_shape_and_a_reload_drop_the_list() {
    // No last `\n`, so the `\n` a `:w` writes is an edit of its own.
    let (_rig, host, _vim, mut tui) =
        over("search-epoch", b"ab\nab\nab\nab\nab");
    typed(&mut tui, ":set hls<CR>/b<CR>");
    let mut seen = vec![held(&host)];
    for keys in ["x", ":s/a/z/<CR>", "u", ".", ":w<CR>"] {
        typed(&mut tui, keys);
        let now = held(&host);
        assert!(!seen.contains(&now), "{keys} left the list standing");
        seen.push(now);
    }
}

#[test]
fn hlsearch_follows_an_insert_session() {
    let (_rig, host, _vim, mut tui) = over("search-insert", b"ab\n");
    typed(&mut tui, ":set hls<CR>/b<CR>");
    assert_eq!(marks(&host), [(1, 2)]);
    typed(&mut tui, "ib");
    assert_eq!(marks(&host), [(1, 2), (2, 3)], "the match typed is marked");
    typed(&mut tui, "<BS>");
    assert_eq!(marks(&host), [(1, 2)], "and gone with the bytes again");
}

#[test]
fn a_client_without_search_says_so_and_no_key_breaks() {
    let rig = Rig::new("search-absent");
    rig.file("file", b"ab\n");
    let host = rig.host();
    let vim = Editor::new();
    let mut tui =
        opened(&host, one(vim.clone()), Clip::none(), "", Some("file"), None)
            .unwrap();
    typed(&mut tui, "/b<CR>");
    assert_eq!(said(&mut tui), "E486: this Client has no /self/search: b");
    assert_eq!(vim.engine().position(), (0, 0), "nothing moved");
    typed(&mut tui, "x");
    assert_eq!(lines(&mut tui), ["b"], "and the key after it runs");
}

#[test]
fn a_text_that_is_not_utf8_is_not_searched() {
    let (_rig, host, vim, mut tui) = over("search-lossy", b"a\xffb\n");
    typed(&mut tui, ":set hls<CR>/b<CR>");
    assert_eq!(
        said(&mut tui),
        "E486: this text is not UTF-8 and is not searched"
    );
    assert_eq!(vim.engine().position(), (0, 0), "nothing moved");
    assert_eq!(marks(&host), Vec::new(), "and nothing is marked");
    assert_eq!(held(&host), encode(&NONE), "no view was opened for it");
}

#[test]
fn a_line_past_a_mebibyte_is_said() {
    let text = format!("{}\nb\n", "a".repeat((1 << 20) + 1));
    let (_rig, _host, _vim, mut tui) =
        over("search-long-line", text.as_bytes());
    typed(&mut tui, "/b<CR>");
    assert_eq!(said(&mut tui), "E363: a line is longer than 1 MiB: b");
}

#[test]
fn a_spent_pattern_is_e363() {
    let text = format!("{}\n", "a".repeat(30));
    let (_rig, _host, _vim, mut tui) = over("search-spent", text.as_bytes());
    typed(&mut tui, "/\\(a*\\)*b<CR>");
    assert_eq!(
        said(&mut tui),
        "E363: pattern costs more than the matcher will spend: \\(a*\\)*b"
    );
}

/// The budget is small, so the slice that spends its offset is a read
/// of `matches` and not the one `open` runs: what that slice found
/// before it spent is kept in the view and handed on with the cause.
/// Where the whole text fits one slice, `open`'s own, it is not —
/// tmp/deferral-draft-open-keeps-its-slice.md has that repair, which
/// is modules/search's and not this minor's to make.
#[test]
fn matches_before_a_spent_offset_are_kept() {
    let text = format!("x\nb\n{}\n", "a".repeat(30));
    let rig = Rig::new("search-spent-kept");
    rig.file("file", text.as_bytes());
    let host = rig.searching(Some(1));
    let vim = Editor::new();
    let mut tui =
        opened(&host, one(vim.clone()), Clip::none(), "", Some("file"), None)
            .unwrap();
    typed(&mut tui, "/\\(a*\\)*b<CR>");
    assert_eq!(vim.engine().position(), (1, 0), "what it found is landed on");
    let row = said(&mut tui);
    assert!(
        row.starts_with("/\\(a*\\)*b  [?/1+]"),
        "no word is said, and the row holds what was found: {row}"
    );
    typed(&mut tui, "n");
    assert!(said(&mut tui).starts_with("E363"), "{}", said(&mut tui));
}

#[test]
fn the_search_cell_names_the_view_vim_holds() {
    let rig = Rig::new("search-cell");
    rig.file("file", b"ab\n");
    rig.file("other", b"cd\n");
    let host = rig.searching(None);
    let mut tui =
        opened(&host, one(Editor::new()), Clip::none(), "", Some("file"), None)
            .unwrap();
    assert_eq!(held(&host), encode(&NONE), "nothing held yet");
    assert_eq!(listed(&host, "/self/search"), ["open"]);
    typed(&mut tui, "/b<CR>");
    let view: Rd = decode(&held(&host)).unwrap();
    assert_ne!(view, NONE, "the view Vim holds");
    assert_eq!(listed(&host, "/self/search"), ["open", "0"]);
    // A list of another key closes the one before it: Vim holds one.
    typed(&mut tui, "/a<CR>");
    assert_eq!(listed(&host, "/self/search"), ["open", "1"]);
    // An edit drops the list, and the frame after it drops the view
    // the list was read from, `hlsearch` wanting nothing.
    typed(&mut tui, "x");
    assert_eq!(held(&host), encode(&NONE));
    assert_eq!(listed(&host, "/self/search"), ["open"]);
    // The stay ends with the swap, and `Host::shed` closes the
    // handle that held the view, which is what ends it.
    typed(&mut tui, ":e other<CR>");
    assert_eq!(held(&host), encode(&NONE));
    assert_eq!(listed(&host, "/self/search"), ["open"]);
}

#[test]
fn the_spans_stop_at_the_views_bound() {
    let text = format!("{}\n", "a".repeat(5000));
    let (_rig, host, _vim, mut tui) = over("search-spans", text.as_bytes());
    typed(&mut tui, ":set hls<CR>/a<CR>");
    assert_eq!(marks(&host).len(), 4096, "what a View carries, no more");
}

/// The search Module as the rig loads it, wrapped: every read of it
/// counted, and refused where a case asks for a refusal of no cause.
struct Watched {
    inner: Search,
    reads: Cell<usize>,
    deny: bool,
}

impl Watched {
    fn new(deny: bool, budget: Option<usize>) -> Rc<Self> {
        let inner = budget.map_or_else(Search::new, Search::with_budget);
        Rc::new(Self { inner, reads: Cell::new(0), deny })
    }
}

impl Module for Watched {
    fn start(&self, ctx: &dyn Calls, home: Rd) -> Result<(), Error> {
        self.inner.start(ctx, home)
    }
}

impl Driver for Watched {
    fn open(&self, ctx: &dyn Calls, rd: Rd, rights: Rights) -> Reply<u64> {
        self.inner.open(ctx, rd, rights)
    }

    fn read(
        &self,
        ctx: &dyn Calls,
        h: u64,
        at: u64,
        len: usize,
    ) -> Reply<Vec<u8>> {
        self.reads.set(self.reads.get() + 1);
        if self.deny {
            return Reply::err(Error::Denied);
        }
        self.inner.read(ctx, h, at, len)
    }

    fn call(
        &self,
        ctx: &dyn Calls,
        h: u64,
        api: Hash,
        method: u16,
        input: &[u8],
    ) -> Reply<Vec<u8>> {
        self.inner.call(ctx, h, api, method, input)
    }

    fn close(&self, ctx: &dyn Calls, h: u64) -> Reply<()> {
        self.inner.close(ctx, h)
    }
}

/// An editor over `text` whose Client loaded `watched` as its search
/// Module, by hand as tests/cells.rs loads coreutils.
fn wrapped(
    name: &str,
    text: &[u8],
    watched: &Rc<Watched>,
) -> (Rig, Rc<Host>, Rc<Editor>, Tui) {
    let rig = Rig::new(name);
    rig.file("file", text);
    let host = rig.host();
    host.load("search", watched.clone(), vec![watched.clone()]).unwrap();
    let vim = Editor::new();
    let tui =
        opened(&host, one(vim.clone()), Clip::none(), "", Some("file"), None)
            .unwrap();
    (rig, host, vim, tui)
}

#[test]
fn an_error_of_no_cause_ends_the_wait() {
    let denying = Watched::new(true, None);
    let (_rig, host, vim, mut tui) =
        wrapped("search-denied", b"ab\n", &denying);
    for key in keys("d/b").unwrap() {
        tui.key(key).unwrap();
    }
    assert!(tui.key(Key::Enter).is_err(), "the refusal goes up");
    // The frame it refused drew nothing, so the row is the next
    // frame's; nothing of the search is read on it.
    tui.tick().unwrap();
    assert_eq!(said(&mut tui), "E486: Pattern not found: b");
    assert_eq!(cat(&host, "pending"), "", "no operator is left waiting");
    typed(&mut tui, "l");
    assert_eq!(vim.engine().position(), (0, 1), "the key after it moves");
}

#[test]
fn ten_idle_frames_read_nothing() {
    let counting = Watched::new(false, None);
    let (_rig, host, _vim, mut tui) =
        wrapped("search-idle", b"ab\nb\n", &counting);
    typed(&mut tui, ":set hls<CR>/b<CR>");
    let ctx = host.own();
    let view: Rd = decode(&held(&host)).unwrap();
    let progress = cell::resolve(&ctx, view, "progress").unwrap();
    let reads = counting.reads.get();
    let mtime = ops::info(&ctx, progress).unwrap().mtime;
    for _ in 0..10 {
        tui.tick().unwrap();
    }
    assert_eq!(counting.reads.get(), reads, "no read of matches");
    assert_eq!(
        ops::info(&ctx, progress).unwrap().mtime,
        mtime,
        "and so no slice"
    );
}

/// A text of about 4 KiB: two-byte lines of `a`, and `zzz` alone on
/// the last. At the budget the cases below spend, a frame's four
/// slices cover some 256 offsets of it, so a search over it stands
/// across many frames.
fn long() -> Vec<u8> {
    let mut text = "a\n".repeat(2047);
    text.push_str("zzz\n");
    text.into_bytes()
}

/// An editor over `text` whose search Module spends at most `budget`
/// on one slice, so a search over a text of any size waits.
fn spending(
    name: &str,
    text: &[u8],
    budget: usize,
) -> (Rig, Rc<Host>, Rc<Editor>, Tui) {
    let rig = Rig::new(name);
    rig.file("file", text);
    let host = rig.searching(Some(budget));
    let vim = Editor::new();
    let tui =
        opened(&host, one(vim.clone()), Clip::none(), "", Some("file"), None)
            .unwrap();
    (rig, host, vim, tui)
}

/// Frames with no key until the search that waits has landed, and how
/// many it took.
fn landed(tui: &mut Tui, vim: &Rc<Editor>) -> usize {
    for frames in 1..200 {
        tui.tick().unwrap();
        if !vim.engine().waiting() {
            return frames;
        }
    }
    panic!("the search never landed");
}

/// `text`'s keys as one write of the keyboard, so the frame after
/// them finds every one of them in its queue at once.
fn together(host: &Rc<Host>, text: &str) {
    let ctx = host.own();
    let keyboard = cell::resolve(&ctx, SELF, "dev/input/keyboard").unwrap();
    let mut codes = Vec::new();
    for key in keys(text).unwrap() {
        codes.extend(key.code().to_le_bytes());
    }
    ops::put(&ctx, keyboard, codes).unwrap();
}

#[test]
fn a_long_search_answers_moved_then_lands_where_the_whole_one_lands() {
    let (_rig, _host, vim, mut tui) = spending("search-long", &long(), 64);
    typed(&mut tui, "/zzz<CR>");
    assert_eq!(vim.engine().position(), (0, 0), "nothing has moved yet");
    assert!(vim.engine().waiting(), "and the search stands");
    let frames = landed(&mut tui, &vim);
    assert!(frames > 1, "it took one frame alone");
    assert!(frames < 100, "it took {frames} frames");

    // Where the same search lands with no budget to run out of.
    let (_rig, _host, whole, mut tui) = over("search-long-whole", &long());
    typed(&mut tui, "/zzz<CR>");
    assert!(!whole.engine().waiting(), "one slice covered it");
    assert_eq!(vim.engine().position(), whole.engine().position());
}

#[test]
fn an_operator_deletes_on_landing_and_one_u_undoes_it() {
    let (_rig, _host, vim, mut tui) =
        spending("search-long-operator", &long(), 64);
    let before = vim.engine().text().to_string();
    typed(&mut tui, "d/zzz<CR>");
    assert!(vim.engine().waiting());
    assert_eq!(
        vim.engine().text(),
        before,
        "nothing is deleted while it waits"
    );
    landed(&mut tui, &vim);
    assert_eq!(vim.engine().text(), "zzz\n", "the delete ran at the landing");
    typed(&mut tui, "u");
    assert_eq!(vim.engine().text(), before, "and one `u` takes it back");
}

#[test]
fn keys_typed_meanwhile_run_after_it_in_order() {
    let (_rig, _host, vim, mut tui) =
        spending("search-long-queued", &long(), 64);
    let before = vim.engine().text().to_string();
    typed(&mut tui, "/zzz<CR>");
    typed(&mut tui, "ix<Esc>");
    assert!(vim.engine().waiting(), "the keys wait behind the search");
    assert_eq!(vim.engine().text(), before, "and change nothing meanwhile");
    landed(&mut tui, &vim);
    assert!(
        vim.engine().text().ends_with("\nxzzz\n"),
        "the `x` stands at the match"
    );
}

#[test]
fn ctrl_c_anywhere_and_esc_at_the_front_end_it() {
    // A first line of two chars, so the `l` after the interrupt has
    // somewhere to go.
    let mut text = String::from("ab\n");
    text.push_str(&"a\n".repeat(2046));
    text.push_str("zzz\n");
    let (_rig, host, vim, mut tui) =
        spending("search-long-interrupt", text.as_bytes(), 64);
    let before = vim.engine().text().to_string();
    typed(&mut tui, "d/zzz<CR>");
    assert!(vim.engine().waiting());
    // The `x` stood behind the search, so it goes through the `<C-c>`
    // with it; the `l` was typed after and runs.
    together(&host, "x<C-c>l");
    tui.tick().unwrap();
    assert!(!vim.engine().waiting(), "the interrupt ended the wait");
    assert_eq!(vim.engine().text(), before, "the text is whole");
    assert_eq!(vim.engine().position(), (0, 1), "one to the right");
    assert_eq!(cat(&host, "pending"), "", "no operator is left waiting");
    assert_eq!(said(&mut tui), "", "and nothing is said");

    typed(&mut tui, "0d/zzz<CR>");
    assert!(vim.engine().waiting());
    together(&host, "<Esc>l");
    tui.tick().unwrap();
    assert!(!vim.engine().waiting(), "an Esc at the front ends it too");
    assert_eq!(vim.engine().text(), before);
    assert_eq!(vim.engine().position(), (0, 1));
    assert_eq!(cat(&host, "pending"), "");
}

#[test]
fn a_macro_waits_at_its_search_and_is_one_undo_step() {
    // The macro's own two lines, then the long text. Its search is
    // anchored, so the line it leaves behind — `/^zzz` — is no match
    // of its own and the landing is the text's last line.
    let mut text = String::from("x/^zzz\nx\n");
    text.push_str(&String::from_utf8(long()).unwrap());
    let (rig, _host, vim, mut tui) =
        spending("search-long-macro", text.as_bytes(), 64);
    let mut other = rig.client();
    let source = other.resolve("/module/fs/root/file").unwrap();
    let buffer = other.resolve(&format!("/module/text/{}", source.0)).unwrap();
    let before = other.read_all(buffer).unwrap();

    typed(&mut tui, "\"a2yy@a");
    assert!(vim.engine().waiting(), "the run waits at its search");
    assert_eq!(
        other.read_all(buffer).unwrap(),
        before,
        "the frame that waits ends no revision"
    );
    landed(&mut tui, &vim);
    assert!(
        vim.engine().text().starts_with("/^zzz\nx\n"),
        "{}",
        &vim.engine().text()[..16]
    );
    assert!(vim.engine().text().ends_with("\nzz\n"), "the landing's `x`");
    typed(&mut tui, "u");
    assert_eq!(vim.engine().text(), String::from_utf8(before).unwrap());
}

#[test]
fn a_replayed_n_waits_like_a_typed_one() {
    let (_rig, _host, vim, mut tui) =
        spending("search-long-repeat", &long(), 64);
    let before = vim.engine().text().to_string();
    typed(&mut tui, "/zzz<CR>");
    landed(&mut tui, &vim);
    typed(&mut tui, "ggdn");
    assert_eq!(vim.engine().text(), "zzz\n", "the list it landed from held it");
    typed(&mut tui, "u");
    assert_eq!(vim.engine().text(), before, "and the undo drops that list");
    // Twice: the second `.` is over a list the second `u` dropped too.
    for _ in 0..2 {
        typed(&mut tui, ".");
        assert!(vim.engine().waiting(), "a replayed `n` waits as a typed one");
        landed(&mut tui, &vim);
        assert_eq!(vim.engine().text(), "zzz\n", "and deletes what `dn` did");
        typed(&mut tui, "u");
        assert_eq!(vim.engine().text(), before);
    }
}

#[test]
fn a_spent_offset_is_e363_and_length_alone_is_not() {
    let mut text = "a\n".repeat(2047);
    text.push_str(&"a".repeat(30));
    text.push('\n');
    let (_rig, _host, vim, mut tui) =
        spending("search-long-spent", text.as_bytes(), 64);
    typed(&mut tui, "/\\(a*\\)*b<CR>");
    assert!(vim.engine().waiting(), "a long text alone never says E363");
    landed(&mut tui, &vim);
    assert_eq!(
        said(&mut tui),
        "E363: pattern costs more than the matcher will spend: \\(a*\\)*b"
    );
}

/// The percent the row holds, in thousandths.
fn percent(row: &str) -> u64 {
    let (_, rest) = row.split_once("]  ").expect(row);
    let (shown, _) = rest.split_once('%').expect(row);
    let (units, thousandths) = shown.split_once('.').expect(row);
    units.parse::<u64>().unwrap() * 1000 + thousandths.parse::<u64>().unwrap()
}

#[test]
fn the_row_moves_with_no_key() {
    let (_rig, _host, vim, mut tui) = spending("search-long-row", &long(), 64);
    typed(&mut tui, "/zzz<CR>");
    let mut last = 0;
    let mut seen = 0;
    while vim.engine().waiting() {
        assert_eq!(tui.tick().unwrap(), Step::Changed, "the row moved");
        if !vim.engine().waiting() {
            break;
        }
        let row = said(&mut tui);
        assert!(row.starts_with("/zzz  [?/0+]  "), "{row}");
        let now = percent(&row);
        assert!(now > last, "{now} is not past {last}");
        last = now;
        seen += 1;
    }
    assert!(seen > 1, "only {seen} rows were drawn");
}

#[test]
fn a_host_with_no_clock_shows_no_eta() {
    let rig = Rig::new("search-long-clockless");
    rig.file("file", &long());
    // A Host whose clock answers 0, as host/os's empty one does: the
    // `progress` rd's own stamps are then one time, and there is no
    // measure of what is left.
    let host = Rc::new(Host::over(rig.client(), || 0));
    bin::boot(&host, Rvm::none()).unwrap();
    let search = Rc::new(Search::with_budget(64));
    host.load("search", search.clone(), vec![search]).unwrap();
    let vim = Editor::new();
    let mut tui =
        opened(&host, one(vim.clone()), Clip::none(), "", Some("file"), None)
            .unwrap();
    typed(&mut tui, "/zzz<CR>");
    let mut seen = 0;
    while vim.engine().waiting() {
        tui.tick().unwrap();
        if !vim.engine().waiting() {
            break;
        }
        let row = said(&mut tui);
        assert!(row.contains('%'), "{row}");
        assert!(!row.contains('~'), "no clock, no time left: {row}");
        seen += 1;
    }
    assert!(seen > 1, "only {seen} rows were drawn");
}

#[test]
fn a_call_from_elsewhere_that_edits_ends_the_wait() {
    let (_rig, host, vim, mut tui) =
        spending("search-long-called", &long(), 64);
    typed(&mut tui, "d/zzz<CR>");
    assert!(vim.engine().waiting());
    // A name of Vim's own, run from a Client of its own while the
    // search waits: it edits, so it ends the wait.
    let ctx = host.own();
    let rd = cell::resolve(&ctx, ROOT, "/self/vim/bin/substitute").unwrap();
    let args = subst::Args { from: 0, to: 0, spec: String::from("/a/z/") };
    ops::call_by_name(&ctx, rd, "run", encode(&args)).unwrap();
    assert!(!vim.engine().waiting(), "the edit ended it");
    assert_eq!(cat(&host, "pending"), "", "no operator is left waiting");
    assert!(vim.engine().text().starts_with("z\na\n"), "the substitute's own");
    tui.tick().unwrap();
    tui.tick().unwrap();
    assert!(
        vim.engine().text().starts_with("z\na\n"),
        "and no frame after it deletes"
    );
}

#[test]
fn the_row_fills_then_counts_with_no_key() {
    let counting = Watched::new(false, Some(64));
    let text = long();
    // Every `a` of the text is a match of its own; `zzz` holds none.
    let whole = std::str::from_utf8(&text).unwrap().matches('a').count();
    let (_rig, _host, vim, mut tui) =
        wrapped("search-count-fills", &text, &counting);
    typed(&mut tui, "/a<CR>");
    assert!(!vim.engine().waiting(), "the first slice landed it");
    let row = said(&mut tui);
    assert!(row.starts_with("/a  [?/"), "no rank while it fills: {row}");
    // Frames with no key: the fill goes on for the row alone, and the
    // `?` turns into a number where the list turns whole.
    let counted = format!("/a  [2/{whole}]");
    let mut frames = 0;
    while said(&mut tui) != counted {
        assert_eq!(tui.tick().unwrap(), Step::Changed, "the row moved");
        frames += 1;
        assert!(frames < 500, "the list never turned whole");
    }
    assert!(frames > 1, "it was whole on the frame that landed");
    // A counted row wants nothing: the list is as full as it will get.
    let reads = counting.reads.get();
    for _ in 0..10 {
        assert_eq!(tui.tick().unwrap(), Step::Same, "the row stands");
    }
    assert_eq!(said(&mut tui), counted);
    assert_eq!(counting.reads.get(), reads, "and nothing is read for it");
}
