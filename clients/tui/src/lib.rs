#![no_std]
#![forbid(unsafe_code)]
//! The tui: one region of a terminal, laid out from `/self/dev`.
//!
//! It knows no Plugin, no table and no name. A key is one write of
//! `/self/dev/input/keyboard`; a frame is one read of
//! `/self/dev/windows/0/view`, which the Client's own Driver routes to
//! whichever Plugin holds the region; a size it has not written goes to
//! `/self/dev/windows/0/size` before that read; and the code it leaves
//! with is what `/self/dev/exit` holds.
//!
//! What it does with the View is the whole of its work: the View names
//! the rd it is in the bytes of, and the tui keeps its own copy of
//! those bytes, watches them, and lays the View's cursor and spans out
//! over them in screen cells. It sees neither a link nor a terminal.

extern crate alloc;

use alloc::format;
use alloc::rc::Rc;
use alloc::string::String;
use alloc::vec::Vec;

use reovim_client::{Host, HostCalls};
use reovim_client_api::{BAR, Key, MATCH, NONE, SELECTION, open, window};
use reovim_kernel_interface::{Calls, Error, Event, Handle, Kind, Rd, Rights};
use reovim_sdk::{cell, decode, encode, ops};
use reovim_text_api::history;

/// The cells a char takes: the declarations', drawn on here and by the
/// app above.
pub use reovim_client_api::width;

/// Bytes asked for per read of a cell of `dev`, which holds a record or
/// a short line of text.
const CHUNK: u32 = 64 * 1024;

/// What a key or a tick did.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Step {
    Same,
    /// Draw again.
    Changed,
    /// Leave with this exit code: 0 says use the file.
    Quit(u8),
}

/// What to draw, in screen cells.
///
/// `rows - 1` text rows as the terminal shows them (a tab already the
/// spaces to its stop, a wide char two cells), the cursor as (row,
/// cell) on them, the selected cells as `(row, from, to)` with `to`
/// exclusive, the spans of every other style in that shape with the
/// style after them, and the status row.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Frame {
    pub lines: Vec<String>,
    pub cursor: (u16, u16),
    /// The cursor's shape, `BLOCK` or `BAR`, for the app to set on the
    /// terminal: the cell it names is the one it is drawn over, or the
    /// one it stands before.
    pub shape: u64,
    pub selection: Vec<(u16, u16, u16)>,
    /// Search matches, in the selection's shape.
    pub marks: Vec<(u16, u16, u16)>,
    /// Every span of a style the tui does not draw itself, in that
    /// shape with the style the View gave it: what a style means is
    /// the app's, which has the terminal.
    pub styled: Vec<(u16, u16, u16, u64)>,
    pub status: String,
}

/// The name of a Buffer over nothing, as nvim shows it.
const UNNAMED: &str = "[No Name]";
/// The size it takes the terminal for until the app has drawn once.
const FIRST: (u16, u16) = (80, 24);

/// The cells of `/self/dev` the tui holds open for as long as it runs.
struct Cells {
    keyboard: Handle,
    size: Handle,
    exit: Handle,
    ask: Handle,
    view: Handle,
}

impl Cells {
    /// Each of them opened once, with the right the tui needs of it.
    fn of(ctx: &dyn Calls, dev: Rd) -> Result<Self, Error> {
        let at = |path: &str| cell::resolve(ctx, dev, path);
        Ok(Self {
            keyboard: ctx.open(at("input/keyboard")?, Rights::WRITE)?,
            size: ctx.open(at("windows/0/size")?, Rights::WRITE)?,
            exit: ctx.open(at("exit")?, Rights::READ)?,
            ask: ctx.open(at("windows/0/ask")?, Rights::READ)?,
            view: ctx.open(at("windows/0/view")?, Rights::READ)?,
        })
    }
}

/// The rd a View named, laid out here: the handle the tui reads and
/// watches it through, what that rd is over once it is over anything,
/// its bytes as the tui last read them, and where its history stands.
struct Laid {
    rd: Rd,
    h: Handle,
    source: Option<(Rd, Handle)>,
    text: String,
    head: u64,
    published: u64,
}

