#![no_std]
#![forbid(unsafe_code)]
//! The cli buffer: a shell in a region of its own; a Module a Client
//! loads.
//!
//! It knows no Module and no name: every line is run by clients/sh, the
//! same `run` the process cli runs and the same `complete`, over the
//! one door the Client hands it and from where a `Shell` of its own
//! stands.
//! Nothing here is a second line editor either; the keys go to the
//! shell's own `Line`, history and all.
//!
//! What it owns is a scratch draft of `/self/dev/buffers` holding the
//! transcript: every line run as `<path> > <line>` with what it
//! printed under it, and last the prompt with the line being typed.
//! One `text.replace` of the whole text per change, as the explorer
//! refills its listing; nothing ever flushes it, so the transcript is
//! this Client's own and the View is laid out over bytes it holds.
//!
//! One cell is served, `view`: a whole read of it is one frame. A run
//! that came back open is a Driver that follows, and each frame asks
//! it for what it has since, so a `cat -f` in the buffer shows new
//! bytes with no key pressed.
//!
//! Leaving is `back` on `/self/dev/open`: the region shows again what
//! it showed before this buffer, which is the Client's to do.

extern crate alloc;

use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;

use reovim_client_api::{BAR, Key, NONE, Surface, count, drafted, window};
use reovim_client_sh::{Act, Line, Open, Ran, Shell, complete, run};
use reovim_kernel_interface::{
    Booted, Calls, Driver, Error, Handle, Module, Rd, Reply, Rights,
};
use reovim_sdk::{encode, sgr};
use reovim_text_api::text;

/// The cli buffer, ready for a Client to load.
#[must_use]
pub fn boot() -> Booted {
    let cli = Rc::new(Loaded::default());
    Booted { module: cli.clone(), drivers: alloc::vec![cli] }
}

/// A shell in a region: the transcript, the line being typed at the
/// prompt under it, and where that line stands.
///
/// `start` takes the rd the region was opened on and stands the Shell
/// there; everything after it is what has been typed since.
#[derive(Default)]
struct Cli {
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
    spans: Vec<window::Span>,
    /// The escape the last batch of a following run was cut in the
    /// middle of, which the batch after it begins with.
    carry: String,
    /// What the Buffer holds, as the last fill wrote it: what the next
    /// one replaces, and what says a fill would write nothing.
    text: String,
    /// The last run, while it came back open: each frame asks it for
    /// what the Driver it follows has since.
    following: Option<Open>,
    /// The region's columns, as `size` last said: what a listing lays
    /// its names across.
    cols: u64,
    /// Its rows, which the transcript scrolls in.
    rows: usize,
    /// Ctrl-C or Ctrl-D asked for the region back: this Plugin is
    /// over, and no key after that one is its own.
    leaving: bool,
}

/// The buffer's state behind one `RefCell`: no borrow of it outlives an
/// op, so a Driver that re-enters finds it free.
#[derive(Default)]
struct Loaded {
    state: RefCell<Cli>,
    surface: Surface,
}

impl Module for Loaded {
    /// A scratch Buffer of its own, the Shell stood where the region
    /// was opened, the first prompt shown, and the one cell a Plugin
    /// serves, `view`.
    fn start(&self, ctx: &dyn Calls, home: Rd) -> Result<(), Error> {
        self.surface.start(ctx, home)?;
        let over = self.surface.region()?.ask(ctx)?.target;
        let buffer = drafted(ctx, None)?;
        let rights = Rights::READ.union(Rights::WRITE);
        let h = ctx.open(buffer, rights)?;
        {
            let mut cli = self.state.borrow_mut();
            cli.buffer = Some(buffer);
            cli.h = Some(h);
        }
        self.sized(ctx)?;
        self.stand(ctx, over);
        self.fill(ctx)
    }
}

impl Driver for Loaded {
    fn open(&self, _: &dyn Calls, rd: Rd, _: Rights) -> Reply<u64> {
        Reply::ok(rd.0)
    }

    fn read(
        &self,
        ctx: &dyn Calls,
        _: u64,
        at: u64,
        len: usize,
    ) -> Reply<Vec<u8>> {
        self.surface.read(at, len, || self.framed(ctx))
    }
}

impl Loaded {
    /// One frame: every key it has not taken, what the run it left
    /// open has printed since, the rows it has now, and the View.
    fn framed(&self, ctx: &dyn Calls) -> Result<Vec<u8>, Error> {
        let keys = self.surface.region()?.keys(ctx)?;
        let mut moved = false;
        for key in keys {
            moved = true;
            self.pressed(ctx, key)?;
        }
        // The region has been given away: there is no frame of this
        // Plugin's left to answer, and what it last showed stands.
        if self.state.borrow().leaving {
            return Ok(encode(&self.state.borrow().view()));
        }
        moved |= self.following(ctx);
        self.sized(ctx)?;
        if moved {
            self.fill(ctx)?;
        }
        Ok(encode(&self.state.borrow().view()))
    }

