#![no_std]
#![forbid(unsafe_code)]
//! The tui: one region of a terminal, with a `Plugin` showing an rd in
//! it.
//!
//! It serves four capabilities and nothing else. `keys` carries every
//! key to the Plugin; `grid` is the one region it draws, whose size it
//! answers and whose View it keeps; `open` is how a Plugin asks for
//! another rd to be shown here, in another Plugin; `process` is the
//! code it leaves with. The Plugin makes its own calls on the tree, so
//! what a key means, what it edits and what it says are none of the
//! tui's business: it lays the View out over its own copy of the text
//! of the rd that View names.
//!
//! Which Plugin a name or a shape stands for is the app's, given as a
//! `Table`; the tui knows no Plugin name and no Plugin type. It sees
//! neither a link nor a terminal.

extern crate alloc;

use alloc::boxed::Box;
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use reovim_client::{
    BAR, Caps, Client, Ctx, Error, Event, Fault, Flags, Handle, Key, Kind,
    MATCH, Plugin, Rd, Rights, SELECTION, Start, grid, keys, open, process,
};
use reovim_text_api::history;

/// The cells a char takes: the Client's own, drawn on here and by the
/// app above.
pub use reovim_client::width;

/// What a key or a tick did.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Step {
    Same,
    /// Draw again.
    Changed,
    /// Leave with this exit code: 0 says use the file.
    Quit(u8),
}

/// What a Client asks its table for.
///
/// A name is the one a Plugin gave `open`; a shape is what the rd to
/// be shown is, a Container or a cell, for an `open` that named no
/// Plugin.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Want<'a> {
    Named(&'a str),
    Container,
    Cell,
}

/// How the region's Plugin is made, which is the app's to say: the
/// names and the types are the app's, and a Want it has no Plugin for
/// is `None`.
pub type Table<P> = Box<dyn FnMut(Want<'_>) -> Option<P>>;

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
    /// The cursor's shape, `BLOCK` or `BAR`, for the app to set on the
    /// terminal: the cell it names is the one it is drawn over, or the
    /// one it stands before.
    pub shape: u64,
    pub selection: Vec<(u16, u16, u16)>,
    /// Search matches, in the selection's shape.
    pub marks: Vec<(u16, u16, u16)>,
    pub status: String,
}

/// The name of a Buffer over nothing, as nvim shows it.
const UNNAMED: &str = "[No Name]";
/// What it serves, one name per line: its capability cell.
const SERVES: &[u8] = b"keys\ngrid\nopen\nprocess\n";
/// The size it takes the terminal for until the app has drawn once.
const FIRST: (u16, u16) = (80, 24);
/// The level an entry the tui writes to `/sys/log` goes in at.
const WARN: &str = "<4>";

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

/// One region over one rd, with the Plugin that shows it.
pub struct Tui<P: Plugin> {
    client: Client,
    plugin: P,
    table: Table<P>,
    caps: Caps,
    /// The rd the region was opened on, when it was opened on one.
    target: Option<Rd>,
    /// The path under the root the tui was opened with while there is
    /// no such file: what a write makes.
    opened: Option<String>,
    /// Where a relative name the Plugin resolves starts.
    dir: String,
    /// What the View last named, opened and watched here.
    laid: Option<Laid>,
    /// What the status row calls the region.
    name: String,
    /// The size `frame` was last asked for.
    size: (u16, u16),
    /// The text or the revisions moved since the last step.
    moved: bool,
}