/// One region of a terminal over `/self/dev`.
pub struct Tui {
    host: Rc<Host>,
    cells: Cells,
    /// The last View, and the bytes it came in: a read that gives the
    /// same bytes is nothing to draw.
    shown: Option<window::View>,
    bytes: Vec<u8>,
    /// What the region was opened with, as `windows/0/ask` says now.
    ask: open::Ask,
    /// What the View last named, opened and watched here.
    laid: Option<Laid>,
    /// What the status row calls the region.
    name: String,
    /// The size `frame` was last asked for.
    size: (u16, u16),
    /// The View, the text or the revisions moved since the last step.
    moved: bool,
    /// Where this Client stands in the one stream of events, which the
    /// Host numbers for both sides.
    cursor: u64,
}

impl Tui {
    /// The region of the `/self/dev` at `dev`, ready to draw.
    ///
    /// The cells it writes and reads are opened once and held: the
    /// keyboard, the region's size, its ask and its view, and the exit
    /// code. The first Plugin comes of the first read of the view,
    /// which is this one.
    ///
    /// # Errors
    /// What the Client says.
    pub fn open(host: Rc<Host>, dev: Rd) -> Result<Self, Error> {
        let cells = Cells::of(&host.own(), dev)?;
        let mut tui = Self {
            host,
            cells,
            shown: None,
            bytes: Vec::new(),
            ask: open::Ask {
                target: NONE,
                plugin: String::new(),
                name: String::new(),
                line: 0,
            },
            laid: None,
            name: String::from(UNNAMED),
            size: FIRST,
            moved: false,
            cursor: 0,
        };
        tui.stepped()?;
        // What the Plugin's own opening left on the watched rds is
        // already in the text read with them.
        tui.since()?;
        tui.moved = false;
        Ok(tui)
    }