    /// The run that came back open, asked for what its Driver has
    /// printed since: true when there was anything, which is how a
    /// `cat -f` shows new bytes with no key pressed.
    fn following(&self, ctx: &dyn Calls) -> bool {
        let Some(mut open) = self.state.borrow_mut().following.take() else {
            return false;
        };
        let (bytes, ended) = match open.more(ctx) {
            Ok(more) => more,
            Err(miss) => {
                self.state.borrow_mut().said(&format!("{miss}\n"));
                return true;
            }
        };
        if !ended {
            self.state.borrow_mut().following = Some(open);
        }
        if bytes.is_empty() {
            return false;
        }
        let said = String::from_utf8_lossy(&bytes).into_owned();
        self.state.borrow_mut().said(&said);
        true
    }

    /// The columns and rows the region has now, and what a run writes
    /// for them: the columns a listing lays its names across, and
    /// color, which [`read`] takes back out of the text.
    fn sized(&self, ctx: &dyn Calls) -> Result<(), Error> {
        let held: window::Size = self.surface.region()?.size(ctx)?;
        let mut cli = self.state.borrow_mut();
        cli.cols = held.cols;
        cli.rows = count(held.rows);
        // What a run writes for: the columns a listing lays its names
        // across, and color, which `read` takes back out of the text.
        cli.shell.screen(held.cols.max(1), true);
        Ok(())
    }

    /// Where the line starts: the rd the region was opened on, named
    /// to the shell's own `cd` as `rd:<n>`, so the path the prompt
    /// shows is the one the shell itself finds and no path is spelled
    /// here. A target that is no Container is that `cd`'s own refusal,
    /// and the line stands at the root with nothing said.
    fn stand(&self, ctx: &dyn Calls, over: Rd) {
        if over == NONE {
            return;
        }
        // Nothing of this is typed, so nothing of it is transcript.
        let mut shell = core::mem::take(&mut self.state.borrow_mut().shell);
        let _ = run(ctx, &mut shell, &format!("cd rd:{}", over.0));
        self.state.borrow_mut().shell = shell;
    }

    /// One key, which is the shell's line editor's: its acts, and the
    /// two that leave the region are `back`. A key after the one that
    /// gave the region away is none of this Plugin's.
    fn pressed(&self, ctx: &dyn Calls, key: Key) -> Result<(), Error> {
        if self.state.borrow().leaving {
            return Ok(());
        }
        let act = self.state.borrow_mut().line.key(key);
        match act {
            Act::Nothing | Act::Redraw => {}
            Act::Submit(line) => self.ran(ctx, &line),
            Act::Complete => self.completing(ctx),
            // Ctrl-L: what has been run and answered goes, and the
            // line being typed stands where it stood.
            Act::Clear => {
                let mut cli = self.state.borrow_mut();
                cli.transcript.clear();
                cli.spans.clear();
            }
            // Ctrl-C, and Ctrl-D on an empty line.
            Act::Quit => {
                self.state.borrow_mut().leaving = true;
                return self.surface.region()?.backs(ctx);
            }
        }
        Ok(())
    }

    /// Tab: what may stand where the line does, found over the tree
    /// from where the Shell stands, which the line then takes.
    fn completing(&self, ctx: &dyn Calls) {
        let (shell, word) = {
            let cli = self.state.borrow();
            (cli.shell.clone(), String::from(cli.line.text()))
        };
        let found = complete(ctx, &shell, &word);
        self.state.borrow_mut().line.apply(&found);
    }

    /// One line submitted: the prompt and the line into the
    /// transcript, then what the line printed under it. An empty line
    /// is a new prompt and nothing else.
    ///
    /// A run that came back open is a Driver that follows: it is kept,
    /// and every frame from now on asks it for what it has since.
    fn ran(&self, ctx: &dyn Calls, line: &str) {
        // The prompt the line was typed at, which a `cd` moves.
        if !self.state.borrow_mut().prompted(line) {
            return;
        }
        // The borrow is gone before the line runs: a Driver of this
        // Client's own may come back in here.
        let mut shell = core::mem::take(&mut self.state.borrow_mut().shell);
        let outcome = run(ctx, &mut shell, line);
        self.state.borrow_mut().shell = shell;
        let said = match outcome {
            Ok(Ran { out, open }) => {
                self.state.borrow_mut().following = open;
                String::from_utf8_lossy(&out).into_owned()
            }
            Err(miss) => format!("{miss}\n"),
        };
        self.state.borrow_mut().said(&said);
    }

