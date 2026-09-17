#![no_std]
#![forbid(unsafe_code)]
//! The cli buffer: the cli's words in a region of their own; the
//! `Plugin` of clients/core.
//!
//! It knows no Module and no word: every line is run by clients/cli,
//! the same `input` the process cli runs and the same `complete`, over
//! the tree the Client has and from where a `Shell` of its own stands.
//! Nothing here is a second line editor either; the keys go to the
//! cli's own `Line`, history and all.
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
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use reovim_client::{
    BAR, Caps, Ctx, Error, Event, Fault, Handle, Hash, Key, Plugin, Rd, Rights,
    Start, Tree, grid, keys, open,
};
use reovim_client_cli::{Act, Line, Out, Shell, complete, input};
use reovim_text_api::{open as text_open, text};

/// The cli's words in a region: the transcript, the line being typed
/// at the prompt under it, and where that line stands.
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
    /// What has been run and answered, above the prompt.
    transcript: String,
    /// What the Buffer holds, as the last fill wrote it: what the next
    /// one replaces, and what says a fill would write nothing.
    text: String,
    /// The region's columns, as `grid` last said: what `ls` lays its
    /// names across.
    cols: u16,
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
    fn start(&mut self, ctx: Ctx<'_>, at: Start) -> Result<(), Fault> {
        let Ctx { tree, caps } = ctx;
        let buffer = scratch(tree)?;
        let rights = Rights::READ.union(Rights::WRITE);
        self.buffer = Some(buffer);
        self.h = Some(tree.open(buffer, rights)?);
        self.region = caps.open(grid::hash());
        self.opener = caps.open(open::hash());
        if let Some(region) = self.region {
            let size = grid::Call(caps, region).size()?;
            self.cols = cells(size.cols);
            self.rows = count(size.rows);
        }
        self.stand(tree, at.over);
        self.fill(tree)?;
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
    ) -> Result<(), Fault> {
        let Ctx { tree, caps } = ctx;
        if from == grid::hash() {
            let resized: grid::Resized = reovim_sdk::decode(bytes)?;
            self.cols = cells(resized.cols);
            self.rows = count(resized.rows);
            return self.show(caps);
        }
        if from != keys::hash() {
            return Ok(());
        }
        let press: keys::Press = reovim_sdk::decode(bytes)?;
        let Some(key) = Key::of(press.key) else { return Ok(()) };
        self.pressed(tree, caps, key)
    }

    /// Nothing: the cli buffer watches no rd of its own, and its own
    /// Buffer moves only where it wrote it. The words that follow the
    /// tree print what they have now and land here at 13.1.
    ///
    /// # Errors
    /// Never.
    fn changed(&mut self, _: Ctx<'_>, _: &[Event]) -> Result<(), Fault> {
        Ok(())
    }
}

impl Cli {
    /// The Buffer `start` opened; `Closed` before it has.
    fn handle(&self) -> Result<Handle, Fault> {
        self.h.ok_or(Fault::Kernel(Error::Closed))
    }

    /// Where the line starts: the rd the region was opened on, named
    /// to the cli's own `cd` as `rd:<n>`, so the path the prompt shows
    /// is the one the cli itself finds and no path is spelled here. A
    /// target that is no Container is that `cd`'s own refusal, and the
    /// line stands at the root with nothing said.
    fn stand(&mut self, tree: &mut Tree, over: Option<Rd>) {
        let Some(rd) = over else { return };
        let words = ["cd".to_string(), format!("rd:{}", rd.0)];
        let out = self.out();
        let _ = input(tree, &mut self.shell, &words, out);
    }

    /// Where the output goes: the region's columns, which `ls` lays
    /// its names across, and no color until 13.1.
    fn out(&self) -> Out {
        Out::screen(self.cols.max(1), false)
    }

    /// One key, which is the cli's line editor's: its acts, and the
    /// two that leave the region are `back`.
    fn pressed(
        &mut self,
        tree: &mut Tree,
        caps: &mut Caps,
        key: Key,
    ) -> Result<(), Fault> {
        match self.line.key(key) {
            Act::Nothing => return Ok(()),
            // The text or the cursor moved; the prompt line holds it.
            Act::Redraw => {}
            Act::Submit(line) => return self.submitted(tree, caps, &line),
            Act::Complete => {
                let found = complete(tree, &self.shell, self.line.text());
                self.line.apply(&found);
            }
            // Ctrl-L: what has been run and answered goes, and the
            // line being typed stands where it stood.
            Act::Clear => self.transcript.clear(),
            // Ctrl-C, and Ctrl-D on an empty line.
            Act::Quit => return self.leave(caps),
        }
        self.fill(tree)?;
        self.show(caps)
    }