    /// The one door its ops go through, taken for the statement that
    /// makes one, as the Host's own borrow rule asks.
    fn ctx(&self) -> HostCalls<'_> {
        self.host.own()
    }

    /// The rd laid out now: what the last View named.
    #[must_use]
    pub fn over(&self) -> Option<Rd> {
        self.laid.as_ref().map(|laid| laid.rd)
    }

    /// One key appended to the keyboard, and the frame after it, so a
    /// key costs no frame of latency.
    ///
    /// # Errors
    /// What the Client says.
    pub fn key(&mut self, key: Key) -> Result<Step, Error> {
        let code = key.code().to_le_bytes().to_vec();
        ops::write(&self.ctx(), self.cells.keyboard, 0, code)?;
        self.stepped()
    }

    /// One step with nothing typed: what happened, and the frame.
    ///
    /// # Errors
    /// `Gone` once what is laid out is gone; otherwise what the Client
    /// says.
    pub fn tick(&mut self) -> Result<Step, Error> {
        self.stepped()
    }

    /// What to draw for `cols` by `rows`: the View laid out over the
    /// tui's own copy of the text it names. A size the region has not
    /// been written reaches it first, so the Plugin can move the View.
    ///
    /// # Errors
    /// What the Client says.
    pub fn frame(&mut self, cols: u16, rows: u16) -> Result<Frame, Error> {
        if (cols, rows) != self.size {
            self.size = (cols, rows);
            let size = window::Size {
                cols: u64::from(cols),
                rows: u64::from(rows.saturating_sub(1)),
            };
            ops::write(&self.ctx(), self.cells.size, 0, encode(&size))?;
            self.stepped()?;
        }
        Ok(self.laid_out(cols, rows))
    }

    /// One step: the region's view read, which is the frame of the
    /// Plugin holding it, then what that left on the watched rds, then
    /// the rd the View names laid out.
    fn stepped(&mut self) -> Result<Step, Error> {
        self.viewed()?;
        self.after()?;
        self.settled()?;
        self.step()
    }

    /// The view read whole, which is one frame of the Plugin holding
    /// the region. A region no Plugin holds reads empty and what was
    /// drawn stands.
    fn viewed(&mut self) -> Result<(), Error> {
        let bytes = ops::whole(&self.ctx(), self.cells.view)?;
        if bytes.is_empty() || bytes == self.bytes {
            return Ok(());
        }
        self.shown = Some(decode(&bytes)?);
        self.bytes = bytes;
        self.moved = true;
        Ok(())
    }

    /// The events drained: the laid text read again on a Modify or an
    /// Overflow, and the revisions and the name after it.
    fn after(&mut self) -> Result<(), Error> {
        let events = self.since()?;
        if events.is_empty() {
            return Ok(());
        }
        self.reread(&events)?;
        self.refresh()
    }

    /// What happened on the watched rds since the last ask, the cursor
    /// moved on: one stream over both sides, the Host numbering it.
    fn since(&mut self) -> Result<Vec<Event>, Error> {
        let (events, cursor) = ops::since(&self.ctx(), self.cursor)?;
        self.cursor = cursor;
        Ok(events)
    }

    /// What the region says of itself now: the ask it was opened with,
    /// which names it, and the rd the View names laid out.
    fn settled(&mut self) -> Result<(), Error> {
        let bytes = ops::read(&self.ctx(), self.cells.ask, 0, CHUNK)?;
        let ask: open::Ask = decode(&bytes)?;
        if ask != self.ask {
            self.ask = ask;
            self.name = self.named()?;
        }
        self.lay()
    }

    /// Quit once the exit cell holds a code; Changed once the View or
    /// what it is laid out over has moved.
    fn step(&mut self) -> Result<Step, Error> {
        let held = ops::read(&self.ctx(), self.cells.exit, 0, CHUNK)?;
        if !held.is_empty() {
            let text = core::str::from_utf8(&held).unwrap_or("0");
            return Ok(Step::Quit(text.trim().parse().unwrap_or(0)));
        }
        let moved = core::mem::take(&mut self.moved);
        Ok(if moved { Step::Changed } else { Step::Same })
    }

    /// The tui's own copy of the laid text, read again when the events
    /// say it moved: a Modify on it or on what it is over, or a ring
    /// that overflowed past the tui. Its going is the end of the
    /// editor.
    fn reread(&mut self, events: &[Event]) -> Result<(), Error> {
        let Some(laid) = &self.laid else { return Ok(()) };
        let (rd, h) = (laid.rd, laid.h);
        let source = laid.source.map(|(source, _)| source);
        let on = |rd: Rd, kind: Kind| {
            events.iter().any(|e| e.rd == rd && e.kind == kind)
        };
        if on(rd, Kind::Gone) {
            return Err(Error::Gone);
        }
        // Overflow: the ring moved past us, so everything is read again.
        let overflow = events.iter().any(|e| e.kind == Kind::Overflow);
        let over = source.is_some_and(|source| on(source, Kind::Modify));
        if !overflow && !on(rd, Kind::Modify) && !over {
            return Ok(());
        }
        let bytes = ops::whole(&self.ctx(), h)?;
        let text = String::from_utf8_lossy(&bytes).into_owned();
        if let Some(laid) = &mut self.laid {
            self.moved |= text != laid.text;
            laid.text = text;
        }
        Ok(())
    }

    /// The rd the View names, laid out: opened READ, watched, read,
    /// and what was laid before closed. A View over what is laid
    /// already leaves it where it is.
    fn lay(&mut self) -> Result<(), Error> {
        let named = self.shown.as_ref().map(|view| view.over);
        let Some(rd) = named else { return Ok(()) };
        if rd == NONE || self.over() == Some(rd) {
            return Ok(());
        }
        let laid = lay_on(&self.ctx(), rd)?;
        if let Some(before) = self.laid.replace(laid) {
            unlay(&self.ctx(), &before)?;
        }
        self.moved = true;
        // The name first: it is what takes the source, which the
        // revisions after it then see.
        self.name = self.named()?;
        self.refresh()
    }

    /// Head and published from `tree`, and the name the region goes by
    /// now: a `:w NAME` on a Buffer over nothing gives it one.
    fn refresh(&mut self) -> Result<(), Error> {
        let Some(laid) = &self.laid else { return Ok(()) };
        let (h, over) = (laid.h, laid.source.is_some());
        let own = self.host.own();
        let mut ctx: &dyn Calls = &own;
        let tree = history::Call(&mut ctx, h).tree()?;
        if let Some(laid) = &mut self.laid {
            let moved =
                (tree.head, tree.published) != (laid.head, laid.published);
            self.moved |= moved;
            (laid.head, laid.published) = (tree.head, tree.published);
        }
        // What is laid out may have taken a source since.
        if !over {
            self.name = self.named()?;
        }
        Ok(())
    }

    /// What the region goes by: the name of what the laid rd is over,
    /// else the name of the rd the region was opened on, else the name
    /// it was opened with, else `[No Name]`.
    fn named(&mut self) -> Result<String, Error> {
        if let Some(source) = self.source()? {
            return name_of(&self.ctx(), source);
        }
        if self.ask.target != NONE {
            return name_of(&self.ctx(), self.ask.target);
        }
        let opened =
            (!self.ask.name.is_empty()).then(|| own_name(&self.ask.name));
        Ok(String::from(opened.unwrap_or(UNNAMED)))
    }

    /// What the laid rd is over, once it is over anything: its
    /// `source` child, read once and watched from then on, as a
    /// Buffer opened over a file is.
    fn source(&mut self) -> Result<Option<Rd>, Error> {
        let Some(laid) = &self.laid else { return Ok(None) };
        if let Some((source, _)) = laid.source {
            return Ok(Some(source));
        }
        let Ok(child) = ops::lookup(&self.ctx(), laid.rd, "source") else {
            return Ok(None);
        };
        let held = ops::bytes(&self.ctx(), child)?;
        let source: Rd = decode(&held)?;
        let watched = ops::watch(&self.ctx(), source)?;
        if let Some(laid) = &mut self.laid {
            laid.source = Some((source, watched));
        }
        self.moved = true;
        Ok(Some(source))
    }

    /// The bytes the View is in, as the tui last read them; nothing
    /// before the first View.
    fn text(&self) -> &str {
        self.laid.as_ref().map_or("", |laid| laid.text.as_str())
    }

    /// The View in cells: the lines from its top, the spans and the
    /// cursor on them, and the status row.
    fn laid_out(&self, cols: u16, rows: u16) -> Frame {
        let width = usize::from(cols.max(1));
        let height = usize::from(rows.saturating_sub(1).max(1));
        let text = self.text();
        let Some(view) = self.shown.as_ref() else {
            return Frame { status: self.status(0, ""), ..Frame::default() };
        };
        let cursor = boundary(text, at_of(view.cursor));
        let mut frame = Frame {
            status: self.status(cursor, &view.status),
            shape: view.shape,
            ..Frame::default()
        };
        let mut at = start_of(text, boundary(text, at_of(view.top)));
        let last = text.len() - usize::from(text.ends_with('\n'));
        while frame.lines.len() < height && at <= last {
            let end = end_of(text, at);
            let row = cell(frame.lines.len());
            frame.lines.push(shown(&text[at..end], width));
            self.on_row(&mut frame, view, row, (at, end), (cursor, width));
            at = end + 1;
        }
        frame
    }

    /// What of the View falls on the row `row`, the line of bytes
    /// `at..end`: its spans in cells, and the cursor when it is there.
    fn on_row(
        &self,
        frame: &mut Frame,
        view: &window::View,
        row: u16,
        (at, end): (usize, usize),
        (cursor, width): (usize, usize),
    ) {
        let line = &self.text()[at..end];
        let cells = |span: &window::Span| {
            span_on(line, at, end, span).map(|at| (span.style, at))
        };
        for (style, (from, to)) in view.spans.iter().filter_map(cells) {
            let (from, to) = (cell(from), cell(to));
            match style {
                SELECTION => frame.selection.push((row, from, to)),
                MATCH => frame.marks.push((row, from, to)),
                style => frame.styled.push((row, from, to, style)),
            }
        }
        if (at..=end).contains(&cursor) {
            let at = cell_at(line, cursor - at, view.shape);
            frame.cursor = (row, cell(at.min(width - 1)));
        }
    }

    /// The status row: the name, where the cursor is, the revision and
    /// whether it is written, then the Plugin's own word.
    fn status(&self, cursor: usize, word: &str) -> String {
        let text = self.text();
        let before = &text[..boundary(text, cursor)];
        let line = before.matches('\n').count() + 1;
        let column = before.rsplit('\n').next().unwrap_or("").chars().count();
        let (head, published) = self
            .laid
            .as_ref()
            .map_or((0, 0), |laid| (laid.head, laid.published));
        let mark = if head == published { "" } else { "[+]" };
        format!("{}  {line}:{}  r{head}{mark}   {word}", self.name, column + 1)
    }
}

