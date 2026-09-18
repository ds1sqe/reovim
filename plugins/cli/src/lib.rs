#![no_std]
#![forbid(unsafe_code)]
//! The cli buffer: a shell in a region of its own; the `Plugin` of
//! clients/core.
//!
//! It knows no Module and no name: every line is run by clients/sh,
//! the same `run` the process cli runs and the same `complete`, over
//! the one door the Client hands it and from where a `Shell` of its
//! own stands.
//! Nothing here is a second line editor either; the keys go to the
//! shell's own `Line`, history and all.
//!
//! What it owns is a scratch Buffer holding the transcript: every line
//! run as `<path> > <line>` with what it printed under it, and last
//! the prompt with the line being typed. One `text.replace` of the
//! whole text per change, as the explorer refills its listing, so a
//! key is one revision and the View is laid out over the Buffer the
//! Client already reads.
//!
//! Leaving is `back` on the `open` capability: the region shows again
//! what it showed before this buffer, which is the Client's to do.

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use reovim_client::{
    BAR, BLUE, BOLD, Calls, Caps, Ctx, DIM, Error, Event, GREEN, Handle, Hash,
    Key, Plugin, RED, RED_BOLD, Rd, Rights, Start, YELLOW, grid, keys, open,
};
use reovim_client_sh::{Act, Line, Ran, Shell, complete, run};
use reovim_sdk::walk::ROOT;
use reovim_sdk::{cell, ops};
use reovim_text_api::{open as text_open, text};

/// A shell in a region: the transcript, the line being typed at the
/// prompt under it, and where that line stands.
///
/// `start` takes the rd the region was opened on and stands the Shell
/// there; everything after it is what has been typed since.
#[derive(Default)]
pub struct Cli {
    /// The scratch Buffer the transcript is in, and the handle on it.
    buffer: Option<Rd>,
    h: Option<Handle>,
    /// Where the line stands: what `cd` moves, what a relative target
    /// is read from, and what the prompt says.
    shell: Shell,
    /// The line being typed, with the lines submitted before it.
    line: Line,
    /// What has been run and answered, above the prompt, with the SGR
    /// escapes a Driver wrote taken out of it.
    transcript: String,
    /// Where those escapes stood, as spans of the Buffer's bytes: what
    /// the View carries for the Client to color.
    spans: Vec<grid::Span>,
    /// What the Buffer holds, as the last fill wrote it: what the next
    /// one replaces, and what says a fill would write nothing.
    text: String,
    /// The region's columns, as `grid` last said: what a listing lays
    /// its names across.
    cols: u64,
    /// Its rows, which the transcript scrolls in.
    rows: usize,
    region: Option<Handle>,
    opener: Option<Handle>,
}

impl Plugin for Cli {
    /// A scratch Buffer of its own, the Shell stood where the region
    /// was opened, and the first prompt shown.
    ///
    /// # Errors
    /// What a call it made says.
    fn start(&mut self, ctx: Ctx<'_>, at: Start) -> Result<(), Error> {
        let Ctx { ctx, caps } = ctx;
        let buffer = scratch(ctx)?;
        let rights = Rights::READ.union(Rights::WRITE);
        self.buffer = Some(buffer);
        self.h = Some(ctx.open(buffer, rights)?);
        self.region = caps.open(grid::hash());
        self.opener = caps.open(open::hash());
        if let Some(region) = self.region {
            let size = grid::Call(caps, region).size()?;
            self.cols = size.cols;
            self.rows = count(size.rows);
        }
        self.screen();
        self.stand(ctx, at.over);
        self.fill(ctx)?;
        self.show(caps)
    }