impl<P: Plugin> Tui<P> {
    /// The region over `file`, a name under the root, or over nothing
    /// when there is no such file; the Plugin `table` gives for what
    /// that rd is, a Container or a cell, started on it at `line`,
    /// counted from 1. `dir`, under the root too, is where a relative
    /// name the Plugin resolves starts.
    ///
    /// What to open over that rd is the Plugin's: the tui opens no
    /// Buffer, and lays out whatever rd the View it is shown names.
    ///
    /// # Errors
    /// What the Client says; `Usage` when the table has no Plugin for
    /// that shape.
    pub fn open(
        mut client: Client,
        dir: &str,
        file: Option<&str>,
        line: Option<usize>,
        mut table: Table<P>,
    ) -> Result<Self, Fault> {
        client.capability(SERVES.to_vec())?;
        let target = match file {
            Some(file) => file_rd(&mut client, file)?,
            None => None,
        };
        let opened = match target {
            Some(_) => None,
            None => file.map(String::from),
        };
        // `reovim DIR` is a region over a Container, which is the
        // explorer's shape, not Vim's: the first Plugin comes from the
        // table by the same shape a later `open` asks it for.
        let want = match target {
            Some(target) => want_of(&mut client, target)?,
            None => Want::Cell,
        };
        let Some(plugin) = table(want) else {
            return Err(Fault::Usage(format!("no Plugin for {}", shape(want))));
        };
        let mut caps = Caps::default();
        for api in [keys::hash(), grid::hash(), open::hash(), process::hash()] {
            caps.serve(api);
        }
        caps.sized(u64::from(FIRST.0), u64::from(FIRST.1 - 1));
        let mut tui = Self {
            client,
            plugin,
            table,
            caps,
            target,
            opened,
            dir: String::from(dir),
            laid: None,
            name: String::from(UNNAMED),
            size: FIRST,
            moved: false,
        };
        tui.name = tui.named()?;
        let start = Start {
            client: tui.client.cid(),
            over: target,
            name: tui.opened.clone(),
            dir: String::from(dir),
            line: line.map(count),
        };
        let ctx = Ctx { tree: tui.client.tree(), caps: &mut tui.caps };
        tui.plugin.start(ctx, start)?;
        tui.settled()?;
        // What the Plugin's own opening left on the watched rds is
        // already in the text read with them.
        tui.client.since()?;
        tui.moved = false;
        Ok(tui)
    }

    /// The rd laid out now: what the last View named.
    #[must_use]
    pub fn over(&self) -> Option<Rd> {
        self.laid.as_ref().map(|laid| laid.rd)
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
        self.settled()?;
        Ok(self.step())
    }

    /// What happened since, to the Plugin.
    ///
    /// # Errors
    /// `Kernel(Gone)` once what is laid out is gone; otherwise what the
    /// Client says.
    pub fn tick(&mut self) -> Result<Step, Fault> {
        // What the Plugin did about what happened is itself something
        // that happened: after a key its calls are drained by the
        // `after` below `input`, and after a tick by this second one,
        // so a listing that refills on a watch is in this frame and
        // not the next. A tick that found nothing still asks once.
        if self.after()? {
            self.after()?;
        }
        self.settled()?;
        Ok(self.step())
    }