/// `rd` as the tui lays a View out over it: opened READ, watched, and
/// read whole.
fn lay_on(ctx: &dyn Calls, rd: Rd) -> Result<Laid, Error> {
    let h = ops::watch(ctx, rd)?;
    let text = String::from_utf8_lossy(&ops::whole(ctx, h)?).into_owned();
    Ok(Laid { rd, h, source: None, text, head: 0, published: 0 })
}

/// What was laid out before, let go: its handle and the one on what it
/// was over.
fn unlay(ctx: &dyn Calls, laid: &Laid) -> Result<(), Error> {
    if let Some((_, watched)) = laid.source {
        ops::close(ctx, watched)?;
    }
    ops::close(ctx, laid.h)
}

/// The name of `rd` under its parent, empty for the root.
fn name_of(ctx: &dyn Calls, rd: Rd) -> Result<String, Error> {
    Ok(ops::info(ctx, rd)?.name.unwrap_or_default())
}

/// What a file under the root goes by on its own: the last segment of
/// `path`, which is the name its rd has under its directory.
fn own_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// A byte of the laid rd as an index, however far the Plugin put it.
fn at_of(at: u64) -> usize {
    usize::try_from(at).unwrap_or(usize::MAX)
}

/// A count as a cell, however far it goes.
fn cell(n: usize) -> u16 {
    u16::try_from(n).unwrap_or(u16::MAX)
}

