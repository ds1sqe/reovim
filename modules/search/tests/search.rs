//! The search Module over the rig: `open`, `matches` and `progress`,
//! driven the way a Plugin drives any declared api. `Search` is
//! loaded bare, under no `Rig::host()`, so a case picks its own
//! budget.

use std::rc::Rc;
use std::sync::atomic::{AtomicU64, Ordering};

use reovim_client::{
    Calls, Clip, Handle, Host, HostCalls, Kind, Rd, Rights, Table, dev,
};
use reovim_client_api::buffers;
use reovim_kernel_interface::{Booted, Error};
use reovim_module_search::{Search, search};
use reovim_rig::Rig;
use reovim_sdk::walk::ROOT;
use reovim_sdk::{cell, ops, progress};
use reovim_search_api::entries;

/// A clock that moves, so `progress`'s `mtime` outruns `ctime`.
static CLOCK: AtomicU64 = AtomicU64::new(1);
fn tick() -> u64 {
    CLOCK.fetch_add(1, Ordering::Relaxed)
}

/// `search` loaded on a bare Host of its own, the rig's fs beside it.
struct Sut {
    rig: Rig,
    host: Rc<Host>,
    open: Rd,
}

/// A view as its opener holds it: the rd `open` answered and the
/// handle held on it, with its two children resolved.
struct View {
    rd: Rd,
    h: Handle,
    matches: Rd,
    progress: Rd,
}

impl Sut {
    /// A budget generous enough that any case's text fits in one
    /// slice.
    fn small(test: &str) -> Self {
        Self::with_budget(test, 1 << 20)
    }

    fn with_budget(test: &str, budget: usize) -> Self {
        let rig = Rig::new(test);
        let host = Rc::new(Host::over(rig.client(), tick));
        let driver = Rc::new(Search::with_budget(budget));
        let booted = Booted { module: driver.clone(), drivers: vec![driver] };
        host.load("search", booted.module, booted.drivers).unwrap();
        let open =
            cell::resolve(&host.own(), ROOT, "/self/search/open").unwrap();
        Self { rig, host, open }
    }

    fn calls(&self) -> HostCalls<'_> {
        self.host.own()
    }

    fn on<T>(&self, f: impl FnOnce(&mut &dyn Calls) -> T) -> T {
        let own = self.calls();
        let mut ctx: &dyn Calls = &own;
        f(&mut ctx)
    }

    /// `f` from `/module/fs/root/<name>`, written on disk first.
    fn source(&self, name: &str, content: &[u8]) -> Rd {
        self.rig.file(name, content);
        cell::resolve(&self.calls(), ROOT, &format!("/module/fs/root/{name}"))
            .unwrap()
    }

    /// `search.open`, on a handle opened and closed the way `call` is.
    fn open_call(&self, args: search::Open) -> Result<Rd, Error> {
        let h = self.calls().open(self.open, Rights::WRITE).unwrap();
        let result = self.on(|ctx| search::Call(ctx, h).open(args));
        let _ = ops::close(&self.calls(), h);
        result
    }

    /// A view held open, as the opener does; panics on a refusal.
    fn opened(&self, args: search::Open) -> View {
        let rd = self.open_call(args).unwrap();
        self.held(rd)
    }

    fn held(&self, rd: Rd) -> View {
        let h = self.calls().open(rd, Rights::READ).unwrap();
        let matches = cell::resolve(&self.calls(), rd, "matches").unwrap();
        let progress = cell::resolve(&self.calls(), rd, "progress").unwrap();
        View { rd, h, matches, progress }
    }

    /// One read, through a handle opened and closed here: closing a
    /// handle on `matches` or `progress` never ends the view.
    fn read(&self, rd: Rd, at: u64, len: u32) -> Result<Vec<u8>, Error> {
        let h = self.calls().open(rd, Rights::READ).unwrap();
        let out = ops::read(&self.calls(), h, at, len);
        let _ = ops::close(&self.calls(), h);
        out
    }

    fn close(&self, h: Handle) {
        let _ = ops::close(&self.calls(), h);
    }

    fn info(&self, rd: Rd) -> reovim_kernel_interface::Info {
        ops::info(&self.calls(), rd).unwrap()
    }

    fn progress(&self, rd: Rd) -> progress::Progress {
        progress::read(&self.calls(), rd).unwrap()
    }

    fn ls(&self, path: &str) -> Vec<String> {
        let rd = cell::resolve(&self.calls(), ROOT, path).unwrap();
        let mut names: Vec<String> = ops::children(&self.calls(), rd)
            .unwrap()
            .into_iter()
            .filter_map(|c| ops::info(&self.calls(), c).ok()?.name)
            .collect();
        names.sort();
        names
    }

    /// A watching handle on `rd`, kept open so the watch stays live.
    fn watch(&self, rd: Rd) -> Handle {
        ops::watch(&self.calls(), rd).unwrap()
    }

    /// `(rd, kind)` of the events after `cursor`, and the next cursor.
    fn since(&self, cursor: u64) -> (Vec<(Rd, Kind)>, u64) {
        let (events, cursor) = ops::since(&self.calls(), cursor).unwrap();
        (events.into_iter().map(|e| (e.rd, e.kind)).collect(), cursor)
    }
}

