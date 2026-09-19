//! The fill: whole lines of a source, read a window at a time, from
//! the start of one line around to itself.
//!
//! Nothing here indexes the source, which may have changed under any
//! read: every window is read fresh, and a slice picks up only from
//! the two offsets [`Fill`] itself keeps. A window reads one chunk
//! ahead of the line it holds, so a match that starts in it may run
//! into the next; a match may not run further than that, one chunk
//! past its own window and no more. A window has no cap on finding
//! its own `\n`, but one line may not cost more than
//! [`reovim_search_api::LINE`]: past it the fill fails `NoSpace` for
//! good, a Driver's own limit, since cutting the window mid-line
//! would fake `^` and `$` at the cut. The read ahead stands outside
//! that cap.

use alloc::string::String;
use alloc::vec::Vec;

use reovim_kernel_interface::{Calls, Error, Handle, Rd};
use reovim_pattern::{Pattern, Scan};
use reovim_sdk::ops;
use reovim_search_api::search;

/// Bytes asked for per read.
const CHUNK: u32 = 64 * 1024;

/// Where a fill stands: the source it goes through, the pattern it
/// runs, and the two offsets that say where a window begins and where
/// the scan inside it goes on.
pub struct Fill {
    pattern: Pattern,
    source: Rd,
    handle: Handle,
    /// The start of `from`'s line, where a wrap ends.
    start: u64,
    total: u64,
    /// Where the current window begins.
    line: u64,
    /// Where the scan goes on, at or after `line`.
    next: u64,
    /// The end has been passed once; a window's own bound is now no
    /// further than `start`, though what it reads ahead may run past
    /// it.
    wrapped: bool,
    /// The end has been passed twice: the fill is whole.
    whole: bool,
}

impl Fill {
    pub const fn new(
        pattern: Pattern,
        source: Rd,
        handle: Handle,
        start: u64,
        total: u64,
    ) -> Self {
        Self {
            pattern,
            source,
            handle,
            start,
            total,
            line: start,
            next: start,
            wrapped: false,
            whole: false,
        }
    }

    pub const fn handle(&self) -> Handle {
        self.handle
    }

    pub const fn whole(&self) -> bool {
        self.whole
    }

    /// `(done, total)`: `done` the fill's own position, turned into a
    /// distance from `start` around to itself, so it is `total` once
    /// whole, a shrunk source included.
    pub fn progress(&self) -> (u64, u64) {
        if self.whole {
            return (self.total, self.total);
        }
        let done = if self.wrapped {
            (self.total - self.start) + self.next.min(self.start)
        } else {
            self.next - self.start
        };
        (done.min(self.total), self.total)
    }
}

/// One slice: matches found until `budget` runs out or the fill
/// reaches its own end, whichever comes first; a window or a scan
/// past it may fail (`NoSpace` for a scan or a line past the cap,
/// `Invalid` for a window that is not UTF-8), and whatever was
/// already found stands either way.
pub fn slice(
    fill: &mut Fill,
    ctx: &dyn Calls,
    budget: usize,
) -> (Vec<search::Match>, Result<(), Error>) {
    let mut found = Vec::new();
    let outcome = run(fill, ctx, budget, &mut found);
    (found, outcome)
}

/// [`slice`]'s own loop, `found` grown as it goes so it stands
/// whatever this answers: a plain `Result` here, so an error can
/// still use `?`.
fn run(
    fill: &mut Fill,
    ctx: &dyn Calls,
    budget: usize,
    found: &mut Vec<search::Match>,
) -> Result<(), Error> {
    let mut budget = budget;
    loop {
        if budget == 0 {
            return Ok(());
        }
        if fill.wrapped && fill.line >= fill.start {
            fill.whole = true;
            return Ok(());
        }
        match window(ctx, fill.handle, fill.line)? {
            Some((text, natural)) => {
                let (bound, upto) = bounded(fill, natural, &text);
                through(fill, &text, bound, upto, &mut budget, found)?;
            }
            None if fill.wrapped => {
                fill.whole = true;
                return Ok(());
            }
            None => {
                fill.wrapped = true;
                fill.line = 0;
                fill.next = 0;
            }
        }
    }
}