    /// A key typed at the prompt, or a resize taken for what the View
    /// is laid out over. An input of anything else is not the cli's.
    ///
    /// # Errors
    /// As [`Self::start`].
    fn input(
        &mut self,
        ctx: Ctx<'_>,
        from: Hash,
        bytes: &[u8],
    ) -> Result<(), Error> {
        let Ctx { ctx, caps } = ctx;
        if from == grid::hash() {
            let resized: grid::Resized = reovim_sdk::decode(bytes)?;
            self.cols = resized.cols;
            self.rows = count(resized.rows);
            self.screen();
            return self.show(caps);
        }
        if from != keys::hash() {
            return Ok(());
        }
        let press: keys::Press = reovim_sdk::decode(bytes)?;
        let Some(key) = Key::of(press.key) else { return Ok(()) };
        self.pressed(ctx, caps, key)
    }

    /// Nothing: the cli buffer watches no rd of its own, and its own
    /// Buffer moves only where it wrote it. A Driver that follows
    /// prints what it has now and lands here at milestone 2.
    ///
    /// # Errors
    /// Never.
    fn changed(&mut self, _: Ctx<'_>, _: &[Event]) -> Result<(), Error> {
        Ok(())
    }
}

impl Cli {
    /// The Buffer `start` opened; `Closed` before it has.
    fn handle(&self) -> Result<Handle, Error> {
        self.h.ok_or(Error::Closed)
    }

    /// Where the line starts: the rd the region was opened on, named
    /// to the shell's own `cd` as `rd:<n>`, so the path the prompt
    /// shows is the one the shell itself finds and no path is spelled
    /// here. A target that is no Container is that `cd`'s own refusal,
    /// and the line stands at the root with nothing said.
    fn stand(&mut self, ctx: &dyn Calls, over: Option<Rd>) {
        let Some(rd) = over else { return };
        let _ = run(ctx, &mut self.shell, &format!("cd rd:{}", rd.0));
    }

    /// What a run writes for: the region's columns, which a listing
    /// lays its names across, and color, which the region has the rows
    /// to show and [`read`] takes back out of the text.
    fn screen(&mut self) {
        self.shell.screen(self.cols.max(1), true);
    }

    /// One key, which is the shell's line editor's: its acts, and the
    /// two that leave the region are `back`.
    fn pressed(
        &mut self,
        ctx: &dyn Calls,
        caps: &mut Caps,
        key: Key,
    ) -> Result<(), Error> {
        match self.line.key(key) {
            Act::Nothing => return Ok(()),
            // The text or the cursor moved; the prompt line holds it.
            Act::Redraw => {}
            Act::Submit(line) => self.ran(ctx, &line),
            Act::Complete => {
                let found = complete(ctx, &self.shell, self.line.text());
                self.line.apply(&found);
            }
            // Ctrl-L: what has been run and answered goes, and the
            // line being typed stands where it stood.
            Act::Clear => {
                self.transcript.clear();
                self.spans.clear();
            }
            // Ctrl-C, and Ctrl-D on an empty line.
            Act::Quit => return self.leave(caps),
        }
        self.fill(ctx)?;
        self.show(caps)
    }

    /// One line submitted: the prompt and the line into the
    /// transcript, then what the line printed under it. An empty line
    /// is a new prompt and nothing else.
    ///
    /// A run that came back open is a Driver that follows: what its
    /// cell holds at this one read is what the buffer shows, and the
    /// cell is closed after it, a Plugin having no tick to read it
    /// again.
    fn ran(&mut self, ctx: &dyn Calls, line: &str) {
        // The prompt the line was typed at, which a `cd` in it moves.
        self.transcript.push_str(&self.shell.prompt());
        self.transcript.push_str(line);
        self.transcript.push('\n');
        if line.trim().is_empty() {
            return;
        }
        let said = match run(ctx, &mut self.shell, line) {
            Ok(ran) => printed(ctx, ran),
            Err(miss) => format!("{miss}\n"),
        };
        self.said(&said);
    }

    /// What a line printed, under it: its own lines with the escapes
    /// taken out and a span where each run of one stood, and a last
    /// line that ends in no newline gets one, the prompt following it.
    fn said(&mut self, out: &str) {
        if out.is_empty() {
            return;
        }
        let (text, spans) = read(out, self.transcript.len() as u64);
        self.transcript.push_str(&text);
        self.spans.extend(spans);
        if !text.ends_with('\n') {
            self.transcript.push('\n');
        }
    }