/// Every entry `matches` holds once whole: a read at a time until the
/// empty one, `Busy` retried; anything else panics, which is the
/// "every read is `Busy` or entries" rule made an assertion.
fn drained(sut: &Sut, matches: Rd) -> Vec<search::Match> {
    let mut found = Vec::new();
    let mut at = 0u64;
    loop {
        match sut.read(matches, at, 1 << 20) {
            Ok(bytes) if bytes.is_empty() => return found,
            Ok(bytes) => {
                at += bytes.len() as u64;
                found.extend(entries(&bytes));
            }
            Err(Error::Busy) => {}
            Err(error) => panic!("drain: {error:?}"),
        }
    }
}

fn args(source: Rd, pattern: &str, from: u64) -> search::Open {
    search::Open {
        source,
        pattern: pattern.to_string(),
        ignorecase: false,
        smartcase: false,
        from,
    }
}

/// `lines` lines of eleven bytes, ten `a`s and a `\n`, an `x` opening
/// every fifth line: `x` stands at `11 * n` for `n % 5 == 0`.
fn generated(lines: usize) -> String {
    let mut text = String::new();
    for n in 0..lines {
        let head = if n % 5 == 0 { 'x' } else { 'a' };
        text.push(head);
        text.extend(std::iter::repeat_n('a', 9));
        text.push('\n');
    }
    text
}

/// Every offset `x` stands at in [`generated`]'s own text of `lines`
/// lines, turned so it starts at `from_line`'s own line: the
/// unbounded, independent answer a fill over the same text and
/// `from` must match.
fn turned(lines: usize, from_line: u64) -> Vec<u64> {
    let start = from_line * 11;
    let all = (0..lines as u64).filter(|n| n % 5 == 0).map(|n| n * 11);
    let (before, after): (Vec<u64>, Vec<u64>) = all.partition(|&p| p < start);
    after.into_iter().chain(before).collect()
}

#[test]
fn a_small_cell_is_gone_through_inside_open() {
    let sut = Sut::small("small");
    let source = sut.source("f", b"one\ntwo\nthree\n");
    let view = sut.opened(args(source, "o", 0));
    let held = sut.read(view.matches, 0, 1 << 20).unwrap();
    let found = entries(&held);
    let ats: Vec<u64> = found.iter().map(|m| m.at).collect();
    assert_eq!(ats, [0, 6], "one and two each hold one o");
    assert!(found.iter().all(|m| m.over == source));
    let empty = sut.read(view.matches, held.len() as u64, 1 << 20).unwrap();
    assert!(empty.is_empty(), "the whole text was gone through inside open");
    let p = sut.progress(view.progress);
    assert_eq!(p.done, p.total);
}

