#![no_std]
#![forbid(unsafe_code)]
//! The tui: one file in one Buffer, watched, with a `Plugin` on it.
//!
//! It serves three capabilities and nothing else. `keys` carries every
//! key to the Plugin; `grid` is the one region it draws, whose size it
//! answers and whose View it keeps; `process` is the code it leaves
//! with. The Plugin makes its own calls on the tree, so what a key
//! means, what it edits and what it says are none of the tui's
//! business: it lays the View out over its own copy of the text.
//!
//! It sees neither a link nor a terminal.

extern crate alloc;

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use reovim_client::{
    Caps, Client, Ctx, Error, Fault, Handle, Key, Kind, MATCH, Plugin, Rd,
    Rights, SELECTION, Start, grid, keys, process,
};
use reovim_text_api::{history, open};

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
/// exclusive, and the status row.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Frame {
    pub lines: Vec<String>,
    pub cursor: (u16, u16),
    pub selection: Vec<(u16, u16, u16)>,
    /// Search matches, in the selection's shape.
    pub marks: Vec<(u16, u16, u16)>,
    pub status: String,
}

/// The name of a Buffer over nothing, as nvim shows it.
const UNNAMED: &str = "[No Name]";
/// What it serves, one name per line: its capability cell.
const SERVES: &[u8] = b"keys\ngrid\nprocess\n";
/// The size it takes the terminal for until the app has drawn once.
const FIRST: (u16, u16) = (80, 24);

/// One Buffer over `/module/fs/root/FILE`, or over nothing, watched,
/// with the Plugin that reads it.
pub struct Tui<P: Plugin> {
    client: Client,
    plugin: P,
    caps: Caps,
    buffer: Rd,
    source: Option<Rd>,
    h: Handle,
    name: String,
    /// The Buffer's bytes, as the tui last read them.
    text: String,
    head: u64,
    published: u64,
    /// The size `frame` was last asked for.
    size: (u16, u16),
    /// The text or the revisions moved since the last step.
    moved: bool,
}

impl<P: Plugin> Tui<P> {
    /// Open `file`, a name under the root, as a Buffer, or one over
    /// nothing when there is no file; watch it and start `plugin` on
    /// it at `line`, counted from 1. `dir`, under the root too, is
    /// where a relative name the Plugin resolves starts.
    ///
    /// # Errors
    /// What the Client says; `Kernel(Invalid)` for a file that is not
    /// under the root.
    pub fn open(
        mut client: Client,
        dir: &str,
        file: Option<&str>,
        line: Option<usize>,
        plugin: P,
    ) -> Result<Self, Fault> {
        client.capability(SERVES.to_vec())?;
        let source = match file {
            Some(file) => Some(file_rd(&mut client, file)?),
            None => None,
        };
        let buffer = buffer_over(&mut client, source)?;
        let h = watched(&mut client, buffer, source)?;
        let text = String::from_utf8_lossy(&client.read_whole(h)?).into_owned();
        client.since()?;
        let mut caps = Caps::default();
        for api in [keys::hash(), grid::hash(), process::hash()] {
            caps.serve(api);
        }
        caps.sized(u64::from(FIRST.0), u64::from(FIRST.1 - 1));
        let at = line.map_or(0, |line| at_line(&text, line));
        let mut tui = Self {
            client,
            plugin,
            caps,
            buffer,
            source,
            h,
            name: String::from(UNNAMED),
            text,
            head: 0,
            published: 0,
            size: FIRST,
            moved: false,
        };
        tui.name = tui.named()?;
        let start = Start {
            client: tui.client.cid(),
            buffer,
            source,
            dir: String::from(dir),
            at: at as u64,
        };
        let ctx = Ctx { tree: tui.client.tree(), caps: &mut tui.caps };
        tui.plugin.start(ctx, start)?;
        tui.refresh()?;
        tui.moved = false;
        Ok(tui)
    }

    /// The Buffer rd.
    #[must_use]
    pub const fn buffer(&self) -> Rd {
        self.buffer
    }

    /// The Plugin behind the keys, for a test to judge its state.
    #[must_use]
    pub const fn plugin(&self) -> &P {
        &self.plugin
    }

    /// One key to the Plugin, as a `keys::Press`.
    ///
    /// # Errors
    /// What the Client says.
    pub fn key(&mut self, key: Key) -> Result<Step, Fault> {
        let press = reovim_sdk::encode(&keys::Press { key: key.code() });
        let ctx = Ctx { tree: self.client.tree(), caps: &mut self.caps };
        self.plugin.input(ctx, keys::hash(), &press)?;
        self.after()?;
        Ok(self.step())
    }