    /// The Buffer filled with what the region shows now, whole, so a
    /// change is one revision. A key that moved the cursor alone
    /// leaves the text as it was and writes nothing.
    fn fill(&self, mut ctx: &dyn Calls) -> Result<(), Error> {
        let Some(edit) = self.state.borrow_mut().refilled() else {
            return Ok(());
        };
        let h = self.state.borrow().h.ok_or(Error::Closed)?;
        text::Call(&mut ctx, h).replace(edit)
    }
}

impl Cli {
    /// What the region shows now in place of what it showed, or
    /// nothing where a key moved the cursor alone.
    fn refilled(&mut self) -> Option<text::Edit> {
        let text = format!(
            "{}{}{}",
            self.transcript,
            self.shell.prompt(),
            self.line.text()
        );
        if text == self.text {
            return None;
        }
        let was = core::mem::replace(&mut self.text, text);
        let text = self.text.clone();
        Some(text::Edit { at: 0, len: was.len() as u64, text })
    }

    /// The prompt and the line into the transcript; false for an empty
    /// line, which is a new prompt and nothing else.
    fn prompted(&mut self, line: &str) -> bool {
        let prompt = self.shell.prompt();
        self.transcript.push_str(&prompt);
        self.transcript.push_str(line);
        self.transcript.push('\n');
        !line.trim().is_empty()
    }

    /// What a line printed, under it: its own lines with the escapes
    /// taken out and a span where each run of one stood, and a last
    /// line that ends in no newline gets one, the prompt following it.
    fn said(&mut self, out: &str) {
        self.carry.push_str(out);
        let said = core::mem::take(&mut self.carry);
        let at = self.transcript.len() as u64;
        let (text, spans) = read(&said, at, &mut self.carry);
        if text.is_empty() {
            return;
        }
        self.transcript.push_str(&text);
        self.spans.extend(spans);
        if !text.ends_with('\n') {
            self.transcript.push('\n');
        }
    }