#[test]
fn a_long_cell_fills_a_slice_a_read_from_from_with_the_wrap() {
    // About 80 KiB, so the forward pass alone crosses one window's
    // own 64 KiB. `from` mid-line, in the second chunk (past 64 KiB).
    const LINES: usize = 7450;
    const LINE: u64 = 6000;
    let text = generated(LINES);
    assert!(text.len() > 65536, "past one window: {}", text.len());
    let from = LINE * 11 + 5;
    assert!(from > 65536, "past one window: {from}");

    let expected = turned(LINES, LINE);

    let sut = Sut::with_budget("long", 500);
    let source = sut.source("f", text.as_bytes());
    let view = sut.opened(args(source, "x", from));
    assert_eq!(
        sut.progress(view.progress).total,
        text.len() as u64,
        "total is the source's own length"
    );

    let found = drained(&sut, view.matches);
    let ats: Vec<u64> = found.iter().map(|m| m.at).collect();
    assert_eq!(ats, expected, "the unbounded list, turned at from's line");
    assert!(found.iter().all(|m| m.len == 1 && m.over == source));
}

#[test]
fn progress_goes_to_total_and_its_mtime_moves_ctime_still() {
    let sut = Sut::with_budget("progress", 64);
    let text = generated(400);
    let source = sut.source("f", text.as_bytes());
    let view = sut.opened(args(source, "x", 0));
    let total = sut.progress(view.progress).total;
    let ctime0 = sut.info(view.progress).ctime;

    let mut at = 0u64;
    let mut dones = vec![sut.progress(view.progress).done];
    let mut mtimes = vec![sut.info(view.progress).mtime];
    loop {
        match sut.read(view.matches, at, 1 << 20) {
            Ok(bytes) if bytes.is_empty() => break,
            Ok(bytes) => at += bytes.len() as u64,
            Err(Error::Busy) => {}
            Err(other) => panic!("{other:?}"),
        }
        dones.push(sut.progress(view.progress).done);
        mtimes.push(sut.info(view.progress).mtime);
    }

    assert!(
        dones.windows(2).all(|w| w[0] <= w[1]),
        "done never goes back: {dones:?}"
    );
    assert!(
        dones.iter().all(|&d| d <= total),
        "done never passes total: {dones:?}"
    );
    assert_eq!(*dones.last().unwrap(), total, "whole once drained");
    let moved = mtimes.windows(2).filter(|w| w[1] > w[0]).count();
    assert!(moved >= 2, "mtime moves across at least two slices: {mtimes:?}");
    assert_eq!(
        sut.info(view.progress).ctime,
        ctime0,
        "ctime never moves again"
    );
}

#[test]
fn a_read_of_progress_alone_moves_no_time() {
    let sut = Sut::with_budget("progress-still", 64);
    let source = sut.source("f", generated(200).as_bytes());
    let view = sut.opened(args(source, "x", 0));
    let before = sut.info(view.progress);
    for _ in 0..5 {
        let _ = sut.progress(view.progress);
    }
    let after = sut.info(view.progress);
    assert_eq!(before.ctime, after.ctime, "a read of progress cranks nothing");
    assert_eq!(before.mtime, after.mtime, "a read of progress cranks nothing");
}

#[test]
fn no_modify_follows_once_the_fill_is_whole() {
    let sut = Sut::with_budget("whole-quiet", 64);
    let source = sut.source("f", generated(200).as_bytes());
    let view = sut.opened(args(source, "x", 0));
    let _watch = sut.watch(view.progress);

    let _found = drained(&sut, view.matches);
    // The empty read that found "whole" is itself one turn; its
    // events are not what is under judgment here.
    let (_, cursor) = sut.since(0);

    let past = sut.read(view.matches, 1 << 30, 1 << 20).unwrap();
    assert!(past.is_empty(), "whole stays whole");
    let _ = sut.progress(view.progress);

    let (events, _) = sut.since(cursor);
    let modified = events
        .iter()
        .any(|&(rd, kind)| rd == view.progress && kind == Kind::Modify);
    assert!(!modified, "{events:?}");
}