/// `at` back to a char boundary of `text`, and not past its end: what
/// the Plugin points at, whatever byte it named.
fn boundary(text: &str, at: usize) -> usize {
    let mut at = at.min(text.len());
    while !text.is_char_boundary(at) {
        at -= 1;
    }
    at
}

/// Where the line holding byte `at` starts.
fn start_of(text: &str, at: usize) -> usize {
    text[..at].rfind('\n').map_or(0, |i| i + 1)
}

/// The newline ending the line that starts at `at`, or the text's end.
fn end_of(text: &str, at: usize) -> usize {
    text[at..].find('\n').map_or(text.len(), |i| at + i)
}

/// The cells `line` takes up to byte `n`, a tab reaching its stop and
/// a wide char two cells.
fn cells_upto(line: &str, n: usize) -> usize {
    let mut cells = 0;
    for (at, c) in line.char_indices() {
        if at >= n {
            break;
        }
        cells = next_cell(cells, c);
    }
    cells
}

/// The cell the cursor is drawn at when it is on byte `at`: where that
/// char starts, but a block on a tab takes the tab's last cell, as
/// nvim draws it. A bar stands before the char, so it keeps the first.
fn cell_at(line: &str, at: usize, shape: u64) -> usize {
    let cells = cells_upto(line, at);
    match line[at.min(line.len())..].chars().next() {
        Some('\t') if shape != BAR => next_cell(cells, '\t') - 1,
        _ => cells,
    }
}

/// `span` on the line of bytes `at..end`, in cells, `to` exclusive;
/// `None` when it does not reach the line or holds no cell of it. A
/// span that takes the line's newline takes the cell past its end,
/// which is how a whole selected line shows.
fn span_on(
    line: &str,
    at: usize,
    end: usize,
    span: &window::Span,
) -> Option<(usize, usize)> {
    let (from, to) = (at_of(span.from), at_of(span.to));
    if from > end || to <= at {
        return None;
    }
    let cells = |byte: usize| cells_upto(line, byte.saturating_sub(at));
    let past = usize::from(to > end);
    let (from, to) = (cells(from), cells(to) + past);
    (from < to).then_some((from, to))
}

/// `line` as the terminal shows it in `width` cells: a tab as the
/// spaces to its stop, a wide char kept whole or left out at the edge.
fn shown(line: &str, width: usize) -> String {
    let mut out = String::new();
    let mut cell = 0;
    for c in line.chars() {
        let next = next_cell(cell, c);
        if next > width {
            break;
        }
        match c {
            '\t' => out.extend(core::iter::repeat_n(' ', next - cell)),
            c => out.push(c),
        }
        cell = next;
    }
    out
}

/// The screen cell after `c` drawn at `cell`: a tab reaches the next
/// stop of eight, a wide char two cells on.
const fn next_cell(cell: usize, c: char) -> usize {
    if c == '\t' { cell / 8 * 8 + 8 } else { cell + width(c) }
}