    /// Enter: the line run and answered above the next prompt, or the
    /// region left where the line was `quit`.
    fn submitted(
        &mut self,
        tree: &mut Tree,
        caps: &mut Caps,
        line: &str,
    ) -> Result<(), Fault> {
        if self.ran(tree, line) {
            return self.leave(caps);
        }
        self.fill(tree)?;
        self.show(caps)
    }

    /// One line submitted: the prompt and the line into the
    /// transcript, then what the line printed under it.
    ///
    /// `quit` is read here and not by the cli, which refuses it: it is
    /// the word that ends an interactive session, and this is one, so
    /// true says the region is to go back. An empty line is a new
    /// prompt and nothing else.
    fn ran(&mut self, tree: &mut Tree, line: &str) -> bool {
        let words: Vec<String> =
            line.split_whitespace().map(ToString::to_string).collect();
        // The prompt the line was typed at, which a `cd` in it moves.
        self.transcript.push_str(&self.shell.prompt());
        self.transcript.push_str(line);
        self.transcript.push('\n');
        match words.first().map(String::as_str) {
            None => return false,
            Some("quit") => return true,
            Some(_) => {}
        }
        let out = self.out();
        let said = match input(tree, &mut self.shell, &words, out) {
            Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
            Err(miss) => format!("{miss}\n"),
        };
        self.said(&said);
        false
    }

    /// What a line printed, under it: its own lines, and a last one
    /// that ends in no newline gets one, the prompt following it.
    fn said(&mut self, out: &str) {
        if out.is_empty() {
            return;
        }
        self.transcript.push_str(out);
        if !out.ends_with('\n') {
            self.transcript.push('\n');
        }
    }

    /// The `quit` word, Ctrl-C, or Ctrl-D on an empty line: the region
    /// goes back to what it showed before this buffer, which is the
    /// Client's to do and the end of this Plugin.
    fn leave(&self, caps: &mut Caps) -> Result<(), Fault> {
        let Some(opener) = self.opener else { return Ok(()) };
        open::Call(caps, opener).back()
    }

    /// The Buffer filled with what the region shows now, whole, so a
    /// change is one revision. A key that moved the cursor alone
    /// leaves the text as it was and writes nothing.
    fn fill(&mut self, tree: &mut Tree) -> Result<(), Fault> {
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
        text::Call(tree, self.handle()?).replace(edit)
    }

    /// The View to the region, if the Client has one.
    fn show(&self, caps: &mut Caps) -> Result<(), Fault> {
        let Some(region) = self.region else { return Ok(()) };
        let view = self.view();
        grid::Call(caps, region).show(view)
    }

    /// What to show: the cursor where the line is being typed, the
    /// prompt line the last of the rows the region has, and no word in
    /// the status row, which the frame already names.
    fn view(&self) -> grid::View {
        grid::View {
            over: self.buffer.map_or(0, |buffer| buffer.0),
            top: self.top() as u64,
            cursor: self.cursor() as u64,
            // The line is always being typed into, so the cursor
            // stands before the char it is on, as a prompt's does.
            shape: BAR,
            spans: Vec::new(),
            status: String::new(),
        }
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

/// A Buffer of the cli's own, over nothing: what the transcript is
/// written into and what its View names.
fn scratch(tree: &mut Tree) -> Result<Rd, Fault> {
    let opener = tree.resolve("/module/text/open")?;
    let opening = tree.open(opener, Rights::WRITE)?;
    let buffer = text_open::Call(tree, opening).new();
    tree.close(opening)?;
    buffer
}

/// A count of cells the Client gave, as far as a row goes.
fn cells(n: u64) -> u16 {
    u16::try_from(n).unwrap_or(u16::MAX)
}

/// A count the Client gave, as far as this machine counts.
fn count(n: u64) -> usize {
    usize::try_from(n).unwrap_or(usize::MAX)
}