#[test]
fn the_close_of_the_view_ends_it() {
    let sut = Sut::small("close");
    let source = sut.source("f", b"one\ntwo\n");
    let before = sut.ls("/self/search");
    let view = sut.opened(args(source, "o", 0));
    assert_ne!(sut.ls("/self/search"), before, "the view is listed");

    // A `matches` handle closed first ends nothing.
    let bystander = sut.calls().open(view.matches, Rights::READ).unwrap();
    sut.close(bystander);
    assert_ne!(sut.ls("/self/search"), before, "still there");

    // A handle held across the close of the view sees it end.
    let held = sut.calls().open(view.matches, Rights::READ).unwrap();
    assert!(ops::info(&sut.calls(), view.rd).is_ok(), "the view stands");
    sut.close(view.h);
    assert_eq!(sut.ls("/self/search"), before, "the view is gone");
    assert_eq!(ops::info(&sut.calls(), view.rd), Err(Error::Invalid));
    assert_eq!(ops::read(&sut.calls(), held, 0, 16), Err(Error::Gone));
}

#[test]
fn a_bad_pattern_is_invalid_and_a_spent_offset_is_nospace() {
    let invalid = Sut::small("invalid");
    let source = invalid.source("f", b"vfoo\n");
    assert_eq!(invalid.open_call(args(source, "\\vx", 0)), Err(Error::Invalid));

    // The expensive line first: one attempt at `from` spends the
    // whole budget the matcher will give one offset, whatever the
    // slice's own budget is, so `open` itself answers `NoSpace` and
    // leaves nothing registered.
    let at_once = Sut::small("spent-open");
    let listing = at_once.ls("/self/search");
    let costly =
        at_once.source("costly", format!("{}\n", "a".repeat(30)).as_bytes());
    let refused = at_once.open_call(args(costly, "\\(a*\\)*b", 0));
    assert_eq!(refused, Err(Error::NoSpace));
    assert_eq!(at_once.ls("/self/search"), listing, "nothing left registered");

    // A budget of 1 tries exactly one offset a slice, so `open`
    // lands short of the costly line and a later read hits it.
    let staged = Sut::with_budget("spent-read", 1);
    let text = format!("z\n{}\n", "a".repeat(30));
    let source = staged.source("staged", text.as_bytes());
    let view = staged.opened(args(source, "\\(a*\\)*b", 0));
    let mut refusal = None;
    for _ in 0..200 {
        match staged.read(view.matches, 0, 1 << 20) {
            Err(Error::NoSpace) => {
                refusal = Some(());
                break;
            }
            Err(Error::Busy) | Ok(_) => {}
            Err(other) => panic!("{other:?}"),
        }
    }
    assert!(refusal.is_some(), "NoSpace was never reached");
    assert_eq!(
        staged.read(view.matches, 0, 1 << 20),
        Err(Error::NoSpace),
        "answered again"
    );
}

#[test]
fn a_match_found_before_a_spent_offset_in_the_same_slice_stands() {
    // `from` past both lines, so `open` goes forward over the filler
    // alone; the filler is long enough that a single budget cannot
    // cross it, so several reads pass before one of them finishes it
    // and, with budget still to spend, wraps into `aaab` and the
    // costly line right after it, in that one slice.
    let filler = "z".repeat(6000);
    let text = format!("aaab\n{}\n{filler}\n", "a".repeat(30));
    let from = 5 + 31;
    let sut = Sut::with_budget("kept-on-spent", 4000);
    let source = sut.source("f", text.as_bytes());
    let view = sut.opened(args(source, "\\(a*\\)*b", from));

    let mut at = 0u64;
    let mut refused = false;
    for _ in 0..2000 {
        match sut.read(view.matches, at, 1 << 20) {
            Err(Error::NoSpace) => {
                refused = true;
                break;
            }
            Ok(bytes) if !bytes.is_empty() => at += bytes.len() as u64,
            Ok(_) | Err(Error::Busy) => {}
            Err(other) => panic!("{other:?}"),
        }
    }
    assert!(refused, "NoSpace was never reached");

    let held = sut.read(view.matches, 0, 1 << 20).unwrap();
    let found: Vec<u64> = entries(&held).iter().map(|m| m.at).collect();
    assert_eq!(found, [0], "the aaab match survives the later Spent");
}