    /// What happened since, to the Plugin.
    ///
    /// # Errors
    /// `Kernel(Gone)` once the Buffer is gone; otherwise what the
    /// Client says.
    pub fn tick(&mut self) -> Result<Step, Fault> {
        self.after()?;
        Ok(self.step())
    }

    /// What to draw for `cols` by `rows`: the View laid out over the
    /// tui's own text. A size the Plugin has not been told of reaches
    /// it first, as a `grid::Resized`, so it can move the View.
    ///
    /// # Errors
    /// What the Client says.
    pub fn frame(&mut self, cols: u16, rows: u16) -> Result<Frame, Fault> {
        if (cols, rows) != self.size {
            self.size = (cols, rows);
            let (cols, rows) =
                (u64::from(cols), u64::from(rows.saturating_sub(1)));
            self.caps.sized(cols, rows);
            let resized = reovim_sdk::encode(&grid::Resized { cols, rows });
            let ctx = Ctx { tree: self.client.tree(), caps: &mut self.caps };
            self.plugin.input(ctx, grid::hash(), &resized)?;
        }
        Ok(self.laid_out(cols, rows))
    }

    /// The events drained: the tui's own copy of the text read again
    /// on a Modify or an Overflow, the revisions and the name after
    /// it, then the Plugin told what happened.
    fn after(&mut self) -> Result<(), Fault> {
        let events = self.client.since()?;
        if events.is_empty() {
            return Ok(());
        }
        let on = |rd: Rd, kind: Kind| {
            events.iter().any(|e| e.rd == rd && e.kind == kind)
        };
        if on(self.buffer, Kind::Gone) {
            return Err(Fault::Kernel(Error::Gone));
        }
        // Overflow: the ring moved past us, so everything is read again.
        let overflow = events.iter().any(|e| e.kind == Kind::Overflow);
        let source = self.source.is_some_and(|s| on(s, Kind::Modify));
        if overflow || on(self.buffer, Kind::Modify) || source {
            let bytes = self.client.read_whole(self.h)?;
            let text = String::from_utf8_lossy(&bytes).into_owned();
            self.moved |= text != self.text;
            self.text = text;
        }
        self.refresh()?;
        let ctx = Ctx { tree: self.client.tree(), caps: &mut self.caps };
        self.plugin.changed(ctx, &events)
    }

    /// Quit once the Plugin has asked to exit; Changed once it has
    /// shown something or the Buffer has moved under it.
    fn step(&mut self) -> Step {
        if let Some(code) = self.caps.exit() {
            return Step::Quit(code);
        }
        let shown = self.caps.shown();
        let moved = core::mem::take(&mut self.moved);
        if shown || moved { Step::Changed } else { Step::Same }
    }

    /// Head and published from `tree`, and the name the Buffer goes by
    /// now: a `:w NAME` on a Buffer over nothing gives it one.
    fn refresh(&mut self) -> Result<(), Fault> {
        let tree = history::Call(self.client.tree(), self.h).tree()?;
        let moved = (tree.head, tree.published) != (self.head, self.published);
        self.moved |= moved;
        (self.head, self.published) = (tree.head, tree.published);
        // A Buffer over nothing may have taken a source since.
        if self.source.is_none() {
            self.name = self.named()?;
        }
        Ok(())
    }

    /// What the Buffer is over, by name, or `[No Name]`. A Buffer that
    /// has taken a source is watched from then on, as its own was.
    fn named(&mut self) -> Result<String, Fault> {
        if let Some(source) = self.source {
            return self.client.name(source);
        }
        let Ok(child) = self.client.lookup(self.buffer, "source") else {
            return Ok(String::from(UNNAMED));
        };
        let source: Rd = reovim_sdk::decode(&self.client.read_all(child)?)?;
        let watched = self.client.open(source, Rights::READ)?;
        self.client.watch(watched, true)?;
        self.source = Some(source);
        self.moved = true;
        self.client.name(source)
    }