/// The window's own bound, and `upto`, how far inside it a match may
/// start. Once wrapped the bound stops at `start` instead of the
/// window's own `\n`, the read not knowing where the wrap falls and
/// may hold more; `upto` stops at the bound's own `\n`, so an entry
/// at `start` stays the opening pass's.
fn bounded(fill: &Fill, natural: usize, text: &str) -> (usize, usize) {
    let bound = if fill.wrapped {
        usize::try_from(fill.start - fill.line)
            .map_or(natural, |s| s.min(natural))
    } else {
        natural
    };
    let upto = bound - usize::from(text.as_bytes()[bound - 1] == b'\n');
    (bound, upto)
}

/// Every match `budget` covers inside `text`, up to `upto`, `bound`
/// carried along for [`close`].
fn through(
    fill: &mut Fill,
    text: &str,
    bound: usize,
    upto: usize,
    budget: &mut usize,
    found: &mut Vec<search::Match>,
) -> Result<(), Error> {
    while once(fill, text, upto, bound, budget, found)? {}
    Ok(())
}

/// One `scan` inside `text`, from where the last one left off: `true`
/// while the window has more worth trying. `next` is rounded up to a
/// char boundary first, since the source may have changed under the
/// read that paused here last.
fn once(
    fill: &mut Fill,
    text: &str,
    upto: usize,
    bound: usize,
    budget: &mut usize,
    found: &mut Vec<search::Match>,
) -> Result<bool, Error> {
    let raw = usize::try_from(fill.next - fill.line).unwrap_or(0);
    let from = text.ceil_char_boundary(raw);
    let scanned = fill
        .pattern
        .scan(text, from, upto, budget)
        .map_err(|_| Error::NoSpace)?;
    match scanned {
        Scan::Match(start, end, _) => {
            Ok(matched(fill, text, bound, start, end, found))
        }
        Scan::None => {
            close(fill, bound);
            Ok(false)
        }
        Scan::Paused(at) => {
            paused(fill, text, at);
            Ok(false)
        }
    }
}

/// A match at `[start, end)` inside `text`: on past its first `\n`
/// when one stands before its last byte, so the line after is
/// anchored too, as nvim finds it; at its own end otherwise, or a
/// char on for an empty one. An empty match unable to move past the
/// text's own end closes the window instead of looping on it.
fn matched(
    fill: &mut Fill,
    text: &str,
    bound: usize,
    start: usize,
    end: usize,
    found: &mut Vec<search::Match>,
) -> bool {
    found.push(search::Match {
        over: fill.source,
        at: fill.line + start as u64,
        len: (end - start) as u64,
    });
    let advance = if end == start {
        onward(text, end)
    } else {
        onto_the_next_line(text, start, end)
    };
    fill.next = fill.line + advance as u64;
    if advance <= start {
        close(fill, bound);
        return false;
    }
    true
}

/// Past the first `\n` inside `[start, end)`, unless it stands at the
/// match's own last byte, in which case `end` itself: nvim anchors
/// every line, `a\na` over `a`, `a`, `a`, `b` being two matches.
fn onto_the_next_line(text: &str, start: usize, end: usize) -> usize {
    let held = &text.as_bytes()[start..end];
    match held.iter().position(|&b| b == b'\n') {
        Some(at) if at + 1 < held.len() => start + at + 1,
        _ => end,
    }
}

/// The window ended: the next one begins right past it.
const fn close(fill: &mut Fill, bound: usize) {
    let end = fill.line + bound as u64;
    fill.line = end;
    fill.next = end;
}