    /// Ctrl-C, or Ctrl-D on an empty line: the region goes back to
    /// what it showed before this buffer, which is the Client's to do
    /// and the end of this Plugin.
    fn leave(&self, caps: &mut Caps) -> Result<(), Error> {
        let Some(opener) = self.opener else { return Ok(()) };
        open::Call(caps, opener).back()
    }

    /// The Buffer filled with what the region shows now, whole, so a
    /// change is one revision. A key that moved the cursor alone
    /// leaves the text as it was and writes nothing.
    fn fill(&mut self, mut ctx: &dyn Calls) -> Result<(), Error> {
        let text = format!(
            "{}{}{}",
            self.transcript,
            self.shell.prompt(),
            self.line.text()
        );
        if text == self.text {
            return Ok(());
        }
        let was = core::mem::replace(&mut self.text, text);
        let edit = text::Edit {
            at: 0,
            len: was.len() as u64,
            text: self.text.clone(),
        };
        text::Call(&mut ctx, self.handle()?).replace(edit)
    }

    /// The View to the region, if the Client has one.
    fn show(&self, caps: &mut Caps) -> Result<(), Error> {
        let Some(region) = self.region else { return Ok(()) };
        let view = self.view();
        grid::Call(caps, region).show(view)
    }

    /// What to show: the cursor where the line is being typed, the
    /// prompt line the last of the rows the region has, and nothing in
    /// the status row, which the frame already names.
    fn view(&self) -> grid::View {
        let top = self.top() as u64;
        grid::View {
            over: self.buffer.map_or(0, |buffer| buffer.0),
            top,
            cursor: self.cursor() as u64,
            // The line is always being typed into, so the cursor
            // stands before the char it is on, as a prompt's does.
            shape: BAR,
            spans: self.shown(top),
            status: String::new(),
        }
    }

    /// The spans from the first byte shown, as Vim's are: a View
    /// carries a bounded sequence, and one above `top` has no cell of
    /// the region to stand on.
    fn shown(&self, top: u64) -> Vec<grid::Span> {
        self.spans.iter().filter(|span| span.to > top).cloned().collect()
    }

    /// The byte the cursor is on: the transcript, then what the prompt
    /// and the line hold before the cursor the line editor keeps,
    /// which it counts in chars.
    fn cursor(&self) -> usize {
        let (line, column) = self.line.render(&self.shell.prompt());
        let column = usize::from(column);
        let before: usize = line.chars().take(column).map(char::len_utf8).sum();
        self.transcript.len() + before
    }

    /// The first byte shown: the start of the line that leaves the
    /// prompt on the last row the region has, so a transcript longer
    /// than the region scrolls with it.
    fn top(&self) -> usize {
        let lines = self.text.matches('\n').count() + 1;
        let height = self.rows.max(1);
        let Some(over) = lines.checked_sub(height) else { return 0 };
        let mut at = 0;
        for _ in 0..over {
            at += self.text[at..].find('\n').map_or(0, |end| end + 1);
        }
        at
    }
}

/// What a Driver's SGR sequence begins with; `m` ends it.
const ESC: &str = "\x1b[";

/// The text `said` holds with the escapes taken out, and one span per
/// run of one, in the Buffer's bytes from `at`.
///
/// A Driver writes one shape and never nests it: `\x1b[<code>m`, the
/// run, `\x1b[0m`. The reset ends the run open, and so does a code
/// that stands for no style.
fn read(said: &str, at: u64) -> (String, Vec<grid::Span>) {
    let (mut text, mut spans) = (String::new(), Vec::new());
    let mut open: Option<grid::Span> = None;
    let mut rest = said;
    while let Some(esc) = rest.find(ESC) {
        let tail = &rest[esc + ESC.len()..];
        let Some(end) = tail.find('m') else { break };
        text.push_str(&rest[..esc]);
        let at = at + text.len() as u64;
        let ended = |run: grid::Span| grid::Span { to: at, ..run };
        match style(&tail[..end]) {
            Some(style) => open = Some(grid::Span { from: at, to: at, style }),
            None => spans.extend(open.take().map(ended)),
        }
        rest = &tail[end + 1..];
    }
    text.push_str(rest);
    (text, spans)
}