    /// The View in cells: the lines from its top, the spans and the
    /// cursor on them, and the status row.
    fn laid_out(&self, cols: u16, rows: u16) -> Frame {
        let width = usize::from(cols.max(1));
        let height = usize::from(rows.saturating_sub(1).max(1));
        let Some(view) = self.caps.view() else {
            return Frame { status: self.status(0, ""), ..Frame::default() };
        };
        let cursor = boundary(&self.text, at_of(view.cursor));
        let mut frame = Frame {
            status: self.status(cursor, &view.status),
            ..Frame::default()
        };
        let mut at =
            start_of(&self.text, boundary(&self.text, at_of(view.top)));
        let last = self.text.len() - usize::from(self.text.ends_with('\n'));
        while frame.lines.len() < height && at <= last {
            let end = end_of(&self.text, at);
            let row = cell(frame.lines.len());
            frame.lines.push(shown(&self.text[at..end], width));
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
        view: &grid::View,
        row: u16,
        (at, end): (usize, usize),
        (cursor, width): (usize, usize),
    ) {
        let line = &self.text[at..end];
        let cells = |span: &grid::Span| {
            span_on(line, at, end, span).map(|at| (span.style, at))
        };
        for (style, (from, to)) in view.spans.iter().filter_map(cells) {
            let marked = match style {
                SELECTION => &mut frame.selection,
                MATCH => &mut frame.marks,
                _ => continue,
            };
            marked.push((row, cell(from), cell(to)));
        }
        if (at..=end).contains(&cursor) {
            frame.cursor =
                (row, cell(cell_at(line, cursor - at).min(width - 1)));
        }
    }

    /// The status row: the name, where the cursor is, the revision and
    /// whether it is written, then the Plugin's own word.
    fn status(&self, cursor: usize, word: &str) -> String {
        let before = &self.text[..boundary(&self.text, cursor)];
        let line = before.matches('\n').count() + 1;
        let column = before.rsplit('\n').next().unwrap_or("").chars().count();
        let mark = if self.head == self.published { "" } else { "[+]" };
        format!(
            "{}  {line}:{}  r{}{mark}   {word}",
            self.name,
            column + 1,
            self.head
        )
    }
}

/// The rd of `file`, a name under the root.
fn file_rd(client: &mut Client, file: &str) -> Result<Rd, Fault> {
    client.resolve(&format!("/module/fs/root/{file}"))
}

/// A Buffer over `source`, or over nothing.
fn buffer_over(client: &mut Client, source: Option<Rd>) -> Result<Rd, Fault> {
    let opener = client.resolve("/module/text/open")?;
    let opening = client.open(opener, Rights::WRITE)?;
    let mut open = open::Call(client.tree(), opening);
    let buffer = match source {
        Some(source) => open.open(source),
        None => open.new(),
    };
    client.close(opening)?;
    buffer
}

/// A handle on `buffer`, watched, with the `source` watched too.
fn watched(
    client: &mut Client,
    buffer: Rd,
    source: Option<Rd>,
) -> Result<Handle, Fault> {
    let h = client.open(buffer, Rights::READ.union(Rights::WRITE))?;
    client.watch(h, true)?;
    if let Some(source) = source {
        let watched = client.open(source, Rights::READ)?;
        client.watch(watched, true)?;
    }
    Ok(h)
}

/// The byte line `n` starts at, counted from 1 and clamped to the last
/// line: the tty's `+N`.
fn at_line(text: &str, n: usize) -> usize {
    let mut at = 0;
    for _ in 1..n.max(1) {
        match text[at..].find('\n') {
            Some(i) if at + i + 1 < text.len() => at += i + 1,
            _ => break,
        }
    }
    at
}

/// A byte of the Buffer as an index, however far the Plugin put it.
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
/// char starts, but a tab's last cell, as nvim draws it.
fn cell_at(line: &str, at: usize) -> usize {
    let cells = cells_upto(line, at);
    match line[at.min(line.len())..].chars().next() {
        Some('\t') => next_cell(cells, '\t') - 1,
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
    span: &grid::Span,
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

/// The cells `c` takes on the screen.
///
/// Two for the East Asian wide and fullwidth chars a terminal draws
/// double (Hangul, the CJK ideographs, kana, the fullwidth forms,
/// emoji), one for the rest. A tab is the caller's, as its width
/// depends on where it stands.
#[must_use]
pub const fn width(c: char) -> usize {
    let wide = matches!(
        c as u32,
        0x1100..=0x115F
            | 0x2E80..=0x303E
            | 0x3041..=0x33FF
            | 0x3400..=0x4DBF
            | 0x4E00..=0x9FFF
            | 0xA000..=0xA4CF
            | 0xAC00..=0xD7A3
            | 0xF900..=0xFAFF
            | 0xFE30..=0xFE4F
            | 0xFF00..=0xFF60
            | 0xFFE0..=0xFFE6
            | 0x1F300..=0x1F64F
            | 0x1F900..=0x1F9FF
            | 0x20000..=0x3FFFD
    );
    if wide { 2 } else { 1 }
}