#[test]
fn a_source_edited_between_slices_does_not_panic() {
    let sut = Sut::with_budget("edited", 8);
    let text = generated(40);
    let source = sut.source("f", text.as_bytes());
    let view = sut.opened(args(source, "x", 0));

    // Shrunk under the fill: further slices must not panic, and the
    // fill still ends.
    sut.rig.file("f", b"short\n");
    let mut at = 0u64;
    let mut steps = 0;
    loop {
        steps += 1;
        assert!(steps < 10_000, "the fill never ended");
        match sut.read(view.matches, at, 1 << 20) {
            Ok(bytes) if bytes.is_empty() => break,
            Ok(bytes) => at += bytes.len() as u64,
            Err(Error::Busy) => {}
            Err(other) => panic!("unexpected: {other:?}"),
        }
    }

    // Not UTF-8 past the wrap: `from` clears the bad first line, so
    // `open` succeeds over the valid rest and only the wrap reads the
    // bad byte back, refused rather than a panic.
    let mut bad_bytes = vec![0xFFu8, b'\n'];
    bad_bytes.extend(std::iter::repeat_n(&b"aaa\n"[..], 10).flatten().copied());
    sut.rig.file("f", &bad_bytes);
    let over = sut.opened(args(source, "x", 2));
    let mut at = 0u64;
    let mut bad = None;
    for _ in 0..2000 {
        match sut.read(over.matches, at, 1 << 20) {
            Err(Error::Invalid) => {
                bad = Some(());
                break;
            }
            Ok(bytes) => at += bytes.len() as u64,
            Err(Error::Busy) => {}
            Err(other) => panic!("{other:?}"),
        }
    }
    assert!(bad.is_some(), "the bad byte was never read");
    assert_eq!(
        sut.read(over.matches, at, 1 << 20),
        Err(Error::Invalid),
        "answered again"
    );
}

#[test]
fn a_paused_offset_is_rounded_to_a_char_boundary_after_a_rewrite() {
    // A budget of 1 tries exactly one offset a slice, so `next`
    // stands still at whatever a read leaves it, one attempt at a
    // time. `open`'s own first slice already moves it once.
    let sut = Sut::with_budget("char-boundary", 1);
    let text = format!("{}\n{}\n", "a".repeat(60), "a".repeat(60));
    let source = sut.source("f", text.as_bytes());
    let view = sut.opened(args(source, "zzzz", 0));
    for _ in 0..8 {
        assert_eq!(sut.read(view.matches, 0, 1 << 20), Err(Error::Busy));
    }

    // `next` now stands at 9 (`open`'s own slice plus the eight
    // above), an odd offset. Rewritten as two-byte characters, where
    // only an even offset is a boundary, 9 falls inside one.
    let multibyte = format!("{}\n{}\n", "é".repeat(60), "é".repeat(60));
    sut.rig.file("f", multibyte.as_bytes());

    // Must not panic, and the fill still ends (`Busy`, entries, or
    // the empty read once whole; never anything else) — cranked well
    // past where the rewritten text's own last char stands, so the
    // char-boundary case is hit more than once on the way.
    let mut at = 0u64;
    let mut steps = 0;
    loop {
        steps += 1;
        assert!(steps < 20_000, "the fill never ended");
        match sut.read(view.matches, at, 1 << 20) {
            Ok(bytes) if bytes.is_empty() => break,
            Ok(bytes) => at += bytes.len() as u64,
            Err(Error::Busy) => {}
            Err(other) => panic!("unexpected: {other:?}"),
        }
    }
}

