//! The fill: whole lines of a source, read a window at a time, from
//! the start of one line around to itself.
//!
//! Nothing here indexes the source, which may have changed under any
//! read: every window is read fresh, and a slice picks up only from
//! the two offsets [`Fill`] itself keeps. A window has no cap on
//! finding its own `\n`, but one line may not cost more than [`LINE`]
//! windows: past it the fill fails `NoSpace` for good, a Driver's own
//! limit, since cutting the window mid-line would fake `^` and `$`
//! at the cut.

use alloc::string::String;
use alloc::vec::Vec;

use reovim_kernel_interface::{Calls, Error, Handle, Rd};
use reovim_pattern::{Pattern, Scan};
use reovim_sdk::ops;
use reovim_search_api::search;

/// Bytes asked for per window read.
const CHUNK: u32 = 64 * 1024;
/// A window may grow this many chunks looking for a `\n`.
const LINE: usize = 16;

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
    /// The end has been passed once; a window is now bounded by
    /// `start`, never read past.
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
/// past it may fail (`NoSpace` for a scan or a line past [`LINE`]
/// windows long, `Invalid` for a window that is not UTF-8), and
/// whatever was already found stands either way.
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
        let end = if fill.wrapped { fill.start } else { u64::MAX };
        match window(ctx, fill.handle, fill.line, end)? {
            Some(text) => through(fill, &text, &mut budget, found)?,
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

/// Every match `budget` covers inside `text`, appended to `found` in
/// the order they turn up.
fn through(
    fill: &mut Fill,
    text: &str,
    budget: &mut usize,
    found: &mut Vec<search::Match>,
) -> Result<(), Error> {
    let upto = text.len() - usize::from(text.ends_with('\n'));
    while once(fill, text, upto, budget, found)? {}
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
            Ok(matched(fill, text, start, end, found))
        }
        Scan::None => {
            close(fill, text);
            Ok(false)
        }
        Scan::Paused(at) => {
            paused(fill, text, at);
            Ok(false)
        }
    }
}

/// A match at `[start, end)` inside `text`, on from its end, or one
/// char on for an empty one; an empty match unable to move past the
/// text's own end closes the window instead of looping on it.
fn matched(
    fill: &mut Fill,
    text: &str,
    start: usize,
    end: usize,
    found: &mut Vec<search::Match>,
) -> bool {
    found.push(search::Match {
        over: fill.source,
        at: fill.line + start as u64,
        len: (end - start) as u64,
    });
    let advance = if end == start { onward(text, end) } else { end };
    fill.next = fill.line + advance as u64;
    if advance <= start {
        close(fill, text);
        return false;
    }
    true
}

/// The window ended: the next one begins right past it.
const fn close(fill: &mut Fill, text: &str) {
    let end = fill.line + text.len() as u64;
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

/// One window from `line`, read up to `end` (`u64::MAX` before the
/// wrap): whole lines, [`CHUNK`] at a time until they hold a `\n` or
/// `end` is reached, or there is nothing more. `None` at an empty
/// read, the fill's own end. Cut after the last `\n`, or, at `end`,
/// there exactly.
fn window(
    ctx: &dyn Calls,
    h: Handle,
    line: u64,
    end: u64,
) -> Result<Option<String>, Error> {
    let mut bytes = Vec::new();
    let mut chunks = 0usize;
    loop {
        let at = line + bytes.len() as u64;
        let want = u32::try_from(end.saturating_sub(at).min(u64::from(CHUNK)))
            .unwrap_or(CHUNK);
        if want == 0 {
            break;
        }
        let chunk = ops::read(ctx, h, at, want)?;
        if chunk.is_empty() {
            break;
        }
        let held_nl = chunk.contains(&b'\n');
        bytes.extend(chunk);
        chunks += 1;
        if chunks > LINE {
            return Err(Error::NoSpace);
        }
        if held_nl {
            break;
        }
    }
    if bytes.is_empty() {
        return Ok(None);
    }
    if let Some(pos) = bytes.iter().rposition(|&b| b == b'\n') {
        bytes.truncate(pos + 1);
    }
    String::from_utf8(bytes).map(Some).map_err(|_| Error::Invalid)
}

/// `at`, past its char; unmoved at the text's end, a char's length
/// there being 0.
fn onward(text: &str, at: usize) -> usize {
    at + text[at..].chars().next().map_or(0, char::len_utf8)
}