    /// What to show: the cursor where the line is being typed, the
    /// prompt line the last of the rows the region has, and nothing in
    /// the status row, which the frame already names.
    fn view(&self) -> window::View {
        let top = self.top() as u64;
        window::View {
            over: self.buffer.unwrap_or(NONE),
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
    fn shown(&self, top: u64) -> Vec<window::Span> {
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

/// What a control sequence begins with, and what a CSI's begins with.
const ESC: char = '\x1b';
const CSI: &str = "\x1b[";

/// The text `said` holds with the escapes taken out, and one span per
/// run of one, in the Buffer's bytes from `at`. A sequence this chunk
/// cut in two is left in `carry` for the chunk after it to begin with.
///
/// A Driver writes one shape and never nests it: `\x1b[<code>m`, the
/// run, `\x1b[0m`. The reset ends the run open, and so do a code that
/// stands for no style, a second colour, and the end of the output.
fn read(
    said: &str,
    at: u64,
    carry: &mut String,
) -> (String, Vec<window::Span>) {
    let (mut text, mut spans) = (String::new(), Vec::new());
    let mut open: Option<window::Span> = None;
    let mut rest = said;
    while let Some(esc) = rest.find(ESC) {
        text.push_str(&rest[..esc]);
        let to = at + text.len() as u64;
        let ended = |run: window::Span| window::Span { to, ..run };
        let Some((code, after)) = csi(&rest[esc..]) else {
            carry.push_str(&rest[esc..]);
            rest = "";
            break;
        };
        // A sequence that is no SGR says nothing of style and goes
        // alone, the run standing across it.
        if let Some(code) = code {
            spans.extend(open.take().map(ended));
            let run = |style| window::Span { from: to, to, style };
            open = sgr::style(code).map(run);
        }
        rest = after;
    }
    text.push_str(rest);
    let to = at + text.len() as u64;
    spans.extend(open.map(|run| window::Span { to, ..run }));
    (text, spans)
}

/// The control sequence `seq` begins with: the parameters of an SGR,
/// `None` for a sequence that is not one, and what follows it either
/// way. A CSI ends at its first byte of 0x40-0x7E, and an escape that
/// begins no CSI is one byte of nothing. `None` for a sequence that is
/// not whole yet, which is the chunk's carry.
fn csi(seq: &str) -> Option<(Option<&str>, &str)> {
    let Some(body) = seq.strip_prefix(CSI) else {
        return (seq.len() > 1).then(|| (None, &seq[1..]));
    };
    let end = body.bytes().position(|b| (0x40..=0x7e).contains(&b))?;
    let code = (body.as_bytes()[end] == b'm').then(|| &body[..end]);
    Some((code, &body[end + 1..]))
}

#[cfg(test)]
mod tests {
    use alloc::format;
    use alloc::string::String;
    use alloc::vec;
    use alloc::vec::Vec;

    use reovim_client_api::{
        BLUE, BOLD, DIM, GREEN, RED, RED_BOLD, YELLOW, window,
    };

    use super::read;

    /// One whole output through the reader, which carries nothing past
    /// its end.
    fn reading(said: &str, at: u64) -> (String, Vec<window::Span>) {
        let mut carry = String::new();
        let read = read(said, at, &mut carry);
        assert!(carry.is_empty(), "nothing of it was cut in two");
        read
    }

    /// One chunk of a following run, as [`super::Cli::said`] gives it:
    /// what the chunk before it left carried, then the chunk.
    fn chunk(
        carry: &mut String,
        said: &str,
        at: u64,
    ) -> (String, Vec<window::Span>) {
        carry.push_str(said);
        let whole = core::mem::take(carry);
        read(&whole, at, carry)
    }

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
    const fn span(from: u64, to: u64, style: u64) -> window::Span {
        window::Span { from, to, style }
    }

    #[test]
    fn every_code_a_driver_writes_round_trips_to_its_style() {
        for (code, style) in CODES {
            let said = format!("one \x1b[{code}mtwo\x1b[0m three\n");
            let (text, spans) = reading(&said, 10);
            assert_eq!(text, "one two three\n", "no escape is left");
            assert_eq!(spans, [span(14, 17, style)], "where the run stood");
        }
    }

    #[test]
    fn each_name_of_a_listing_is_a_span_of_its_own() {
        let said = "\x1b[1;34mbin\x1b[0m  \x1b[32mls\x1b[0m  plain\n";
        let (text, spans) = reading(said, 0);
        assert_eq!(text, "bin  ls  plain\n");
        let shown = vec![span(0, 3, BLUE), span(5, 7, GREEN)];
        assert_eq!(spans, shown, "what was written plain is no span");
    }

    #[test]
    fn a_run_a_driver_wrote_without_color_is_no_span() {
        let (text, spans) = reading("a  b  sub\n", 0);
        assert_eq!(text, "a  b  sub\n");
        assert_eq!(spans, [], "nothing to take out and nothing to color");
    }

    #[test]
    fn a_code_that_stands_for_no_style_ends_the_run_and_is_dropped() {
        let (text, spans) = reading("\x1b[7mone\x1b[0m two\n", 0);
        assert_eq!(text, "one two\n", "the escape goes either way");
        assert_eq!(spans, [], "a style the Client does not name is none");
    }

    #[test]
    fn a_sequence_that_is_no_style_goes_alone_and_takes_no_text_with_it() {
        // `\x1b[2K` clears a row and says nothing of style: it ends at
        // its own final byte and the text on either side of it stands.
        let (text, spans) = reading("one\x1b[2Ktwo\x1b[32mthree\x1b[0m\n", 0);
        assert_eq!(text, "onetwothree\n", "nothing between was eaten");
        assert_eq!(spans, [span(6, 11, GREEN)], "the run after it");
    }

    #[test]
    fn an_escape_two_chunks_were_cut_between_is_read_with_the_second() {
        let mut carry = String::new();
        let (text, spans) = chunk(&mut carry, "one\x1b[3", 0);
        assert_eq!(text, "one", "no half of an escape is text");
        assert_eq!(spans, []);
        let (text, spans) = chunk(&mut carry, "2mtwo\x1b[0m\n", 3);
        assert_eq!(text, "two\n", "the escape was whole with this chunk");
        assert_eq!(spans, [span(3, 6, GREEN)]);
        assert!(carry.is_empty(), "and nothing is held for the next");
    }

    #[test]
    fn a_second_colour_ends_the_run_the_first_opened() {
        // A Driver never nests a run, so a colour before the reset is
        // where the run before it ended.
        let (text, spans) = reading("\x1b[32ma\x1b[31mb\x1b[0m\n", 0);
        assert_eq!(text, "ab\n");
        let shown = vec![span(0, 1, GREEN), span(1, 2, RED)];
        assert_eq!(spans, shown, "neither run is lost");
    }

    #[test]
    fn a_run_the_output_ends_inside_is_a_span_to_the_end() {
        let (text, spans) = reading("\x1b[32mgreen\n", 0);
        assert_eq!(text, "green\n", "a Driver that wrote no reset");
        assert_eq!(spans, [span(0, 6, GREEN)]);
    }
}
