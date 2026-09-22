//! Vim's search through the search Module: the list it reads over its
//! own draft, the marks `hlsearch` draws from it, the words it says
//! when there is no list, and the cell that names the view it holds.
//!
//! Every case here runs the Plugin through the tui on a rig whose
//! Client has loaded `search` under `/self`, as the app's does.

mod common;

use std::cell::Cell;
use std::rc::Rc;

use common::{cat, on, one};
use reovim_client::dev::Clip;
use reovim_client::{Host, Rd, Rvm, SELF, bin};
use reovim_client_api::notation::keys;
use reovim_client_api::{MATCH, NONE, window};
use reovim_client_tui::{Step, Tui};
use reovim_kernel_interface::{
    Calls, Driver, Error, Hash, Module, Reply, Rights,
};
use reovim_module_search::Search;
use reovim_plugin_vim::bins::subst;
use reovim_plugin_vim::{Effect, Filled};
use reovim_plugin_vim_host::Editor;
use reovim_rig::{Rig, lines, listed, opened, said, typed};
use reovim_sdk::walk::ROOT;
use reovim_sdk::{cell, decode, encode, ops};
use reovim_search_api::search::Match;

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

/// The search's answer is a typed door of the engine's own, not a
/// declared reply: the host hands over the Module's own `Match`es and
/// how far the fill got, and the key that waited runs again on that
/// call alone. Nothing of a match is in the resolver's contract,
/// which names no offset and no length.
#[test]
fn a_search_answers_through_its_own_door() {
    // Through the host, where the entries come off `matches`: the
    // key lands where the Module's list says it does.
    let (_rig, _host, vim, mut tui) = over("search-own-door", b"a\nzzz\n");
    typed(&mut tui, "/zzz<CR>");
    assert_eq!(vim.position(), (1, 0), "the list the host read landed it");

    // And by hand, which is the door itself: the epoch the want was
    // asked on, the matches as the Module spells them, and how far
    // the fill got.
    let mut engine = on("a\nzzz\n");
    let Effect::Search(want) = common::typed(&mut engine, "/zzz<CR>") else {
        panic!("the key asks for the list it cannot answer from")
    };
    let found = [Match { over: NONE, at: 2, len: 3 }];
    assert_eq!(
        engine.matched(want.epoch + 9, &found, Filled::Whole),
        Effect::Nothing,
        "a list that is gone moves nothing"
    );
    assert_eq!(engine.position(), (0, 0), "so the key still waits");
    assert_eq!(
        engine.matched(want.epoch, &found, Filled::Whole),
        Effect::Moved
    );
    assert_eq!(engine.position(), (1, 0), "the motion ran again");
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

/// `x*` matches the empty string at every column, including each
/// line's end; nvim's own per-line search never finds the one there
/// that the match before it already reached, and neither does the
/// fill, so `hlsearch` marks what the count counts. `b\|$` is the
/// same for one match a literal reaches rather than another
/// zero-width one.
#[test]
fn hlsearch_marks_the_entries_the_count_counts() {
    let (_rig, host, _vim, mut tui) = over("search-hls-skipped", b"ab\ncd\n");
    typed(&mut tui, ":set hls<CR>/x*<CR>");
    assert_eq!(
        marks(&host),
        [(0, 1), (1, 2), (3, 4), (4, 5)],
        "not the two line ends x* also matches"
    );
    typed(&mut tui, "/b\\|$<CR>");
    assert_eq!(
        marks(&host),
        [(1, 2), (5, 6)],
        "not the line end b already reached"
    );
}

#[test]
fn the_lookup_passes_the_entries_before_the_cursor_on_its_line() {
    // The list is begun at the start of the cursor's own line, so the
    // entry at 0 is read before the cursor and must be passed.
    let (_rig, _host, vim, mut tui) = over("search-before", b"aXa\nb\n");
    typed(&mut tui, "/a<CR>");
    assert_eq!(vim.position(), (0, 2), "the one after the cursor");
    typed(&mut tui, "n");
    assert_eq!(vim.position(), (0, 0), "then around to the top");
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
    assert_eq!(vim.position(), (0, 0), "nothing moved");
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
    assert_eq!(vim.position(), (0, 0), "nothing moved");
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
    assert_eq!(vim.position(), (1, 0), "what it found is landed on");
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
    // Another line over the same draft seeks the view Vim already
    // holds rather than opening a second one.
    typed(&mut tui, "/a<CR>");
    let same: Rd = decode(&held(&host)).unwrap();
    assert_eq!(same, view, "the one view, sought and not reopened");
    assert_eq!(listed(&host, "/self/search"), ["open", "0"]);
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
/// counted, its calls counted by the method the api declares, and
/// refused where a case asks for a refusal of no cause.
struct Watched {
    inner: Search,
    reads: Cell<usize>,
    /// `search.open`, method 0, however it answered.
    opens: Cell<usize>,
    /// `search.seek`, method 1, however it answered.
    seeks: Cell<usize>,
    deny: bool,
}

impl Watched {
    fn new(deny: bool, budget: Option<usize>) -> Rc<Self> {
        let inner = budget.map_or_else(Search::new, Search::with_budget);
        Rc::new(Self {
            inner,
            reads: Cell::new(0),
            opens: Cell::new(0),
            seeks: Cell::new(0),
            deny,
        })
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
        let counter = match method {
            0 => Some(&self.opens),
            1 => Some(&self.seeks),
            _ => None,
        };
        if let Some(counter) = counter {
            counter.set(counter.get() + 1);
        }
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
    // One write, so the whole line runs in one frame and the read
    // refused is the Enter's own search, not a preview's: a key of
    // the line asks for its own list, and this Driver denies that too.
    together(&host, "d/b<CR>");
    assert!(tui.tick().is_err(), "the refusal goes up");
    // The frame it refused drew nothing, so the row is the next
    // frame's; nothing of the search is read on it.
    tui.tick().unwrap();
    assert_eq!(said(&mut tui), "E486: Pattern not found: b");
    assert_eq!(cat(&host, "pending"), "", "no operator is left waiting");
    typed(&mut tui, "l");
    assert_eq!(vim.position(), (0, 1), "the key after it moves");
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
        if !vim.waiting() {
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
    assert_eq!(vim.position(), (0, 0), "nothing has moved yet");
    assert!(vim.waiting(), "and the search stands");
    let frames = landed(&mut tui, &vim);
    assert!(frames > 1, "it took one frame alone");
    assert!(frames < 100, "it took {frames} frames");

    // Where the same search lands with no budget to run out of.
    let (_rig, _host, whole, mut tui) = over("search-long-whole", &long());
    typed(&mut tui, "/zzz<CR>");
    assert!(!whole.waiting(), "one slice covered it");
    assert_eq!(vim.position(), whole.position());
}

#[test]
fn an_operator_deletes_on_landing_and_one_u_undoes_it() {
    let (_rig, _host, vim, mut tui) =
        spending("search-long-operator", &long(), 64);
    let before = vim.text();
    typed(&mut tui, "d/zzz<CR>");
    assert!(vim.waiting());
    assert_eq!(vim.text(), before, "nothing is deleted while it waits");
    landed(&mut tui, &vim);
    assert_eq!(vim.text(), "zzz\n", "the delete ran at the landing");
    typed(&mut tui, "u");
    assert_eq!(vim.text(), before, "and one `u` takes it back");
}

#[test]
fn keys_typed_meanwhile_run_after_it_in_order() {
    let (_rig, _host, vim, mut tui) =
        spending("search-long-queued", &long(), 64);
    let before = vim.text();
    typed(&mut tui, "/zzz<CR>");
    typed(&mut tui, "ix<Esc>");
    assert!(vim.waiting(), "the keys wait behind the search");
    assert_eq!(vim.text(), before, "and change nothing meanwhile");
    landed(&mut tui, &vim);
    assert!(vim.text().ends_with("\nxzzz\n"), "the `x` stands at the match");
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
    let before = vim.text();
    typed(&mut tui, "d/zzz<CR>");
    assert!(vim.waiting());
    // The `x` stood behind the search, so it goes through the `<C-c>`
    // with it; the `l` was typed after and runs.
    together(&host, "x<C-c>l");
    tui.tick().unwrap();
    assert!(!vim.waiting(), "the interrupt ended the wait");
    assert_eq!(vim.text(), before, "the text is whole");
    assert_eq!(vim.position(), (0, 1), "one to the right");
    assert_eq!(cat(&host, "pending"), "", "no operator is left waiting");
    assert_eq!(said(&mut tui), "", "and nothing is said");

    typed(&mut tui, "0d/zzz<CR>");
    assert!(vim.waiting());
    together(&host, "<Esc>l");
    tui.tick().unwrap();
    assert!(!vim.waiting(), "an Esc at the front ends it too");
    assert_eq!(vim.text(), before);
    assert_eq!(vim.position(), (0, 1));
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
    assert!(vim.waiting(), "the run waits at its search");
    assert_eq!(
        other.read_all(buffer).unwrap(),
        before,
        "the frame that waits ends no revision"
    );
    landed(&mut tui, &vim);
    assert!(vim.text().starts_with("/^zzz\nx\n"), "{}", &vim.text()[..16]);
    assert!(vim.text().ends_with("\nzz\n"), "the landing's `x`");
    typed(&mut tui, "u");
    assert_eq!(vim.text(), String::from_utf8(before).unwrap());
}

#[test]
fn a_replayed_n_waits_like_a_typed_one() {
    let (_rig, _host, vim, mut tui) =
        spending("search-long-repeat", &long(), 64);
    let before = vim.text();
    typed(&mut tui, "/zzz<CR>");
    landed(&mut tui, &vim);
    typed(&mut tui, "ggdn");
    assert_eq!(vim.text(), "zzz\n", "the list it landed from held it");
    typed(&mut tui, "u");
    assert_eq!(vim.text(), before, "and the undo drops that list");
    // Twice: the second `.` is over a list the second `u` dropped too.
    for _ in 0..2 {
        typed(&mut tui, ".");
        assert!(vim.waiting(), "a replayed `n` waits as a typed one");
        landed(&mut tui, &vim);
        assert_eq!(vim.text(), "zzz\n", "and deletes what `dn` did");
        typed(&mut tui, "u");
        assert_eq!(vim.text(), before);
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
    assert!(vim.waiting(), "a long text alone never says E363");
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
    while vim.waiting() {
        assert_eq!(tui.tick().unwrap(), Step::Changed, "the row moved");
        if !vim.waiting() {
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
    while vim.waiting() {
        tui.tick().unwrap();
        if !vim.waiting() {
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
    assert!(vim.waiting());
    // A name of Vim's own, run from a Client of its own while the
    // search waits: it edits, so it ends the wait.
    let ctx = host.own();
    let rd = cell::resolve(&ctx, ROOT, "/self/vim/bin/substitute").unwrap();
    let args =
        subst::Args { address: String::from("1"), spec: String::from("/a/z/") };
    ops::call_by_name(&ctx, rd, "run", encode(&args)).unwrap();
    assert!(!vim.waiting(), "the edit ended it");
    assert_eq!(cat(&host, "pending"), "", "no operator is left waiting");
    assert!(vim.text().starts_with("z\na\n"), "the substitute's own");
    tui.tick().unwrap();
    tui.tick().unwrap();
    assert!(vim.text().starts_with("z\na\n"), "and no frame after it deletes");
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
    assert!(!vim.waiting(), "the first slice landed it");
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

/// A list still filling marks what it has found and nothing else,
/// whatever the pattern before it found. `x*` finds two entries a
/// line and none at a line's end; `$` finds every line's end and
/// nothing else, so a `$` search right after an `x*` one is the case
/// where the two lists agree on no byte at all.
#[test]
fn a_new_pattern_marks_what_its_own_list_has_found() {
    let mut text = String::from("ab\ncd\n");
    text.push_str(&"e\n".repeat(3));
    let rig = Rig::new("search-hls-skip-cleared");
    rig.file("file", text.as_bytes());
    let host = rig.searching(Some(1));
    let vim = Editor::new();
    let mut tui =
        opened(&host, one(vim.clone()), Clip::none(), "", Some("file"), None)
            .unwrap();
    typed(&mut tui, ":set hls<CR>/x*<CR>");
    while vim.counted().is_none() {
        tui.tick().unwrap();
    }
    assert_eq!(vim.counted(), Some((2, 7)), "x*'s own list is whole");
    assert_eq!(
        marks(&host),
        [(0, 1), (1, 2), (3, 4), (4, 5), (6, 7), (8, 9), (10, 11)],
        "none of the five line ends x* also matches"
    );

    // One write, so the whole line runs in one frame: a key of the
    // line asks for its own list, and a preview frame for `$` would
    // have filled it before the Enter ever read it.
    together(&host, "/$<CR>");
    tui.tick().unwrap();
    assert_eq!(vim.counted(), None, "$'s own list is still filling");
    assert_eq!(
        marks(&host),
        [(2, 3), (5, 6), (7, 8)],
        "what $ has found so far, and nothing of x*'s"
    );

    // `$`'s view is x*'s, sought rather than opened, and a seek runs
    // one slice inside its own call as an open does: the tick after
    // is the whole list, no later than a fresh view's own would be.
    tui.tick().unwrap();
    assert_eq!(vim.counted(), Some((1, 5)), "$'s own list is whole");
    assert_eq!(
        marks(&host),
        [(2, 3), (5, 6), (7, 8), (9, 10), (11, 12)],
        "every line end, none of $'s own left out"
    );
}
/// Sixty lines, `fx` on line 10 and `foo` on line 40: a typed `f`
/// finds the near one, `fo` the far one, which is past the window.
fn apart() -> String {
    let mut text = String::new();
    for n in 1..=60u32 {
        let line = match n {
            10 => String::from("fx"),
            40 => String::from("foo"),
            _ => format!("line {n}"),
        };
        text.push_str(&line);
        text.push('\n');
    }
    text
}

/// `held`'s answer kept where it is one this run has not seen: how
/// many views the line has opened so far.
fn noted(host: &Rc<Host>, seen: &mut Vec<Vec<u8>>) {
    let now = held(host);
    if !seen.contains(&now) {
        seen.push(now);
    }
}

#[test]
fn the_match_follows_each_key() {
    let text = apart();
    let near = text.find("fx").unwrap() as u64;
    let (_rig, host, vim, mut tui) = over("inc-follows", text.as_bytes());
    typed(&mut tui, "/");
    assert_eq!(vim.position(), (0, 0), "an empty line moves none");
    typed(&mut tui, "f");
    assert_eq!(vim.position(), (9, 0), "the near one");
    assert_eq!(
        marks(&host),
        [(near, near + 1)],
        "one MATCH span, `hlsearch` off"
    );
    assert_eq!(lines(&mut tui)[0], "line 1", "the window has not moved");
    typed(&mut tui, "o");
    assert_eq!(vim.position(), (39, 0), "the far one");
    assert_eq!(marks(&host).len(), 1, "and the mark with it");
    assert_ne!(lines(&mut tui)[0], "line 1", "the window followed");
    typed(&mut tui, "<BS>");
    assert_eq!(vim.position(), (9, 0), "the near one again");
    assert_eq!(lines(&mut tui)[0], "line 1", "and the window back");
    typed(&mut tui, "<Esc>");
    assert_eq!(vim.position(), (0, 0), "the origin");
    assert_eq!(marks(&host), Vec::new(), "and no mark");
    assert_eq!(lines(&mut tui)[0], "line 1");
}

/// The view the first key of a pattern opens is the one every later
/// key seeks, whether it comes on its own frame or with others: an
/// `<Esc>` and a wholly new line after it still read that same view,
/// the source under it unchanged.
#[test]
fn a_key_opens_a_view_and_every_later_key_seeks_it() {
    let (_rig, host, _vim, mut tui) = over("inc-views", b"abc\nfoo bar\n");
    let mut seen = vec![held(&host)];
    for key in ["/", "f", "o"] {
        typed(&mut tui, key);
        noted(&host, &mut seen);
    }
    assert_eq!(seen.len(), 2, "the empty line opens none, `f` one");
    typed(&mut tui, "<Esc>");
    noted(&host, &mut seen);
    together(&host, "/foo<CR>");
    tui.tick().unwrap();
    noted(&host, &mut seen);
    assert_eq!(seen.len(), 2, "still the one view, sought and not reopened");
}

/// The views `held` has named over a run, in order and each once.
/// `NONE` is no view and is none of them.
fn views(host: &Rc<Host>, seen: &mut Vec<Vec<u8>>) {
    let now = held(host);
    if now != encode(&NONE) && !seen.contains(&now) {
        seen.push(now);
    }
}

/// A `/` line's own journey: the view opened at the line's first key
/// and sought at every key after it, one call a key.
#[test]
fn a_line_seeks_one_view_a_key() {
    let counting = Watched::new(false, None);
    let (_rig, host, vim, mut tui) =
        wrapped("inc-one-view", b"abc\nfoo abc\n", &counting);
    let mut seen = Vec::new();
    for key in ["/", "a", "b", "c", "<CR>"] {
        typed(&mut tui, key);
        views(&host, &mut seen);
    }
    assert_eq!(seen.len(), 1, "one view from the first key to the last");
    assert_eq!(counting.opens.get(), 1, "opened once");
    // `<CR>` asks the engine no new list of its own: the preview's
    // own already stands and lands the motion on it, so `b` and `c`
    // are this line's only two calls after its first.
    assert_eq!(counting.seeks.get(), 2, "and sought at every key that grew it");
    assert_eq!(vim.position(), (1, 4), "and the line landed");
}

/// The view is the stay's: the swap ends it, and the next line opens
/// one of its own over the source it now has.
#[test]
fn a_stay_swap_opens_another_view() {
    let counting = Watched::new(false, None);
    let (rig, host, _vim, mut tui) = wrapped("inc-swap", b"abc\n", &counting);
    rig.file("other", b"xa\n");
    let mut seen = Vec::new();
    typed(&mut tui, "/a");
    views(&host, &mut seen);
    assert_eq!(counting.opens.get(), 1, "the line's own view");
    typed(&mut tui, "<Esc>:e other<CR>/a");
    views(&host, &mut seen);
    assert_eq!(counting.opens.get(), 2, "another source, another view");
    assert_eq!(seen.len(), 2, "and it is not the one before it");
}

/// A half-typed pattern is the line's own middle: the Module refuses
/// the seek as `Invalid`, the engine hears what a refused `open`
/// says, and the view keeps the list it had for the next key.
#[test]
fn a_pattern_that_does_not_parse_keeps_the_view() {
    let counting = Watched::new(false, None);
    let (_rig, host, vim, mut tui) =
        wrapped("inc-invalid", b"abc\nxab\n", &counting);
    let mut seen = Vec::new();
    // `a` parses and every key after it until the group closes does
    // not: `a\`, `a\(`, `a\(b` and `a\(b\` are each `Invalid`.
    for key in ["/", "a", "\\", "(", "b", "\\", ")"] {
        typed(&mut tui, key);
        views(&host, &mut seen);
        assert!(counting.opens.get() <= 1, "{key}: no second view");
    }
    assert_eq!(seen.len(), 1, "the one view, kept through every refusal");
    assert_eq!(counting.seeks.get(), 5, "a call a key, refused or not");
    assert_eq!(vim.position(), (1, 1), "and the group's match");
    assert_eq!(said(&mut tui), "/a\\(b\\)", "the line says no word");
}

#[test]
fn a_macro_of_a_search_opens_one_view() {
    // The macro's own line first: `"ayy` holds `/foo`, and its
    // newline is the `<CR>`.
    let (_rig, host, vim, mut tui) = over("inc-macro", b"/foo\nabc\nfoo\n");
    let mut seen = vec![held(&host)];
    typed(&mut tui, "\"ayy");
    noted(&host, &mut seen);
    assert_eq!(seen.len(), 1, "nothing has searched yet");
    typed(&mut tui, "@a");
    noted(&host, &mut seen);
    assert_eq!(seen.len(), 2, "one view for the whole run");
    assert_eq!(vim.position(), (0, 1), "and it landed");
}

#[test]
fn typing_never_waits_and_enter_does() {
    let (_rig, _host, vim, mut tui) = spending("inc-no-wait", &long(), 64);
    for key in ["/", "z", "z", "z"] {
        typed(&mut tui, key);
        assert!(!vim.waiting(), "{key}: typing never waits");
    }
    assert_eq!(said(&mut tui), "/zzz", "the status holds the line");
    assert_eq!(vim.position(), (0, 0), "nothing found yet");
    typed(&mut tui, "<CR>");
    assert!(vim.waiting(), "the Enter does");
    landed(&mut tui, &vim);
    assert_eq!(vim.position(), (2047, 0), "where the whole search lands");
}

#[test]
fn an_open_line_is_idle_once_answered() {
    let counting = Watched::new(false, None);
    let (_rig, _host, _vim, mut tui) =
        wrapped("inc-idle", b"abc\nfoo bar\n", &counting);
    for keys in ["/fo", "<Esc>:set hls<CR>/fo"] {
        typed(&mut tui, keys);
        let reads = counting.reads.get();
        for _ in 0..10 {
            assert_eq!(tui.tick().unwrap(), Step::Same, "{keys}: it stands");
        }
        assert_eq!(counting.reads.get(), reads, "{keys}: nothing is read");
    }
}

#[test]
fn hlsearch_marks_every_match_of_the_line_so_far() {
    let (_rig, host, _vim, mut tui) = over("inc-hls", b"ab\nb\nfb\nfc\n");
    // No pattern has been run yet, so what the marks are of is the
    // line and nothing else.
    typed(&mut tui, ":set hls<CR>/f");
    assert_eq!(marks(&host), [(5, 6), (5, 6), (8, 9)], "the line's own");
    typed(&mut tui, "<Esc>");
    assert_eq!(marks(&host), Vec::new(), "and gone with the line");
    typed(&mut tui, "/b<CR>");
    assert_eq!(marks(&host), [(1, 2), (3, 4), (6, 7)], "the last pattern's");
    typed(&mut tui, "/f");
    assert_eq!(
        marks(&host),
        [(5, 6), (5, 6), (8, 9)],
        "every match of the line so far, the preview's own once more"
    );
    typed(&mut tui, "<Esc>");
    assert_eq!(marks(&host), [(1, 2), (3, 4), (6, 7)], "the last pattern's");
}

/// One search run to the end of its list: the row, the cursor, the
/// views the Client holds, and where `n` goes after it — all a person
/// sees of a `/` line, and what the `search` name must leave alike.
///
/// `n` is the last pattern's own door: a name that searched without
/// setting it would move nowhere.
fn sought(
    name: &str,
    keys: &str,
) -> (String, (usize, usize), Vec<String>, (usize, usize)) {
    let (_rig, host, vim, mut tui) = over(name, b"foo one\nbar\nfoo two\n");
    typed(&mut tui, keys);
    let mut frames = 0;
    while vim.counted().is_none() {
        tui.tick().unwrap();
        frames += 1;
        assert!(
            frames < 500,
            "{keys}: the list never turned whole: {}",
            said(&mut tui)
        );
    }
    let row = said(&mut tui);
    let at = vim.position();
    let views = listed(&host, "/self/search");
    typed(&mut tui, "n");
    (row, at, views, vim.position())
}

/// `search` is a name on PATH and the `/` line is that name chosen:
/// `:search foo` does what `/foo` does, and `--backward` what `?`
/// does — the same cursor, the same last pattern, the same count row
/// and the same one view.
#[test]
fn search_the_name_is_the_slash_line() {
    assert_eq!(
        sought("search-name-forward", ":search foo<CR>"),
        sought("search-slash-forward", "/foo<CR>"),
    );
    assert_eq!(
        sought("search-name-backward", ":search --backward foo<CR>"),
        sought("search-question-backward", "?foo<CR>"),
    );
}

/// The name with no pattern opens the prompt the `/` key opens, so a
/// key bound to it is the `/` key: `bind <C-s> search` and `<C-s>foo`
/// is `/foo`.
#[test]
fn search_with_no_pattern_opens_the_prompt() {
    assert_eq!(
        sought("search-name-prompt", ":search<CR>foo<CR>"),
        sought("search-slash-prompt", "/foo<CR>"),
    );
    assert_eq!(
        sought("search-name-bound", ":bind <lt>C-s> search<CR><C-s>foo<CR>"),
        sought("search-slash-bound", "/foo<CR>"),
    );
}

/// The view is the stay's, whichever door asked for it: a `/` line
/// opens one and the name after it seeks that same one, as 04.8's
/// counting wrapper shows.
#[test]
fn search_is_one_view_a_stay_with_the_slash_line() {
    let counting = Watched::new(false, None);
    let (_rig, host, vim, mut tui) =
        wrapped("search-name-one-view", b"abc\nfoo abc\n", &counting);
    let mut seen = Vec::new();
    typed(&mut tui, "/a<CR>");
    views(&host, &mut seen);
    let view = held(&host);
    assert_eq!(vim.position(), (1, 4), "the `/` line landed");
    typed(&mut tui, ":search b<CR>");
    views(&host, &mut seen);
    let row = said(&mut tui);
    assert_eq!(vim.position(), (1, 5), "the name's line after it: {row}");
    assert_eq!(held(&host), view, "the one view, sought and not reopened");
    assert_eq!(seen.len(), 1, "one view across the two lines");
    assert_eq!(counting.opens.get(), 1, "opened once");
}

/// `:search`'s pattern is the rest of the line by its declaration: a
/// flag stands before it and is read once, and a word under a dash
/// past that is the pattern's own — which is what the `/` line does
/// with the same characters, the two being one door.
#[test]
fn a_search_pattern_is_the_rest_of_the_line() {
    let ran = |name: &str, keys: &str| {
        let (_rig, _host, vim, mut tui) =
            over(name, b"zz\nfoo --backward bar\n-x here\n");
        typed(&mut tui, keys);
        let mut frames = 0;
        while vim.counted().is_none() {
            tui.tick().unwrap();
            frames += 1;
            assert!(frames < 500, "{keys}: {}", said(&mut tui));
        }
        (said(&mut tui), vim.position())
    };
    assert_eq!(
        ran("search-rest-name", ":search foo --backward bar<CR>"),
        ran("search-rest-slash", "/foo --backward bar<CR>"),
    );
    assert_eq!(
        ran("search-dash-name", ":search -x<CR>"),
        ran("search-dash-slash", "/-x<CR>"),
    );
}