    /// What to draw for `cols` by `rows`: the View laid out over the
    /// tui's own copy of the text it names. A size the Plugin has not
    /// been told of reaches it first, as a `grid::Resized`, so it can
    /// move the View.
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
            // A View is laid out wherever it arrives; an ask waits for
            // the next step, a frame not being one.
            self.lay()?;
        }
        Ok(self.laid_out(cols, rows))
    }

    /// The events drained: the laid text read again on a Modify or an
    /// Overflow, the revisions and the name after it, then the Plugin
    /// told what happened. True when there was anything. Nothing
    /// happened is nothing asked of the tree, so an idle editor makes
    /// one call a tick.
    fn after(&mut self) -> Result<bool, Fault> {
        let events = self.client.since()?;
        if events.is_empty() {
            return Ok(false);
        }
        self.reread(&events)?;
        self.refresh()?;
        let ctx = Ctx { tree: self.client.tree(), caps: &mut self.caps };
        self.plugin.changed(ctx, &events)?;
        Ok(true)
    }

    /// What the Plugin's calls left behind: the region opened
    /// elsewhere if it asked for that, and the rd the View names laid
    /// out. A View over what is laid already asks nothing of the tree.
    fn settled(&mut self) -> Result<(), Fault> {
        self.swapped()?;
        self.lay()
    }

    /// The tui's own copy of the laid text, read again when the events
    /// say it moved: a Modify on it or on what it is over, or a ring
    /// that overflowed past the tui. Its going is the end of the
    /// editor.
    fn reread(&mut self, events: &[Event]) -> Result<(), Fault> {
        let Some(laid) = &self.laid else { return Ok(()) };
        let (rd, h) = (laid.rd, laid.h);
        let source = laid.source.map(|(source, _)| source);
        let on = |rd: Rd, kind: Kind| {
            events.iter().any(|e| e.rd == rd && e.kind == kind)
        };
        if on(rd, Kind::Gone) {
            return Err(Fault::Kernel(Error::Gone));
        }
        // Overflow: the ring moved past us, so everything is read again.
        let overflow = events.iter().any(|e| e.kind == Kind::Overflow);
        let over = source.is_some_and(|source| on(source, Kind::Modify));
        if !overflow && !on(rd, Kind::Modify) && !over {
            return Ok(());
        }
        let bytes = self.client.read_whole(h)?;
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
    fn lay(&mut self) -> Result<(), Fault> {
        let named = self.caps.view().map(|view| Rd(view.over));
        let Some(rd) = named else { return Ok(()) };
        if self.over() == Some(rd) {
            return Ok(());
        }
        let laid = lay_on(&mut self.client, rd)?;
        if let Some(before) = self.laid.replace(laid) {
            unlay(&mut self.client, &before)?;
        }
        self.moved = true;
        // The name first: it is what takes the source, which the
        // revisions after it then see.
        self.name = self.named()?;
        self.refresh()
    }

    /// What a Plugin asked of `open`, acted on now that its call has
    /// returned: the Plugin the table gives, the handles the Plugin
    /// being replaced left closed for it, and the region started over
    /// the rd asked for.
    ///
    /// One ask per step, so a Plugin that asks in its own `start` is
    /// served at the next: the tui answers keys, it does not chase
    /// them.
    fn swapped(&mut self) -> Result<(), Fault> {
        let Some((target, plugin)) = self.caps.asked() else { return Ok(()) };
        let Some(made) = self.made(target, &plugin)? else {
            return self.unsupported(target, &plugin);
        };
        self.shut()?;
        self.plugin = made;
        self.target = Some(target);
        self.opened = None;
        let start = Start {
            client: self.client.cid(),
            over: Some(target),
            name: None,
            dir: self.dir.clone(),
            line: None,
        };
        let ctx = Ctx { tree: self.client.tree(), caps: &mut self.caps };
        self.plugin.start(ctx, start)
    }

    /// The Plugin for an ask: the one the name stands for, or, with no
    /// name, the one the target's shape stands for.
    fn made(&mut self, target: Rd, plugin: &str) -> Result<Option<P>, Fault> {
        if !plugin.is_empty() {
            return Ok((self.table)(Want::Named(plugin)));
        }
        let want = want_of(&mut self.client, target)?;
        Ok((self.table)(want))
    }

    /// An ask the table has no Plugin for: the region stays as it is
    /// and the log says so, `open` carrying no answer back to the
    /// Plugin that asked.
    fn unsupported(&mut self, target: Rd, plugin: &str) -> Result<(), Fault> {
        let entry = self.client.resolve("/sys/log")?;
        let said =
            format!("{WARN}open rd={} plugin={plugin} unsupported", target.0);
        self.client.write_all(entry, said.into_bytes()).map(|_| ())
    }

    /// Every handle the Tree holds but the ones the tui lays out with:
    /// what the Plugin being replaced opened, closed for it, nothing
    /// else holding a Plugin's end.
    fn shut(&mut self) -> Result<(), Fault> {
        let mut kept: Vec<Handle> = Vec::new();
        if let Some(laid) = &self.laid {
            kept.push(laid.h);
            kept.extend(laid.source.map(|(_, watched)| watched));
        }
        let theirs: Vec<Handle> = self
            .client
            .handles()
            .iter()
            .copied()
            .filter(|h| !kept.contains(h))
            .collect();
        for h in theirs {
            shut_one(&mut self.client, h)?;
        }
        Ok(())
    }

    /// Quit once the Plugin has asked to exit; Changed once it has
    /// shown something or what is laid out has moved under it.
    fn step(&mut self) -> Step {
        if let Some(code) = self.caps.exit() {
            return Step::Quit(code);
        }
        let shown = self.caps.shown();
        let moved = core::mem::take(&mut self.moved);
        if shown || moved { Step::Changed } else { Step::Same }
    }

    /// Head and published from `tree`, and the name the region goes by
    /// now: a `:w NAME` on a Buffer over nothing gives it one.
    fn refresh(&mut self) -> Result<(), Fault> {
        let Some(laid) = &self.laid else { return Ok(()) };
        let (h, over) = (laid.h, laid.source.is_some());
        let tree = history::Call(self.client.tree(), h).tree()?;
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
    /// the tui was opened with, else `[No Name]`.
    fn named(&mut self) -> Result<String, Fault> {
        if let Some(source) = self.source()? {
            return self.client.name(source);
        }
        if let Some(target) = self.target {
            return self.client.name(target);
        }
        let opened = self.opened.as_deref().map(own_name);
        Ok(String::from(opened.unwrap_or(UNNAMED)))
    }

    /// What the laid rd is over, once it is over anything: its
    /// `source` child, read once and watched from then on, as a
    /// Buffer opened over a file is.
    fn source(&mut self) -> Result<Option<Rd>, Fault> {
        let Some(laid) = &self.laid else { return Ok(None) };
        if let Some((source, _)) = laid.source {
            return Ok(Some(source));
        }
        let Ok(child) = self.client.lookup(laid.rd, "source") else {
            return Ok(None);
        };
        let source: Rd = reovim_sdk::decode(&self.client.read_all(child)?)?;
        let watched = self.client.open(source, Rights::READ)?;
        self.client.watch(watched, true)?;
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
        let Some(view) = self.caps.view() else {
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
        view: &grid::View,
        row: u16,
        (at, end): (usize, usize),
        (cursor, width): (usize, usize),
    ) {
        let line = &self.text()[at..end];
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

/// What the table is asked for an rd: the Plugin of a Container, or
/// the Plugin of a cell, nothing on disk being a cell's shape too.
fn want_of(client: &mut Client, target: Rd) -> Result<Want<'static>, Fault> {
    let flags = client.info(target)?.flags;
    if flags.contains(Flags::CONTAINER) {
        return Ok(Want::Container);
    }
    Ok(Want::Cell)
}

/// What a shape is called where the tui says it has no Plugin for one.
const fn shape(want: Want<'_>) -> &'static str {
    match want {
        Want::Container => "a Container",
        _ => "a cell",
    }
}

/// `rd` as the tui lays a View out over it: opened READ, watched, and
/// read whole.
fn lay_on(client: &mut Client, rd: Rd) -> Result<Laid, Fault> {
    let h = client.open(rd, Rights::READ)?;
    client.watch(h, true)?;
    let text = String::from_utf8_lossy(&client.read_whole(h)?).into_owned();
    Ok(Laid { rd, h, source: None, text, head: 0, published: 0 })
}

/// One handle of the Plugin being replaced, closed. A handle whose rd
/// went while the Plugin held it is gone with it; what matters is that
/// nothing of the Plugin's is left open.
fn shut_one(client: &mut Client, h: Handle) -> Result<(), Fault> {
    match client.close(h) {
        Ok(()) | Err(Fault::Kernel(_)) => Ok(()),
        Err(fault) => Err(fault),
    }
}

/// What was laid out before, let go: its handle and the one on what it
/// was over.
fn unlay(client: &mut Client, laid: &Laid) -> Result<(), Fault> {
    if let Some((_, watched)) = laid.source {
        client.close(watched)?;
    }
    client.close(laid.h)
}

/// What a file under the root goes by on its own: the last segment of
/// `path`, which is the name its rd has under its directory.
fn own_name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// The rd of `file`, a name under the root, or `None` when there is
/// no such file: what a region over nothing is opened for. The two
/// ways the tree says nothing is there are a name not on disk and a
/// name below a file.
fn file_rd(client: &mut Client, file: &str) -> Result<Option<Rd>, Fault> {
    match client.resolve(&format!("/module/fs/root/{file}")) {
        Err(Fault::Kernel(Error::Gone | Error::Unsupported)) => Ok(None),
        found => found.map(Some),
    }
}

/// A count as the contract carries it, however far it goes.
fn count(n: usize) -> u64 {
    u64::try_from(n).unwrap_or(u64::MAX)
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