#[test]
fn two_views_fill_apart() {
    const LINES: usize = 200;
    const FROM_LINE: u64 = 100;
    let sut = Sut::with_budget("two", 32);
    let text = generated(LINES);
    let source = sut.source("f", text.as_bytes());
    let one = sut.opened(args(source, "x", 0));
    let two = sut.opened(args(source, "x", FROM_LINE * 11));

    let expected_one = turned(LINES, 0);
    let expected_two = turned(LINES, FROM_LINE);

    let found_one = drained(&sut, one.matches);
    let found_two = drained(&sut, two.matches);
    let ats_one: Vec<u64> = found_one.iter().map(|m| m.at).collect();
    let ats_two: Vec<u64> = found_two.iter().map(|m| m.at).collect();
    assert_eq!(ats_one, expected_one, "the first view starts at 0");
    assert_eq!(ats_two, expected_two, "the second turns at its own from");
}

#[test]
fn an_empty_match_ends_a_window_without_a_final_newline() {
    // No trailing `\n`: the window is unterminated, so `upto` is the
    // window's own length, inclusive, and an empty match may stand
    // exactly there.
    let sut = Sut::small("empty-match");
    let source = sut.source("f", b"ab");
    let view = sut.opened(args(source, "x*", 0));
    let found = drained(&sut, view.matches);
    let ats: Vec<(u64, u64)> = found.iter().map(|m| (m.at, m.len)).collect();
    assert_eq!(
        ats,
        [(0, 0), (1, 0), (2, 0)],
        "one empty match at every offset"
    );
}

#[test]
fn a_multibyte_source_finds_its_matches_by_byte_offset() {
    let sut = Sut::small("multibyte");
    let text = "café\nmore café\n";
    let source = sut.source("f", text.as_bytes());
    let view = sut.opened(args(source, "é", 0));
    let found = drained(&sut, view.matches);
    let ats: Vec<(u64, u64)> = found.iter().map(|m| (m.at, m.len)).collect();
    let expected: Vec<(u64, u64)> = text
        .match_indices('é')
        .map(|(i, m)| (i as u64, m.len() as u64))
        .collect();
    assert_eq!(ats, expected);
}

#[test]
fn total_is_the_source_length_at_every_boundary_size() {
    for size in [0usize, 1, 65536, 65537] {
        let sut = Sut::small(&format!("size-{size}"));
        let source = sut.source("f", &vec![b'a'; size]);
        let view = sut.opened(args(source, "zzz", 0));
        let total = sut.progress(view.progress).total;
        assert_eq!(total, size as u64, "size {size}");
    }
}

/// The Client's own draft, over nothing: `dev::boot` and a scratch
/// buffer, as a Plugin makes one.
fn scratch(host: &Rc<Host>) -> Rd {
    let table: Table = Box::new(|_| None);
    let dev = dev::boot(host, table, Clip::none(), "", None, None).unwrap();
    let list = cell::resolve(&host.own(), dev, "buffers").unwrap();
    let own = host.own();
    let mut ctx: &dyn Calls = &own;
    let h = ctx.open(list, Rights::WRITE).unwrap();
    let rd = buffers::Call(&mut ctx, h).scratch().unwrap();
    let _ = ops::close(&own, h);
    rd
}

#[test]
fn a_view_over_a_draft_sends_nothing() {
    let sut = Sut::small("draft");
    let draft = scratch(&sut.host);

    let before = sut.rig.sent();
    let view = sut.opened(args(draft, "h", 0));
    let _ = drained(&sut, view.matches);
    let _ = sut.info(view.progress);
    let _ = sut.progress(view.progress);
    assert_eq!(sut.rig.sent(), before, "a draft never leaves the Client");
}