/// The style a Driver's SGR parameter stands for: the one place the
/// two spellings meet, the escape being the Driver's and the number
/// the Client's. Anything else is no style, the reset first.
fn style(code: &str) -> Option<u64> {
    Some(match code {
        "1;34" => BLUE,
        "32" => GREEN,
        "31" => RED,
        "1;31" => RED_BOLD,
        "1" => BOLD,
        "33" => YELLOW,
        "2" => DIM,
        _ => return None,
    })
}

/// What a run printed, its cell closed where the run came back open: a
/// Driver that follows is read this once and no more, a Plugin having
/// no tick to read it again.
fn printed(ctx: &dyn Calls, ran: Ran) -> String {
    let Ran { out, open } = ran;
    if let Some(mut open) = open {
        let _ = open.close(ctx);
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A Buffer of the buffer's own, over nothing: what the transcript is
/// written into and what its View names.
fn scratch(mut ctx: &dyn Calls) -> Result<Rd, Error> {
    let opener = cell::resolve(ctx, ROOT, "/module/text/open")?;
    let opening = ctx.open(opener, Rights::WRITE)?;
    let buffer = text_open::Call(&mut ctx, opening).new();
    ops::close(ctx, opening)?;
    buffer
}

/// A count the Client gave, as far as this machine counts.
fn count(n: u64) -> usize {
    usize::try_from(n).unwrap_or(usize::MAX)
}

#[cfg(test)]
mod tests {
    use alloc::format;
    use alloc::vec;

    use reovim_client::{BLUE, BOLD, DIM, GREEN, RED, RED_BOLD, YELLOW, grid};

    use super::read;

    /// The seven a Driver under `/bin` writes, and the style each
    /// stands for: coreutils' listing, and sysutils' through its `sgr`.
    const CODES: [(&str, u64); 7] = [
        ("1;34", BLUE),
        ("32", GREEN),
        ("31", RED),
        ("1;31", RED_BOLD),
        ("1", BOLD),
        ("33", YELLOW),
        ("2", DIM),
    ];

    /// One span of the Buffer's bytes.
    const fn span(from: u64, to: u64, style: u64) -> grid::Span {
        grid::Span { from, to, style }
    }

    #[test]
    fn every_code_a_driver_writes_round_trips_to_its_style() {
        for (code, style) in CODES {
            let said = format!("one \x1b[{code}mtwo\x1b[0m three\n");
            let (text, spans) = read(&said, 10);
            assert_eq!(text, "one two three\n", "no escape is left");
            assert_eq!(spans, [span(14, 17, style)], "where the run stood");
        }
    }

    #[test]
    fn each_name_of_a_listing_is_a_span_of_its_own() {
        let said = "\x1b[1;34mbin\x1b[0m  \x1b[32mls\x1b[0m  plain\n";
        let (text, spans) = read(said, 0);
        assert_eq!(text, "bin  ls  plain\n");
        let shown = vec![span(0, 3, BLUE), span(5, 7, GREEN)];
        assert_eq!(spans, shown, "what was written plain is no span");
    }

    #[test]
    fn a_run_a_driver_wrote_without_color_is_no_span() {
        let (text, spans) = read("a  b  sub\n", 0);
        assert_eq!(text, "a  b  sub\n");
        assert_eq!(spans, [], "nothing to take out and nothing to color");
    }

    #[test]
    fn a_code_that_stands_for_no_style_ends_the_run_and_is_dropped() {
        let (text, spans) = read("\x1b[7mone\x1b[0m two\n", 0);
        assert_eq!(text, "one two\n", "the escape goes either way");
        assert_eq!(spans, [], "a style the Client does not name is none");
    }
}