/// The budget ran out at `at` inside `text`: `next` resumes there,
/// and `line` moves up to that offset's own line, so the next window
/// re-reads only from there on.
fn paused(fill: &mut Fill, text: &str, at: usize) {
    let restart = text[..at].rfind('\n').map_or(0, |p| p + 1);
    fill.next = fill.line + at as u64;
    fill.line += restart as u64;
}

/// The source's byte length, found with one-byte reads alone: `Info`
/// carries no size, and a slice at a time is all a window ever asks
/// for. `at` doubles from 64 KiB while a byte answers, then halves
/// down to the exact end.
pub fn measured(ctx: &dyn Calls, h: Handle) -> Result<u64, Error> {
    let has = |at: u64| -> Result<bool, Error> {
        Ok(!ops::read(ctx, h, at, 1)?.is_empty())
    };
    if !has(0)? {
        return Ok(0);
    }
    let mut lo = 0u64;
    let mut hi = u64::from(CHUNK);
    while has(hi)? {
        lo = hi;
        hi = hi.checked_mul(2).ok_or(Error::NoSpace)?;
    }
    while hi - lo > 1 {
        let mid = lo + (hi - lo) / 2;
        if has(mid)? { lo = mid } else { hi = mid }
    }
    Ok(hi)
}

/// The start of the line holding `from`, clamped to `total`: read
/// back in chunks until a `\n` turns up, or there is nothing before.
pub fn line_start(
    ctx: &dyn Calls,
    h: Handle,
    from: u64,
    total: u64,
) -> Result<u64, Error> {
    let mut at = from.min(total);
    while at > 0 {
        let len = CHUNK.min(u32::try_from(at).unwrap_or(CHUNK));
        let start = at - u64::from(len);
        let bytes = ops::read(ctx, h, start, len)?;
        if let Some(pos) = bytes.iter().rposition(|&b| b == b'\n') {
            return Ok(start + pos as u64 + 1);
        }
        at = start;
    }
    Ok(0)
}

/// Past `bytes`'s own last `\n`, or its length where it holds none.
fn cut(bytes: &[u8]) -> usize {
    bytes.iter().rposition(|&b| b == b'\n').map_or(bytes.len(), |p| p + 1)
}

/// One window from `line`, its line plus one chunk more: a match that
/// starts in the line may run into what follows, and the extra read
/// costs no step of the fill's own budget, standing outside its cap.
/// The bound answered is the line alone, for [`bounded`]; the text is
/// longer where the read ahead found more, except at the source's
/// true end, where its last line stands whole with no `\n` to cut at.
fn window(
    ctx: &dyn Calls,
    h: Handle,
    line: u64,
) -> Result<Option<(String, usize)>, Error> {
    let cap = usize::try_from(reovim_search_api::LINE).unwrap_or(usize::MAX);
    let mut bytes = Vec::new();
    let mut ended = false;
    loop {
        let at = line + bytes.len() as u64;
        let chunk = ops::read(ctx, h, at, CHUNK)?;
        if chunk.is_empty() {
            ended = true;
            break;
        }
        let held_nl = chunk.contains(&b'\n');
        bytes.extend(chunk);
        if bytes.len() > cap {
            return Err(Error::NoSpace);
        }
        if held_nl {
            break;
        }
    }
    if bytes.is_empty() {
        return Ok(None);
    }
    let bound = if ended { bytes.len() } else { cut(&bytes) };
    if !ended {
        let at = line + bytes.len() as u64;
        let ahead = ops::read(ctx, h, at, CHUNK)?;
        if ahead.is_empty() {
            ended = true;
        } else {
            bytes.extend(ahead);
        }
    }
    if !ended {
        bytes.truncate(cut(&bytes));
    }
    String::from_utf8(bytes)
        .map(|text| Some((text, bound)))
        .map_err(|_| Error::Invalid)
}

/// `at`, past its char; unmoved at the text's end, a char's length
/// there being 0.
fn onward(text: &str, at: usize) -> usize {
    at + text[at..].chars().next().map_or(0, char::len_utf8)
}
