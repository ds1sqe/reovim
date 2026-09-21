#![no_std]
#![forbid(unsafe_code)]
//! Vim: keys in, calls out, over the text and the cursor it holds; a
//! Module a Client loads.
//!
//! The engine below is pure: a key becomes an `Effect`, which is what
//! the keys asked for and has already happened to Vim's own copy of
//! the text. plugin.rs is what performs one: the edits go to the
//! Buffer, the undo to its history, the save to its source, and what
//! it all looks like is the View the region's one read answers with.
//! Nothing of the screen is Vim's: it keeps the first line shown and
//! the cursor, and the Client turns those into cells.
//!
//! The text is the Buffer's bytes. A `\n` ends a line; the last one
//! ends the last line rather than starting an empty one, as Vim has
//! it, and a text without one is a last line without an end.

extern crate alloc;

pub mod bins;
mod command;
mod global;
mod jumps;
mod keymap;
mod macros;
mod marks;
mod motion;
mod object;
mod operator;
mod plugin;
mod search;
mod visual;
mod word;

use alloc::collections::BTreeMap;
use alloc::string::String;
use alloc::vec::Vec;

pub use marks::Pos;
pub use operator::Motion;
pub use plugin::Editor;
pub use reovim_client_api::{Key, notation};
pub use search::{Filled, Want};
pub use visual::Visual;

use jumps::{Jump, Jumps};
use keymap::{Fact, Held, Keymap};
use marks::Marks;
use operator::{Find, Operator};
use reovim_client_api::keyboard::Press;
use reovim_client_api::resolver::Input;
use reovim_client_api::{BAR, BLOCK, MATCH, NONE, SELECTION, origin, window};
use reovim_client_sh::{Line, Shell};
use reovim_kernel_interface::{Handle, Rd};
use search::{Found, SPANS, Search};
use visual::{BlockInsert, Selection};

/// One behaviour reovim's default has of its own, where nvim has
/// another: the keys it is on, what they do here, and what they do
/// under `:set vi`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct Divergence {
    /// The keys the two ways part on.
    pub keys: &'static str,
    /// What the default does, which is reovim's own style.
    pub ours: &'static str,
    /// What `:set vi` does instead, which is nvim's way.
    pub vi: &'static str,
}

/// Every behaviour the default has of its own: the one list of them,
/// so none is hidden. `:set vi` takes each back to nvim's way, and
/// `:help divergences` prints the table.
///
/// A row is documentation; the switch is the branch on
/// `Options::vi` at the one place the behaviour lives. A divergence
/// added without its row, or a row without its branch, is the pinned
/// count in tests/tab.rs to change on purpose.
///
/// The three Vim-versus-nvim divergences the corpus carries (`Y`, the
/// column a file opens at, `Vjd` under `nostartofline`) are Vim's
/// against nvim's, not reovim's: the default follows nvim on all
/// three, so `:set vi` leaves them alone and they are no rows.
pub const DIVERGENCES: &[Divergence] = &[Divergence {
    keys: "Tab on the : line",
    ours: "the name expanded, with the usage line it declares",
    vi: "the wildmenu, one candidate as several, and no help",
}];

/// What a name of Vim's says of a Buffer over nothing: a write has
/// no file to write and a point has no path to name.
pub(crate) const E32: &str = "E32: No file name";

/// One change to the text, already made in Vim: `len` bytes at `at`
/// became `text`.
///
/// With `join` the edit belongs to the revision under way and not a
/// new one, so one insert is one undo step; without it the Plugin ends
/// that revision before this edit goes into the next.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Edit {
    pub at: usize,
    pub len: usize,
    pub text: String,
    pub join: bool,
}

/// What a key did, before the calls it asks for are made.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Effect {
    /// Keys for the host's queue, never keyboard writes: a macro's
    /// run, or, with the flag, the keys `.` answered, which run as a
    /// repeat's. The records' `Fed { keys, repeat }`.
    ///
    /// The host makes an [`Input`] of each, carrying the press of the
    /// key that made them — the `@a` or the `.` — so no clock and no
    /// source is invented. The engine replays nothing itself, so a
    /// key of them that cannot be answered in its turn is an `Effect`
    /// like any other.
    ///
    /// ```
    /// use reovim_client_api::{Key, NONE, keyboard::Press, origin};
    /// use reovim_plugin_vim::{Effect, Vim, keyed, notation};
    /// let mut vim = Vim::default();
    /// vim.loaded(b"abc\n");
    /// let none = Press { key: 0, when: 0, source: NONE };
    /// for key in notation::keys("qaxq@").unwrap() {
    ///     vim.key(keyed(key, origin::TYPED, &none));
    /// }
    /// assert_eq!(vim.key(keyed(Key::Char('a'), origin::TYPED, &none)),
    ///     Effect::Fed(vec![Key::Char('x')], false));
    /// ```
    Fed(Vec<Key>, bool),
    Nothing,
    /// The cursor or the mode moved; the text did not.
    Moved,
    Edit(Edit),
    /// Several edits in order, as a repeat replays them.
    Edits(Vec<Edit>),
    Undo,
    Redo,
    /// `:w`; `true` for `:w!`, which writes over a file that appeared
    /// under the name a Buffer over nothing was opened with.
    Save(bool),
    /// `:w NAME`: the draft written to that file, resolved under the
    /// directory the Client started Vim in; `true` for `:w! NAME`.
    SaveAs(String, bool),
    Quit(Quit),
    /// Navigate the Client's source-backed drafts, retaining unsaved text.
    Switch(Switch),
    /// Close a draft by its decimal tree name; empty means current.
    /// The bool forces departure from a modified draft. Closing the
    /// current draft is deferred until its stay has ended.
    Close(String, bool),
    /// Print the source-backed drafts, current/alternate flags and names.
    List,
    /// Write the modified current draft and retained source-backed drafts.
    /// Later candidates are attempted after failure; the Client stays.
    SaveAll,
    /// `:exp[lore][!] [NAME]`: the Client asked to show an rd in this
    /// region, in the Plugin its table gives that rd's shape. The name
    /// given, resolved as `:w NAME` is, or the directory the file is
    /// in with none; `true` for `:explore!`, which goes although the
    /// draft is unsaved.
    Explore(Option<String>, bool),
    /// `:e[dit][!] PATH`: the same ask over the file that name holds,
    /// which comes back a Vim of its own. A missing ordinary path
    /// opens a named draft without creating the file.
    Open(String, bool),
    /// `:e` with no name: the Buffer read again. `true` for `:e!`,
    /// which drops what has not gone down whatever it is; without it
    /// a stale draft says W12 instead and reads nothing.
    Reload(bool),
    /// A `:` line with a name on it: the range Vim read off its front,
    /// and the rest of it, which the Client runs.
    Line(Ex),
    /// `:cli[!]`: a shell in a region of its own, asked for by name
    /// over where the `:` line's own lines stand. `true` for `:cli!`,
    /// which goes although the draft is unsaved.
    Cli(bool),
    /// Tab on the `:` line: the line as it stands, which only the
    /// Client can complete, the names and the paths both being in the
    /// tree. What it finds it hands back to the line.
    Complete(String),
    /// A put wants the clipboard's text: `pasted` finishes it.
    Clipboard,
    /// A search key the list Vim holds cannot answer: the list to
    /// fill, which the Plugin opens over its draft and reads.
    /// `matched` finishes it, running the motion that waited again.
    Search(Want),
    /// A file mark or a jump list entry whose source is not the
    /// loaded Buffer's: the source to show here instead, and the
    /// position and how it lands, for the Plugin to read into
    /// `arrive` once the ask has gone out; see that field for the
    /// rest of the story.
    Goto(Rd, Pos, Land),
}

/// One `:` line as Vim read it: the range off its front, in lines from
/// 0 as the engine counts them, and the rest of it, whole and as it was
/// typed.
///
/// What the first token names is none of the engine's business: the
/// Client expands it under Vim's own `bin` and runs it there, or hands
/// the whole line to the shell.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Ex {
    /// Whether a range was explicitly typed, even when it is one line.
    /// `:g/x/d` is false; `:1g/x/d` is true.
    pub ranged: bool,
    pub from: u64,
    pub to: u64,
    pub line: String,
}

/// How a quit was asked for; what it does with unsaved changes
/// depends on the Buffer's revisions.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Quit {
    /// `:q`: refused with E37 while there are unsaved changes.
    Ask,
    /// `:q!`, `ZQ`: skip this draft; other modified drafts still refuse.
    Force,
    /// `:wq`, `:x`, `ZZ`: write this draft, then check the others.
    /// Another modified draft refuses exit and becomes current.
    Write,
    /// `:cq`: exit 1, so the caller uses nothing.
    Abort,
    /// `:qall[!]`: check all drafts unless forced. A true argument
    /// permits exit without writing any of their modifications.
    All(bool),
    /// `:wqall`: attempt every required write and leave only if all succeed.
    /// Unchanged unnamed and missing-file drafts require no write.
    WriteAll,
}

/// A request to select a retained source-backed draft.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum Switch {
    /// The following source-backed draft, wrapping at the end.
    Next,
    /// The preceding source-backed draft, wrapping at the start.
    Previous,
    /// A decimal name under `/self/dev/buffers`, or `#` for the alternate.
    /// This is a Client draft name, not a source rd or filesystem path.
    Named(String),
    /// The source last left for another source; a source-less draft
    /// never becomes the alternate, and a missing alternate is E23.
    ///
    /// ```
    /// use reovim_client_api::{Key, NONE, keyboard::Press, origin};
    /// use reovim_plugin_vim::{Effect, Switch, Vim, keyed};
    /// let none = Press { key: 0, when: 0, source: NONE };
    /// for key in [Key::Ctrl('^'), Key::Ctrl('6')] {
    ///     assert_eq!(Vim::default().key(keyed(key, origin::TYPED, &none)),
    ///         Effect::Switch(Switch::Alternate));
    /// }
    /// ```
    Alternate,
}

/// Where a jump lands.
///
/// `'x`'s own `Line`, to the first non-blank; `` `x ``'s own `Exact`,
/// on the mark's own column; `<C-o>`'s and `<Tab>`'s own `Walked`,
/// exact like `Exact` but not itself a jump — `jump_to` sets neither
/// `'` nor a jump-list entry for it, and `leave` reads it straight off
/// `arrive` to know a walk of the list from a real jump.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Land {
    Line,
    Exact,
    Walked,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Mode {
    #[default]
    Normal,
    Insert,
    Command,
    Visual(Visual),
}

impl Mode {
    /// The mode's name, which `/self/vim/mode` reads. It is a name and
    /// not the status row's words: a reader that wants `-- INSERT --`
    /// is drawing a statusline and can spell it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Normal => "normal",
            Self::Insert => "insert",
            Self::Command => "command",
            Self::Visual(Visual::Char) => "visual",
            Self::Visual(Visual::Line) => "visual line",
            Self::Visual(Visual::Block) => "visual block",
        }
    }
}

/// What a register holds: text, and its shape.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Register {
    pub text: String,
    pub kind: Kind,
}

/// The shape of a register's text: chars, whole lines, or a block of
/// columns, whose lines the text holds joined by newlines.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Kind {
    #[default]
    Char,
    Line,
    Block,
}

/// What a normal-mode command has typed so far.
#[derive(Debug, Default)]
struct Pending {
    /// Digits typed before a command; 0 is none.
    count: usize,
    /// A `g`, `d`, `Z`, `f` or the like waiting for its second key.
    key: Option<char>,
    /// An operator waiting for its motion, with its own count.
    operator: Option<(Operator, usize)>,
    /// `"x` typed: the register the next operator or put uses.
    register: Option<char>,
    /// A motion is being run for an operator, not for the cursor.
    running: bool,
}

/// What `.` needs: the keys of the command under way and of the last
/// change.
#[derive(Debug, Default)]
struct Repeat {
    record: Vec<Key>,
    last: Vec<Key>,
    /// `.` is feeding keys back in.
    replaying: bool,
    /// A repeat's keys are at the queue's front and none of them has
    /// edited yet: the first that does pushes the undo cursor, as a
    /// typed change's first edit does. One stay's, cleared by
    /// `leave()`.
    first: bool,
}

/// A put waiting for the clipboard's text.
#[derive(Clone, Copy, Debug)]
struct Put {
    after: bool,
    count: usize,
    /// The selection it goes over, if any.
    over: Option<Visual>,
}

/// The command line: what it is for, the mode it returns to, and the
/// lines themselves, which are the shell's line editor and nothing
/// of Vim's own.
///
/// There are two of them because nvim keeps two histories: everything
/// typed after `:` walks with the `:` lines, everything after `/` and
/// `?` with the searches.
#[derive(Debug, Default)]
struct Prompt {
    /// `:`, `/` or `?`.
    at: char,
    before: Mode,
    ex: Line,
    search: Line,
}

impl Prompt {
    /// The line the open prompt is over.
    const fn line(&mut self) -> &mut Line {
        if self.at == ':' { &mut self.ex } else { &mut self.search }
    }

    /// What stands on it.
    fn text(&self) -> &str {
        if self.at == ':' { self.ex.text() } else { self.search.text() }
    }
}

/// What the last Tab on the `:` line found: the candidates, which of
/// them the line holds now, the byte of the line they replace, and the
/// usage line the status row says where there is one candidate and it
/// is a name that may run.
///
/// Several are the wildmenu, which Tab and Ctrl-P walk. One name that
/// may run is kept as the one candidate, for that usage line; one name
/// found under a Container is not kept at all, there being nothing
/// left to choose between and the next Tab going on under it.
#[derive(Debug)]
struct Wild {
    candidates: Vec<String>,
    at: usize,
    from: usize,
    form: Option<String>,
}

/// The cursor and the `a`-`z` marks as they stood before one change,
/// for undo to extend the current marks with, as nvim's `uh_namedm`
/// does; redo is `marked` alone.
#[derive(Debug)]
struct Change {
    cursor: (usize, usize),
    marks: Vec<(char, Pos)>,
}

/// Where the cursor and the marks were before each change, for undo
/// to put them back as nvim does, and how many of those changes stand
/// undone.
#[derive(Debug, Default)]
struct Undone {
    before: Vec<Change>,
    count: usize,
    /// An undo (`false`) or redo (`true`) is waiting for its reload.
    /// Never outlives one key, so a `Place` carries it for free.
    walking: Option<bool>,
}

/// What one Buffer keeps of Vim between a `left` and the `opened` that
/// finds it again: the cursor, the first line shown, the undo cursor
/// list, the visual marks and the local marks. Kept in
/// [`Vim::places`], by the draft's rd. Not the index: that is never
/// stale, always rebuilt by the `loaded` inside the `opened` that
/// restores this.
#[derive(Debug, Default)]
struct Place {
    cursor: usize,
    top: usize,
    undone: Undone,
    last_visual: Option<Selection>,
    marks: BTreeMap<char, Pos>,
}

/// The text, the cursor as a byte index on a char boundary, the first
/// line shown, and the mode. In normal mode the cursor sits on a
/// char, never past the last one of its line; insert may sit past it.
///
/// The fields below are of three kinds: one stay at the Plugin, reset
/// by `left` and filled again by the next `start`; one Buffer, kept in
/// a `Place` across a `left` and restored by the `opened` that finds
/// the same draft again; and kept, which no swap touches.
// The bell, insert joining, wanted column and visual end are independent
// state flags; combining them into an enum would invent invalid coupling.
#[allow(clippy::struct_excessive_bools)]
#[derive(Debug, Default)]
pub struct Vim {
    macros: macros::Macros,
    bell: bool,
    // One stay.
    global: global::Global,
    mode: Mode,
    pending: Pending,
    /// The keys a binding may still be, and the keys a failed prefix
    /// let go for the host's queue to run again.
    held: Held,
    /// The input now running, whose a released key reads as its
    /// maker's: every key the engine keeps or asks for carries this
    /// press, so no clock and no source is ever invented.
    taken: Option<Input>,
    /// What Vim has read of one view's `matches`, and the motion
    /// waiting on it.
    found: Found,
    /// The `/` or `?` line's preview, while one of the two is open:
    /// the origin every way out goes back to, and the entry shown.
    preview: Option<search::Preview>,
    /// The next insert edit joins the revision under way.
    joining: bool,
    /// The last key kept `want` rather than setting it.
    keep_want: bool,
    /// Bytes of indent `o`, `O` or Enter just copied, dropped again if
    /// nothing is typed on the line.
    indent: usize,
    /// Text for the clipboard, until the tui takes it.
    clipboard: Option<String>,
    put: Option<Put>,
    /// The selection's other end in visual mode.
    anchor: usize,
    /// `$` was the last motion of a selection: it reaches every line's
    /// end.
    to_end: bool,
    /// A block insert under way.
    block: Option<BlockInsert>,
    /// What Tab last found on the `:` line; any other key drops it.
    wild: Option<Wild>,
    /// Shown until the next key.
    message: String,
    /// `at` and `before`, and what stands on the two lines, reset;
    /// their histories are kept.
    prompt: Prompt,
    /// The draft `start` opened over what the region is on: what the
    /// View it shows is in the bytes of.
    buffer: Option<Rd>,
    /// That draft's handle, READ and WRITE.
    h: Option<Handle>,
    /// What the Buffer under it is over, once it is over anything.
    source: Option<Rd>,
    /// The file a Buffer over nothing was opened for while there is
    /// no such file: what its first write makes.
    name: Option<String>,

    // One Buffer, in a `Place`.
    text: String,
    /// The bytes the draft held were not UTF-8, so `text` is what
    /// `from_utf8_lossy` made of them: nothing is searched over it,
    /// the Module reading the draft's own bytes and not this. Set at
    /// the head of `loaded`, so `opened` sets it again.
    lossy: bool,
    cursor: usize,
    top: usize,
    undone: Undone,
    last_visual: Option<Selection>,
    /// This Buffer's local marks, `a`-`z` and `' [ ] < > .`, and the
    /// index that moves them.
    marks: Marks,

    // Kept.
    /// The bindings and the leader, which no swap of the region
    /// touches: a binding an `init` made outlives every Buffer.
    keymap: Keymap,
    left: Option<Rd>,
    alt: Option<Rd>,
    greeting: Option<String>,
    /// The unnamed register, `None` until something fills it; an
    /// empty yank fills it, an empty delete does not.
    register: Option<Register>,
    /// `"a` to `"z`.
    registers: BTreeMap<char, Register>,
    /// What the clipboard last gave, for `.` to put again.
    last_clip: Register,
    /// The last `f`, `t`, `F` or `T`, for `;` and `,`.
    find: Option<(Find, char)>,
    /// `.record` is reset by `left`; `.last` is kept.
    repeat: Repeat,
    search: Search,
    /// A Buffer's `Place`, from its `left` to the `opened` that finds
    /// the same draft again.
    places: BTreeMap<Rd, Place>,
    /// The screen column a vertical motion wants, `usize::MAX` after
    /// `$`.
    want: usize,
    /// Where a line of the shell's typed on the `:` line stands: the
    /// mirror of the directory the Client started in, which `:cd`
    /// moves and `:pwd` says.
    shell: Shell,
    /// Where the Client started, under the root: what the announce
    /// says a new file's name by, and nothing else.
    dir: String,
    /// The mirror's own root, where a leading `/` starts, and where
    /// the editor stands, which `:cd` moves: both `None` until
    /// `start` has found them.
    root: Option<Rd>,
    cwd: Option<Rd>,
    /// `/self/dev/clipboard`, when the Client serves one.
    clip: Option<Rd>,
    /// How many rows the region has: what `top` scrolls by.
    rows: usize,
    /// `A`-`Z`, by source: not one Buffer's, and not moved by an edit
    /// in a Buffer that is not the one they name.
    files: BTreeMap<char, (Rd, Pos)>,
    /// `<C-o>` and `<Tab>`'s own list: one for every Buffer, moved
    /// through `marked` as the file marks are.
    jumps: Jumps,
    /// A file mark or a jump list entry named by a Buffer that is not
    /// the loaded one: set by the Plugin once `Effect::Goto`'s ask has
    /// gone out, never by the engine itself, so a refused ask leaves
    /// nothing here to set. `start` takes it at its very own top,
    /// before the fallible work between the swap and its own
    /// `opened`, which is passed what was taken: a `start` that fails
    /// drops it with the local it became, rather than leaving it here
    /// for a later, unrelated `start` to find. Not `left`'s to reset:
    /// it must outlive the `leave` and `start` the swap runs between,
    /// which is what lets `leave` read its own `Land` (see that type).
    arrive: Option<(Rd, Pos, Land)>,
}

/// The input of `key` in `origin`, taking `from`'s clock and source,
/// on its way in and let go by nothing yet.
///
/// Every key no person typed carries the press of the key that made
/// it — the `@a`, the `.`, or the prefix that let it go — so the host
/// invents no keyboard and a recorded stream replays as it was.
#[must_use]
pub fn keyed(key: Key, origin: u64, from: &Press) -> Input {
    let press = Press { key: key.code(), when: from.when, source: from.source };
    Input { press, origin, released: false }
}

/// One key of `origin`, for the tests inside this crate: a press of
/// no clock and no source, which the engine keeps and never reads.
#[cfg(test)]
pub(crate) fn tapped(key: Key, origin: u64) -> Input {
    keyed(key, origin, &Press { key: 0, when: 0, source: NONE })
}

/// The key a press names, or none for a code no `Key` stands for:
/// the engine's one reading of a code, at its entry and of the keys
/// it kept from one.
fn key_of(press: &Press) -> Option<Key> {
    Key::of(press.key)
}

impl Vim {
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    #[must_use]
    pub const fn mode(&self) -> Mode {
        self.mode
    }

    /// The unnamed register, `None` while nothing has been yanked
    /// or deleted into it.
    #[must_use]
    pub const fn register(&self) -> Option<&Register> {
        self.register.as_ref()
    }

    /// What showcmd shows, which `/self/vim/pending` reads: the
    /// register, the operator's count where it is more than one, the
    /// operator's key, the count typed after it, and the key waiting
    /// for its second. Empty once the command has fired.
    #[must_use]
    pub fn showcmd(&self) -> String {
        let Pending { count, key, operator, register, .. } = &self.pending;
        let named = register.map(|name| alloc::format!("\"{name}"));
        let twice = operator.filter(|(_, n)| *n > 1);
        let after = (*count > 0).then(|| alloc::format!("{count}"));
        let mut out = named.unwrap_or_default();
        out.extend(twice.map(|(_, n)| alloc::format!("{n}")));
        out.extend(operator.map(|(op, _)| op.key()));
        out.extend(after);
        out.extend(*key);
        out
    }

    /// What register `name` holds, `"` being the unnamed one, with the
    /// newline a linewise one ends in, which is what the clipboard is
    /// handed: the kind is in the bytes as Vim treats it. Empty for a
    /// register nothing has been put in.
    #[must_use]
    pub fn stored(&self, name: char) -> String {
        let held = if name == '"' {
            self.register.as_ref()
        } else {
            self.registers.get(&name)
        };
        let Some(held) = held else { return String::new() };
        let end = if held.kind == Kind::Line { "\n" } else { "" };
        alloc::format!("{}{end}", held.text)
    }

    /// Text a `"+` or `"*` yank left for the clipboard, once.
    pub const fn clipped(&mut self) -> Option<String> {
        self.clipboard.take()
    }

    /// After an undo or redo, as nvim places it: on the first changed
    /// line (`top`), at the column the cursor had before that change
    /// when this was its line, one line up for a change made by `o`,
    /// else at the wanted column.
    ///
    /// An undo also extends the current marks with the `a`-`z` ones
    /// that change's `Change` kept, wiping nothing set since (nvim's
    /// `uh_namedm`); a redo is `marked` alone, its own forward shift
    /// already applied. `[`, `]` and `.` are set as nvim sets them
    /// for either: `[` and `]` to `top` and `bottom`, `.` to `top` on
    /// an undo and to `bottom` on a redo.
    fn undone_cursor(&mut self, top: usize, bottom: usize, redo: bool) {
        let made = self.undone.before.len() - self.undone.count;
        let before = if redo {
            self.undone.count = self.undone.count.saturating_sub(1);
            self.undone.before.get(made)
        } else {
            self.undone.count = (self.undone.count + 1).min(made.max(1));
            made.checked_sub(1).and_then(|i| self.undone.before.get(i))
        };
        if !redo && let Some(change) = before {
            self.marks.local.extend(change.marks.iter().copied());
        }
        let line = match before {
            Some(change) if change.cursor.0 + 1 == top && top > 0 => {
                change.cursor.0
            }
            _ => top,
        };
        let start = self.line_at(line).unwrap_or_else(|| {
            self.line_at(self.line_count() - 1).unwrap_or(0)
        });
        self.cursor = match before {
            Some(change) if change.cursor.0 == line => {
                self.at_column(start, change.cursor.1)
            }
            _ => self.at_screen_column(start, self.want),
        };
        self.want = self.wanted();
        self.marks.local.insert('[', Pos { line: top, column: 0 });
        self.marks.local.insert(']', Pos { line: bottom, column: 0 });
        let dot = if redo { bottom } else { top };
        self.marks.local.insert('.', Pos { line: dot, column: 0 });
    }

    /// `[`, from the lowest edit start of `edits`, and `]`, from the
    /// highest edit end, each carried through the edits after it into
    /// final coordinates (`carried`); `None` for no edits at all,
    /// `.min()`'s and `.max()`'s own `?` on an empty `edits`.
    ///
    /// `typed` (insert mode's own edits, `changing`'s caller says)
    /// leaves `]` where the typing itself left the cursor, one past
    /// the last byte put in, nvim's own rule for `Insstart`; untyped
    /// (a put among them, `len == 0` on every edit just as typing is)
    /// leaves `]` on the last byte actually put in, the general case
    /// `last_char_before` reads, as does one that took bytes too
    /// (`:s`, `J`, a delete).
    fn batch_span(&self, edits: &[Edit], typed: bool) -> Option<(Pos, Pos)> {
        let open = (0..edits.len())
            .map(|i| carried(edits[i].at, &edits[i + 1..]))
            .min()?;
        let close_end = (0..edits.len())
            .map(|i| {
                carried(edits[i].at + edits[i].text.len(), &edits[i + 1..])
            })
            .max()?;
        let close = if typed {
            close_end
        } else {
            self.last_char_before(close_end, open)
        };
        Some((self.pos_at(open), self.pos_at(close)))
    }

    /// A change is being made: the cursor kept for undo, with the
    /// `a`-`z` marks as they stood; what stood undone is forgotten,
    /// as a new change ends redo. Every edit the effect made moves
    /// the marks in order, [`Self::marked`] being the one door, a
    /// repeat's keys included: they come through the queue once, and
    /// there is no second pass over them.
    /// `typed` is whether the mode this key (or `pasted`, `did`) found
    /// Vim in was Insert, before the key itself may have left it: a
    /// block insert's Esc is still typed on its way out; a `c` that
    /// enters Insert as part of the same key is not, on its way in.
    fn changing(
        &mut self,
        before: (usize, usize),
        effect: &mut Effect,
        typed: bool,
    ) {
        if matches!(effect, Effect::Undo | Effect::Redo) {
            self.macros.joined = false;
        }
        match effect {
            Effect::Undo => return self.undone.walking = Some(false),
            Effect::Redo => return self.undone.walking = Some(true),
            _ => {}
        }
        let own = effect_edits(effect).first().is_some_and(|e| !e.join);
        self.continued(effect);
        let edits = effect_edits(effect);
        let starting = edits.first().is_some_and(|e| !e.join);
        // A repeat's keys come through the queue, so there is no
        // outer pass to take the snapshot: its own first edit takes
        // it, exactly as a typed change's first edit does.
        let first =
            !edits.is_empty() && core::mem::take(&mut self.repeat.first);
        if starting && (!self.repeat.replaying || first) {
            self.snapshot(before);
        }
        for edit in edits {
            self.marked(edit);
        }
        self.mark_batch(edits, own, typed);
    }

    /// The undo snapshot a revision's own first edit takes: the
    /// cursor before it, with the `a`-`z` marks as they stood.
    fn snapshot(&mut self, before: (usize, usize)) {
        let made = self.undone.before.len() - self.undone.count;
        self.undone.before.truncate(made);
        let marks = self.marks.local.range('a'..='z').map(|(&c, &p)| (c, p));
        self.undone
            .before
            .push(Change { cursor: before, marks: marks.collect() });
        self.undone.count = 0;
    }

    /// `[`, `]` and `.` after `edits`: `batch_span` reads the text
    /// now held, which is every edit of this effect already applied;
    /// `carried` maps each edit's own start and end through the edits
    /// after it into that final state. `[` and `.` are the
    /// revision's own start, set once and never moved by an edit that
    /// joins it (a session's second keystroke and on); `]` is the
    /// revision's own end so far, and every edit of it, joining or
    /// not, reaches that far.
    fn mark_batch(&mut self, edits: &[Edit], starting: bool, typed: bool) {
        let Some((open, close)) = self.batch_span(edits, typed) else { return };
        if starting {
            self.marks.local.insert('[', open);
            self.marks.local.insert('.', open);
        }
        self.marks.local.insert(']', close);
    }

    /// The one door every edit reaches positions through: the local
    /// marks, and of this source the file marks, moved past `edit`.
    /// `loaded`'s own synthetic edit goes through it too, before
    /// `undone_cursor`, and stops there: a reload is not a change,
    /// and sets no `[`, `]` or `.`.
    fn marked(&mut self, edit: &Edit) {
        // The one door positions move through is the one door the
        // list is dropped at: it is never patched, and the fill
        // begins again for whoever asks next.
        self.found.epoch += 1;
        self.found.key = None;
        self.found.after.clear();
        self.found.before.clear();
        self.found.skipped.clear();
        self.found.filled = Filled::More;
        self.found.shown = None;
        // An edit from anywhere ends a wait, and here alone: the list
        // the motion waited on is gone, so there is nothing left to
        // run it again on. Unconditional, this would put `pending`
        // back at every edit, and a record left would hold `d/b<CR>`
        // before the next change's own keys.
        if self.found.waiting.take().is_some() {
            self.pending = Pending::default();
            self.repeat.record.clear();
        }
        let source = self.source;
        self.marks.moved(
            edit,
            &mut self.files,
            &mut self.jumps.list,
            source,
            &mut self.global.lines,
        );
    }

    /// The last real char before byte `end` of the text now held, a
    /// trailing `\n` skipped first, `prev_char` reading the char
    /// boundaries of the text so a wide char never splits, never
    /// before `floor`.
    pub(crate) fn last_char_before(&self, end: usize, floor: usize) -> usize {
        let before_nl = end - usize::from(self.text[..end].ends_with('\n'));
        if before_nl <= floor { floor } else { self.prev_char(before_nl) }
    }

    /// `at` as a [`Pos`]: the line it is on, and its column, in
    /// bytes from that line's own start, as nvim's `col` reads.
    pub(crate) fn pos_at(&self, at: usize) -> Pos {
        let line = self.text[..at].matches('\n').count();
        Pos { line, column: at - self.line_start(at) }
    }

    /// A [`Pos`] as a byte of the text now held, its line clamped to
    /// one the text still has, its column floored to a char boundary
    /// the text still has one at or before. A column of `usize::MAX`
    /// is nvim's MAXCOL, a line's end however long it has grown.
    pub(crate) fn byte_at(&self, pos: Pos) -> usize {
        let line = pos.line.min(self.line_count() - 1);
        let start = self.line_at(line).unwrap_or(0);
        let mut at = start.saturating_add(pos.column).min(self.line_end(start));
        while !self.text.is_char_boundary(at) {
            at -= 1;
        }
        at
    }

    /// The cursor's line and column, in chars from 0.
    #[must_use]
    pub fn position(&self) -> (usize, usize) {
        self.position_of(self.cursor)
    }

    /// A digit onto the count: `0` only after another, being a motion
    /// else.
    const fn digit(&mut self, key: Key) -> bool {
        if self.pending.key.is_some() {
            return false;
        }
        let Key::Char(digit @ '0'..='9') = key else { return false };
        if digit == '0' && self.pending.count == 0 {
            return false;
        }
        let digit = digit as usize - '0' as usize;
        self.pending.count =
            self.pending.count.saturating_mul(10).saturating_add(digit);
        self.keep_want = true;
        true
    }

    fn normal(&mut self, key: Key) -> Effect {
        if self.digit(key) {
            return Effect::Nothing;
        }
        let count = core::mem::take(&mut self.pending.count);
        let n = count.max(1);
        if let Some(first) = self.pending.key.take() {
            return self.second(first, key, count);
        }
        if let Some((op, first)) = self.pending.operator
            && key == Key::Char(op.key())
        {
            self.pending.operator = None;
            return self.lines(op, first.saturating_mul(n));
        }
        if let Some(motion) = Motion::of(key, count) {
            return self.motion(motion, n);
        }
        let effect = match key {
            Key::Char(
                first @ ('g' | 'f' | 't' | 'F' | 'T' | '"' | '\'' | '`' | 'm'),
            ) => {
                return self.wait(first, count);
            }
            Key::Char(first @ ('i' | 'a'))
                if self.pending.operator.is_some() =>
            {
                return self.wait(first, count);
            }
            Key::Char('Z') if self.pending.operator.is_none() => {
                return self.wait('Z', count);
            }
            Key::Char('d') => return self.operator(Operator::Delete, n),
            Key::Char('c') => return self.operator(Operator::Change, n),
            Key::Char('y') => return self.operator(Operator::Yank, n),
            Key::Char('>') => return self.operator(Operator::ShiftRight, n),
            Key::Char('<') => return self.operator(Operator::ShiftLeft, n),
            Key::Char(c @ ('/' | '?')) => return self.prompt(c, count),
            _ if self.pending.operator.take().is_some() => Effect::Nothing,
            Key::Char('q') if self.macros.fed => Effect::Nothing,
            Key::Ctrl('^' | '6') => Effect::Switch(Switch::Alternate),
            Key::Char('q') if self.macros.recording.is_some() => {
                return self.recorded();
            }
            Key::Char(first @ ('q' | '@')) => return self.wait(first, count),
            Key::Ctrl('o') => return self.jumped(true, n),
            Key::Tab => return self.jumped(false, n),
            Key::Char('~') => self.tilde(n),
            Key::Char('v') => self.enter_visual(Visual::Char),
            Key::Char('V') => self.enter_visual(Visual::Line),
            Key::Ctrl('v') => self.enter_visual(Visual::Block),
            Key::Char('i') => self.enter_insert(),
            Key::Char('a') => {
                self.cursor = self.append_at();
                self.enter_insert()
            }
            Key::Char('I') => {
                self.cursor = self.first_non_blank(self.cursor);
                self.enter_insert()
            }
            Key::Char('A') => {
                self.cursor = self.line_end(self.cursor);
                self.enter_insert()
            }
            Key::Char('o') => {
                let indent = self.indent_of(self.cursor);
                self.cursor = self.line_end(self.cursor);
                self.enter_insert();
                self.open_line(&indent)
            }
            Key::Char('O') => {
                let indent = self.indent_of(self.cursor);
                self.cursor = self.line_start(self.cursor);
                self.enter_insert();
                let effect = self.put_str(&alloc::format!("{indent}\n"));
                self.cursor -= 1;
                self.indent = indent.len();
                effect
            }
            Key::Char('x') => self.operate(Operator::Delete, n, Motion::Right),
            Key::Char('X') => self.operate(Operator::Delete, n, Motion::Left),
            Key::Char('D') => {
                self.operate(Operator::Delete, n, Motion::LineEnd)
            }
            Key::Char('C') => {
                self.operate(Operator::Change, n, Motion::LineEnd)
            }
            // nvim maps `Y` to `y$`.
            Key::Char('Y') => self.operate(Operator::Yank, n, Motion::LineEnd),
            Key::Char('p') => self.paste(true, n),
            Key::Char('P') => self.paste(false, n),
            Key::Char('J') => self.join(n),
            Key::Char('.') => return self.replay(count),
            Key::Char('u') => Effect::Undo,
            Key::Ctrl('r') => Effect::Redo,
            Key::Char(':') => self.prompt(':', count),
            _ => Effect::Nothing,
        };
        self.settle(&effect);
        effect
    }

    /// The second key of a pair: `gg`, `dd`, `ZZ`, `fx` and the like.
    fn second(&mut self, first: char, key: Key, count: usize) -> Effect {
        let n = count.max(1);
        if let Some(motion) = self.pair(first, key, count) {
            return self.motion(motion, n);
        }
        match (first, key) {
            ('g', Key::Char('u')) => self.operator(Operator::Lower, n),
            ('g', Key::Char('U')) => self.operator(Operator::Upper, n),
            ('g', Key::Char('~')) => self.operator(Operator::Toggle, n),
            ('g', Key::Char('v')) => self.reselect(),
            ('g', Key::Char('F')) => self.file_under_cursor(),
            ('"', Key::Char(name)) => self.named(name, count),
            ('m', Key::Char(name @ ('a'..='z' | 'A'..='Z'))) => {
                self.repeat.record.clear();
                self.set_mark(name)
            }
            ('q', Key::Char(name @ ('a'..='z' | 'A'..='Z'))) => {
                self.recording(name)
            }
            ('@', Key::Char(name @ ('a'..='z' | 'A'..='Z' | '@' | ':'))) => {
                self.macro_run(name, n)
            }
            ('Z', Key::Char('Z')) => Effect::Quit(Quit::Write),
            ('Z', Key::Char('Q')) => Effect::Quit(Quit::Force),
            _ => {
                self.pending.operator = None;
                self.repeat.record.clear();
                Effect::Nothing
            }
        }
    }

    /// The motion a pair names, in normal and visual mode alike:
    /// `gg`, an object after `i` or `a`, or a find, which `;` and `,`
    /// then repeat.
    pub(crate) const fn pair(
        &mut self,
        first: char,
        key: Key,
        count: usize,
    ) -> Option<Motion> {
        let Key::Char(c) = key else { return None };
        let motion = match first {
            'g' if c == 'e' => Motion::WordEndBack { big: false },
            'g' if c == 'E' => Motion::WordEndBack { big: true },
            'g' if c == 'j' => Motion::ScreenDown,
            'g' if c == 'k' => Motion::ScreenUp,
            'g' if c == 'g' => Motion::Line(count.saturating_sub(1)),
            'i' | 'a' => Motion::Object { inner: first == 'i', kind: c },
            'f' => Motion::Find(Find::Forward, c),
            't' => Motion::Find(Find::ForwardTo, c),
            'F' => Motion::Find(Find::Backward, c),
            'T' => Motion::Find(Find::BackwardTo, c),
            '\'' => Motion::Mark { name: c, exact: false },
            '`' => Motion::Mark { name: c, exact: true },
            _ => return None,
        };
        if let Motion::Find(kind, c) = motion {
            self.find = Some((kind, c));
        }
        Some(motion)
    }

    /// `"x`: the register `name` for what comes next; the count waits.
    pub(crate) const fn named(&mut self, name: char, count: usize) -> Effect {
        self.pending.register = Some(name);
        self.pending.count = count;
        self.keep_want = true;
        Effect::Nothing
    }

    /// The clipboard's `text` put: lines of its own when it ends in a
    /// newline, as nvim reads it.
    fn put_text(&mut self, text: &str, put: Put) -> Effect {
        let kind = if text.ends_with('\n') { Kind::Line } else { Kind::Char };
        let text = String::from(text.strip_suffix('\n').unwrap_or(text));
        let register = Register { text, kind };
        self.last_clip = register.clone();
        match put.over {
            Some(kind) => self.paste_over(kind, register),
            None => self.paste_register(register, put.after, put.count),
        }
    }

    /// `first` waits for its second key; the count waits with it.
    const fn wait(&mut self, first: char, count: usize) -> Effect {
        self.pending.key = Some(first);
        self.pending.count = count;
        self.keep_want = true;
        Effect::Nothing
    }

    /// An operator key: the operator waits for a motion, or doubled
    /// (`dd`, `cc`, `yy`, `gUgU`) takes `n` lines.
    fn operator(&mut self, op: Operator, n: usize) -> Effect {
        match self.pending.operator.take() {
            Some((waiting, count)) if waiting == op => {
                self.lines(op, count.saturating_mul(n))
            }
            Some(_) => {
                self.repeat.record.clear();
                Effect::Nothing
            }
            None => {
                self.pending.operator = Some((op, n));
                self.keep_want = true;
                Effect::Nothing
            }
        }
    }

    /// `op` over `n` whole lines from the cursor's.
    fn lines(&mut self, op: Operator, n: usize) -> Effect {
        let effect = self.operate(op, n, Motion::Lines);
        self.settle(&effect);
        effect
    }

    /// `motion` `n` times for the cursor, or for the operator waiting.
    ///
    /// A search the list cannot answer leaves from here, before
    /// anything is taken: `pending` stands through the wait, so the
    /// operator is still the operator when the motion runs again.
    fn motion(&mut self, motion: Motion, n: usize) -> Effect {
        if let Some(effect) = self.wanting(motion, n) {
            return effect;
        }
        let effect = match self.pending.operator.take() {
            Some((op, count)) => {
                self.operate(op, count.saturating_mul(n), motion)
            }
            None => self.move_by(motion, n),
        };
        self.settle(&effect);
        effect
    }

    /// The command is over: a change is what `.` repeats, anything
    /// else is forgotten. An insert goes on recording until Esc.
    fn settle(&mut self, effect: &Effect) {
        if self.repeat.replaying
            || matches!(self.mode, Mode::Insert | Mode::Visual(_))
        {
            return;
        }
        match effect {
            Effect::Edit(_) | Effect::Edits(_) => {
                self.repeat.last = core::mem::take(&mut self.repeat.record);
            }
            Effect::Clipboard => {}
            _ => self.repeat.record.clear(),
        }
    }

    /// `.`: the last change's keys for the queue to run, its count
    /// replaced by `count` when there is one.
    fn replay(&mut self, count: usize) -> Effect {
        self.repeat.record.clear();
        let mut keys: Vec<Key> = self.repeat.last.clone();
        if keys.is_empty() {
            return Effect::Nothing;
        }
        if count > 0 {
            let digits = keys.iter().take_while(|k| is_digit(**k)).count();
            keys.drain(..digits);
            let count = alloc::format!("{count}");
            keys.splice(0..0, count.chars().map(Key::Char));
        }
        self.repeat.first = true;
        Effect::Fed(keys, true)
    }

    fn insert(&mut self, key: Key) -> Effect {
        match key {
            Key::Char(c) => self.put(c),
            Key::Enter => {
                let indent = self.indent_of(self.cursor);
                self.open_line(&indent)
            }
            Key::Tab => self.put('\t'),
            Key::Backspace => self.backspace(),
            Key::Left => self.arrow(Self::left),
            Key::Right => self.arrow(Self::right),
            Key::Up => self.arrow(|v| v.vertical(false)),
            Key::Down => self.arrow(|v| v.vertical(true)),
            Key::Home => self.arrow(|v| v.go(v.line_start(v.cursor))),
            Key::End => self.arrow(|v| v.go(v.line_end(v.cursor))),
            Key::Esc => self.leave_insert(),
            Key::Ctrl(_) | Key::Alt(_) => Effect::Nothing,
        }
    }

    /// Esc: back to normal mode, the insert being the last change,
    /// a pending indent dropped, the cursor stepped back.
    fn leave_insert(&mut self) -> Effect {
        self.mode = Mode::Normal;
        if !self.repeat.replaying {
            self.repeat.last = core::mem::take(&mut self.repeat.record);
        }
        if let Some(block) = self.block.take() {
            let effect = self.finish_block(block);
            self.clamp();
            return effect;
        }
        let dropped = self.drop_indent();
        self.cursor = self.stepped_back();
        dropped.unwrap_or(Effect::Moved)
    }

    /// A new line after the cursor with `indent` copied onto it, as
    /// `autoindent` does; an indent still pending on the old line goes
    /// with the same edit, so that line is left empty.
    fn open_line(&mut self, indent: &str) -> Effect {
        let pending = core::mem::take(&mut self.indent);
        let at = self.cursor - pending;
        let text = alloc::format!("\n{indent}");
        self.cursor = at + text.len();
        self.indent = indent.len();
        self.edit(at, pending, text)
    }

    /// The indent nothing was typed after, removed; the cursor at
    /// the line's start.
    fn drop_indent(&mut self) -> Option<Effect> {
        let pending = core::mem::take(&mut self.indent);
        if pending == 0 {
            return None;
        }
        let at = self.cursor - pending;
        self.cursor = at;
        Some(self.edit(at, pending, String::new()))
    }

    /// Insert mode at the cursor; the first edit starts a revision.
    const fn enter_insert(&mut self) -> Effect {
        self.mode = Mode::Insert;
        self.joining = false;
        Effect::Moved
    }

    /// A move inside an insert ends its undo step, as in Vim, and
    /// keeps whatever indent was copied.
    fn arrow(&mut self, motion: impl FnOnce(&mut Self) -> Effect) -> Effect {
        self.joining = false;
        self.indent = 0;
        motion(self)
    }

    /// `c` at the cursor.
    fn put(&mut self, c: char) -> Effect {
        self.indent = 0;
        self.put_str(&String::from(c))
    }

    /// `text` at the cursor.
    fn put_str(&mut self, text: &str) -> Effect {
        let at = self.cursor;
        self.cursor += text.len();
        self.edit(at, 0, String::from(text))
    }

    fn backspace(&mut self) -> Effect {
        self.indent = 0;
        let Some(c) = self.text[..self.cursor].chars().next_back() else {
            return Effect::Nothing;
        };
        self.cursor -= c.len_utf8();
        self.edit(self.cursor, c.len_utf8(), String::new())
    }

    /// `len` bytes at `at` become `text`, joined to the revision
    /// under way when an insert is.
    fn edit(&mut self, at: usize, len: usize, mut text: String) -> Effect {
        self.owed_eol(at, len, &mut text);
        self.text.replace_range(at..at + len, &text);
        let join = core::mem::replace(&mut self.joining, true);
        Effect::Edit(Edit { at, len, text, join })
    }

    /// `text`, about to replace `at..at + len`, given the `\n` it
    /// owes: a final `\n` ends a line, so an edit that leaves a text
    /// which had none ending in one — `o` at that end, a `\r` of
    /// `:s` there, the last line's chars taken — has put the cursor
    /// on a line the text does not hold, and one `\n` more is that
    /// line, as nvim holds `["11", ""]`. The splices that can reach
    /// the end (`edit`, `:s`, a block's) pass here first.
    pub(crate) fn owed_eol(&self, at: usize, len: usize, text: &mut String) {
        let last = at + len == self.text.len() && !self.text.ends_with('\n');
        let tail =
            if text.is_empty() { &self.text[..at] } else { text.as_str() };
        let owed = last && tail.ends_with('\n');
        if owed {
            text.push('\n');
        }
    }

    /// `len` bytes at `at` become `text`, as a revision of their own.
    fn replace(&mut self, at: usize, len: usize, text: String) -> Effect {
        self.joining = false;
        self.edit(at, len, text)
    }

    /// `len` bytes at `at` gone as a revision of their own. Nothing
    /// when there is nothing to remove.
    fn remove(&mut self, at: usize, len: usize) -> Effect {
        if len == 0 {
            return Effect::Nothing;
        }
        self.replace(at, len, String::new())
    }

    /// `motion` `n` times, or until one goes nowhere; `Moved` if any
    /// of them moved.
    fn repeat(
        &mut self,
        n: usize,
        mut motion: impl FnMut(&mut Self) -> Effect,
    ) -> Effect {
        let count =
            (0..n).take_while(|_| motion(self) == Effect::Moved).count();
        if n > 0 && count == 0 {
            self.ring();
        }
        moved(count > 0)
    }
}

/// The engine's own calls: what a key does, and what the Plugin at
/// the bottom of this file needs of it.
impl Vim {
    /// `bytes` as the text, lossily; the cursor stays where it can.
    /// True when the text differs from before.
    ///
    /// The difference from the text held before is one synthetic
    /// edit over the common prefix and suffix, which moves every
    /// position through `marked` before the cursor is placed: an
    /// undo, a redo, and the reload a stale flush or `:e`/`:e!` makes
    /// are each one of these, never a change of their own. The
    /// prefix and suffix are found over chars, never a byte that
    /// splits one, so the edit's own `at` is always a boundary.
    pub fn loaded(&mut self, bytes: &[u8]) -> bool {
        let text = String::from_utf8_lossy(bytes);
        self.lossy = matches!(text, alloc::borrow::Cow::Owned(_));
        let walking = self.undone.walking.take();
        if text == self.text {
            return false;
        }
        let synthetic = minimal_edit(0, &self.text, &text, false);
        let top = self.text[..synthetic.at].matches('\n').count();
        // Not `synthetic.text.len()` whole: a trailing `\n` of the
        // inserted text is the *next* line's own start, not this
        // one's, and would count one line too many.
        let put = synthetic.text.strip_suffix('\n').unwrap_or(&synthetic.text);
        let bottom = text[..synthetic.at + put.len()].matches('\n').count();
        self.text = text.into_owned();
        self.marked(&synthetic);
        match walking {
            Some(redo) => self.undone_cursor(top, bottom, redo),
            None => self.cursor = self.cursor.min(self.bound()),
        }
        self.resettled();
        true
    }

    /// The cursor onto a char boundary, then Normal mode's clamp: what
    /// `loaded` leaves the cursor at, and what a `Place` restored by
    /// `opened` is put through too.
    pub(crate) fn resettled(&mut self) {
        while !self.text.is_char_boundary(self.cursor) {
            self.cursor -= 1;
        }
        if self.mode == Mode::Normal {
            self.clamp();
        }
    }

    /// One input in the current mode: the key its press names, run as
    /// its origin says. A message showing or shown goes with it, so
    /// the View is shown again.
    ///
    /// This is the engine's one entry. A key a person typed comes in
    /// as `TYPED`, a macro's as `FED` and a `.`'s as `REPEAT`;
    /// `released` says a failed prefix let this key go, which is the
    /// one thing that keeps it from being looked up in the keymap
    /// again. Whose it is it keeps through that release — a macro's
    /// key is still the macro's, so its edits join that run's one
    /// revision and a bell still drops it. A code no `Key` stands for
    /// is nothing here, and the queue moves on.
    ///
    /// ```
    /// use reovim_client_api::{Key, NONE, keyboard::Press, origin};
    /// use reovim_plugin_vim::{Vim, keyed};
    /// let mut vim = Vim::default();
    /// vim.loaded(b"abc\n");
    /// let none = Press { key: 0, when: 0, source: NONE };
    /// vim.key(keyed(Key::Char('x'), origin::FED, &none));
    /// assert_eq!(vim.text(), "bc\n");
    /// ```
    pub fn key(&mut self, input: Input) -> Effect {
        let Some(key) = key_of(&input.press) else { return Effect::Nothing };
        let past = input.released;
        // A `.`'s keys replay, a key a prefix let go among them: the
        // release says this key is not looked up again and nothing
        // about whose it is, so it records no new last change.
        self.repeat.replaying = input.origin == origin::REPEAT;
        // A release is this key's second arrival: it was recorded,
        // and it ended a join, the first time.
        let typed = !past && input.origin == origin::TYPED;
        if typed {
            self.macros.joined = false;
        }
        if typed && self.macros.recording.is_some() {
            self.macros.record.push(key);
        }
        self.macros.fed = matches!(input.origin, origin::FED | origin::REPEAT);
        self.taken = Some(input);
        self.processed(key, past)
    }

    /// The input the engine last took: what the host reads for the
    /// press to give the keys that input asks it to feed.
    #[must_use]
    pub const fn taken(&self) -> Option<&Input> {
        self.taken.as_ref()
    }

    /// The keys a failed prefix let go, in the order they were taken,
    /// for the host to put at the front of its queue; empty once
    /// there are none.
    ///
    /// The whole batch at once, and the engine keeps no copy: each
    /// input carries the origin it was taken with and `released`, so
    /// the queue alone says whose a released key is and a bell can
    /// drop a macro's while it keeps a person's.
    ///
    /// ```
    /// use reovim_client_api::{Key, NONE, keyboard::Press, origin};
    /// use reovim_plugin_vim::{Effect, Vim, keyed};
    /// let mut vim = Vim::default();
    /// vim.loaded(b"abc\n");
    /// vim.bind("xy", "write");
    /// let none = Press { key: 0, when: 0, source: NONE };
    /// let typed = |key| keyed(key, origin::TYPED, &none);
    /// assert_eq!(vim.key(typed(Key::Char('x'))), Effect::Nothing);
    /// vim.key(typed(Key::Char('p')));
    /// for input in vim.released() {
    ///     assert_eq!(input.origin, origin::TYPED, "the person's still");
    ///     vim.key(input);
    /// }
    /// assert_eq!(vim.text(), "bac\n");
    /// ```
    pub fn released(&mut self) -> Vec<Input> {
        let mut inputs = core::mem::take(&mut self.held.released);
        for input in &mut inputs {
            input.released = true;
        }
        inputs
    }

    /// A bell or the replay budget: what a macro fed goes and what a
    /// person typed stays. So a macro that ends inside a prefix
    /// cannot be finished by the next typed key.
    ///
    /// Only what the engine still holds: what it has let go is the
    /// host's queue's, which sweeps it by the same origin.
    pub(crate) fn cancel(&mut self) {
        self.held.keys.retain(|input| input.origin == origin::TYPED);
    }

    /// The keymap at `key`, before anything else this key would do:
    /// `Some` where the map took it, `None` where the key is itself.
    ///
    /// `past` says the key is one a failed prefix let go, which is not
    /// looked up again — but for an Alt the map knows alone, which
    /// nvim runs wherever it arrives.
    ///
    /// Named for the key and not `bound` alone: `bound()` is already
    /// the last byte the cursor may take (motion.rs).
    fn bound_key(&mut self, key: Key, past: bool) -> Option<Effect> {
        if self.repeat.replaying
            || !matches!(self.mode, Mode::Normal | Mode::Visual(_))
            || self.pending.key.is_some()
            || self.pending.operator.is_some()
        {
            return None;
        }
        let alone = self.held.keys.is_empty()
            && matches!(key, Key::Alt(_))
            && self.keymap.fact(&[key]) != Fact::None;
        let counted = self.pending.count > 0 || self.pending.register.is_some();
        if !alone && (past || counted) {
            return None;
        }
        if counted {
            // nvim runs a mapped Alt behind a count and behind a
            // register alike, and neither reaches the line.
            self.pending = Pending::default();
        }
        self.hold()
    }

    /// The input now running held with the keys before it, and what
    /// the map makes of them all.
    fn hold(&mut self) -> Option<Effect> {
        self.held.keys.extend(self.taken.clone());
        let keys: Vec<Key> = self
            .held
            .keys
            .iter()
            .filter_map(|input| key_of(&input.press))
            .collect();
        match self.keymap.fact(&keys) {
            Fact::Exact(line) => {
                self.held.keys.clear();
                Some(self.binding(&line))
            }
            // No timeout: the next key settles it, a macro's too, and
            // nothing settles where the fed keys run out.
            Fact::Longer(_) | Fact::Prefix => Some(Effect::Nothing),
            Fact::None if keys.len() == 1 => {
                self.held.keys.clear();
                None
            }
            Fact::None => Some(self.failed(&keys)),
        }
    }

    /// Keys that began no binding after all: the longest head that is
    /// a binding runs, and everything after it is let go as it was
    /// taken.
    fn failed(&mut self, keys: &[Key]) -> Effect {
        let bound = |end: usize| match self.keymap.fact(&keys[..end]) {
            Fact::Exact(line) | Fact::Longer(line) => Some((end, line)),
            Fact::Prefix | Fact::None => None,
        };
        let (head, line) = (1..keys.len())
            .filter_map(bound)
            .next_back()
            .map_or((0, None), |(end, line)| (end, Some(line)));
        let rest = self.held.keys.split_off(head);
        self.held.keys.clear();
        self.held.released.extend(rest);
        line.map_or(Effect::Nothing, |line| self.binding(&line))
    }

    /// A binding's line, through the `:` door and settled as a typed
    /// line is. In visual it leaves visual first, as a typed `:` does,
    /// so the line finds no live selection, and then it runs over
    /// `'<,'>` as nvim's `:` from a selection does, typed or mapped.
    fn binding(&mut self, line: &str) -> Effect {
        let selected = matches!(self.mode, Mode::Visual(_));
        if selected {
            self.leave_visual();
        }
        // `run`'s own settle is no part of this: the lookup answers
        // only where nothing is pending, so `repeat.record` is empty
        // wherever a binding runs and there is nothing to settle.
        let effect = self.ex(line);
        let Effect::Line(mut ex) = effect else { return effect };
        if selected && !ex.ranged {
            (ex.ranged, ex.from, ex.to) = self.selected();
        }
        Effect::Line(ex)
    }

    /// The range `'<,'>` names now: the lines of the marks `<` and
    /// `>`, which `leave_visual` set. A line of its own address keeps
    /// it, as `:'<,'>%s` does in nvim, so this is asked for only where
    /// there is none.
    fn selected(&self) -> (bool, u64, u64) {
        let ends = self.marks.local.get(&'<').zip(self.marks.local.get(&'>'));
        ends.map_or((false, 0, 0), |(lo, hi)| {
            (true, lo.line as u64, hi.line as u64)
        })
    }

    /// Where the cursor or the selection is, as a compiler, a grep or
    /// this editor reads it: `PATH:LOCATOR`.
    ///
    /// `path` is the file the Buffer is over, absolute or from where
    /// the editor stands, which is the Plugin's to resolve; a Buffer
    /// over nothing has none. `range` is the `:` line's, and says the
    /// locator is a selection's.
    ///
    /// ```
    /// let mut vim = reovim_plugin_vim::Vim::default();
    /// vim.loaded(b"one\ntwo\n");
    /// let point = vim.point(Some("/a"), false, None);
    /// assert_eq!(point, Ok(String::from("/a:1")));
    /// assert_eq!(vim.point(None, false, None), Err("E32: No file name"));
    /// ```
    ///
    /// # Errors
    /// `E32: No file name` for a Buffer over nothing.
    pub fn point(
        &self,
        path: Option<&str>,
        col: bool,
        range: Option<(usize, usize)>,
    ) -> Result<String, &'static str> {
        let path = path.ok_or(E32)?;
        Ok(alloc::format!("{path}:{}", self.locator(col, range)))
    }

    /// The locator of the text view: `LINE`, `LINE:COL` under `col`,
    /// `FROM-TO` for a linewise selection and `LINE:COL-LINE:COL` for
    /// a charwise or block one, the corners in text order. Lines and
    /// columns count from 1 and a column is bytes, as nvim's `line()`
    /// and `col()` say them.
    fn locator(&self, col: bool, range: Option<(usize, usize)>) -> String {
        let Some((from, to)) = range else { return self.here(col) };
        let ends = self.marks.local.get(&'<').zip(self.marks.local.get(&'>'));
        let kind = self.last_visual.map(|last| last.kind);
        match (kind, ends) {
            (Some(Visual::Char | Visual::Block), Some((lo, hi))) => {
                alloc::format!(
                    "{}:{}-{}:{}",
                    lo.line + 1,
                    lo.column + 1,
                    hi.line + 1,
                    hi.column + 1
                )
            }
            _ => alloc::format!("{}-{}", from + 1, to + 1),
        }
    }

    /// Where the cursor is: its line, and its column beside it under
    /// `col`.
    fn here(&self, col: bool) -> String {
        let at = self.pos_at(self.cursor);
        if !col {
            return alloc::format!("{}", at.line + 1);
        }
        alloc::format!("{}:{}", at.line + 1, at.column + 1)
    }

    /// The cursor where `locator` names, in the grammar [`Self::point`]
    /// writes: `LINE`, `LINE:COL`, `FROM-TO` and `LINE:COL-LINE:COL`,
    /// a range landing at its start. A line past the last is the last,
    /// as `+N` is, and the landing is a jump, as `G`'s is.
    ///
    /// A locator this view cannot read belongs to another domain
    /// (`px12,34` for a canvas, `t1m23.4s` for media): it is said back
    /// and nothing moves. No registry tells them apart — text's own
    /// begins with a digit and every other one with the letter that
    /// names it.
    ///
    /// ```
    /// let mut vim = reovim_plugin_vim::Vim::default();
    /// vim.loaded(b"one\ntwo\n");
    /// vim.goto("2:3");
    /// assert_eq!(vim.point(Some("/a"), true, None), Ok("/a:2:3".into()));
    /// ```
    pub fn goto(&mut self, locator: &str) -> Effect {
        let Some((line, column)) = placed(locator) else {
            return self
                .error(&alloc::format!("E: invalid locator: {locator}"));
        };
        self.set_previous();
        let Some(column) = column else {
            self.begin(self.at_line(line));
            return Effect::Moved;
        };
        let pos = Pos { line: line.max(1) - 1, column: column.max(1) - 1 };
        self.cursor = self.byte_at(pos);
        self.clamp();
        self.want = self.wanted();
        Effect::Moved
    }

    /// KEYS bound to LINE, which `/self/vim/bin/bind` runs: no LINE
    /// takes the binding away, and there is no `unbind`.
    ///
    /// KEYS is the key notation, `<Leader>` among its names, and LINE
    /// is a `:` line kept as typed and not read until its key. The
    /// keymap cell is a script of one `bind KEYS LINE` a line, read
    /// back at blanks, so KEYS holds no blank — nvim's `:map` spells
    /// one `<Space>` — and LINE holds no line end.
    pub fn bind(&mut self, keys: &str, line: &str) -> Effect {
        if keys.is_empty() {
            return self.error("E471: Argument required");
        }
        if keys.contains(char::is_whitespace) {
            return self.refused(keys);
        }
        if line.contains(['\n', '\r']) {
            return self.refused(line);
        }
        let Some(read) = self.keymap.read(keys) else {
            return self.refused(keys);
        };
        if line.is_empty() {
            return self.unbound(&read);
        }
        self.keymap.put(&read, keys, line);
        Effect::Moved
    }

    /// Every binding whose keys read the same, taken away; `E31`
    /// where there was none. There is no `unbind` of its own.
    fn unbound(&mut self, keys: &[Key]) -> Effect {
        if self.keymap.unbind(keys) {
            return Effect::Moved;
        }
        self.error("E31: No such mapping")
    }

    /// `E474` naming the argument that was refused.
    fn refused(&mut self, arg: &str) -> Effect {
        self.error(&alloc::format!("E474: Invalid argument: {arg}"))
    }

    fn processed(&mut self, key: Key, past: bool) -> Effect {
        let shown = !self.message.is_empty();
        self.message.clear();
        self.found.shown = None;
        let taken = self.bound_key(key, past);
        let effect = taken.unwrap_or_else(|| self.acted(key));
        let message = shown || !self.message.is_empty();
        if message && effect == Effect::Nothing {
            Effect::Moved
        } else {
            effect
        }
    }

    /// An `Alt` no binding knows, let go as `<Esc>` and its char in
    /// the origin and with the press this key was taken with, as
    /// every key a failed prefix is let go with.
    fn let_alt_go(&mut self, c: char) {
        let Some(taken) = self.taken.clone() else { return };
        for key in [Key::Esc, Key::Char(c)] {
            self.held.released.push(keyed(key, taken.origin, &taken.press));
        }
    }

    /// What a key no binding took does: the mode's own arm, and the
    /// bookkeeping of the change it may have made.
    ///
    /// An `Alt(c)` reaching here is `<Esc>` then `c`, as nvim has it
    /// in every mode — insert, visual, the `:` line, and normal, of
    /// which its help says nothing.
    fn acted(&mut self, key: Key) -> Effect {
        if let Key::Alt(c) = key {
            // Let go in the origin this key was taken with, and with
            // its press, as every key a prefix lets go is.
            self.let_alt_go(c);
            return Effect::Nothing;
        }
        if !self.repeat.replaying {
            self.repeat.record.push(key);
        }
        self.undone.walking = None;
        // While a `/` or `?` line is open the cursor stands on what
        // the preview found; the change the line runs begins where
        // the line was opened, which is where undo belongs.
        let origin =
            self.preview.map_or_else(|| self.cursor, |preview| preview.cursor);
        let before = self.position_of(self.inside(origin));
        // Caught before the mode this key leaves: a block insert's
        // Esc is still Insert here, so `finish_block`'s own edits, on
        // the way out, still read as typed; a `c` that enters Insert
        // is still Normal here, so its own delete does not.
        let typed = self.mode == Mode::Insert;
        let effect = match self.mode {
            Mode::Normal => self.normal(key),
            Mode::Insert => self.insert(key),
            Mode::Command => self.command(key),
            Mode::Visual(kind) => {
                let effect = self.visual(kind, key);
                self.settle(&effect);
                effect
            }
        };
        self.around(before, typed, effect)
    }

    /// The bookkeeping around one act: the change it made, the column
    /// a vertical motion will want, and the end a selection reaches.
    /// `before` and `typed` are read before the act, the key being
    /// free to leave the mode it was read in.
    ///
    /// Both doors into the engine's work come through here: a key
    /// through `acted`, and a motion that waited for its list through
    /// `matched`.
    pub(crate) fn around(
        &mut self,
        before: (usize, usize),
        typed: bool,
        mut effect: Effect,
    ) -> Effect {
        self.changing(before, &mut effect, typed);
        if !core::mem::take(&mut self.keep_want) {
            self.want = self.wanted();
        }
        if matches!(self.mode, Mode::Visual(_)) {
            self.to_end = self.want == usize::MAX;
        }
        effect
    }

    /// The clipboard's answer to `Effect::Clipboard`: the put goes on
    /// with `text`, or E353 when there is none.
    pub fn pasted(&mut self, text: Option<String>) -> Effect {
        let Some(put) = self.put.take() else { return Effect::Nothing };
        let before = self.position();
        let mut effect = if let Some(text) = text {
            self.put_text(&text, put)
        } else {
            self.error("E353: Nothing in register +");
            Effect::Moved
        };
        // A put, never typing, whatever mode asked for it.
        self.changing(before, &mut effect, false);
        self.settle(&effect);
        effect
    }

    /// The `\n` Vim writes at the end of a last line that has none, as
    /// `fixendofline` does: its own edit, through `marked`, since it
    /// reaches no key's `changing`.
    pub fn fixed_eol(&mut self) -> Option<Edit> {
        if self.text.is_empty() || self.text.ends_with('\n') {
            return None;
        }
        let at = self.text.len();
        self.text.push('\n');
        let edit = Edit { at, len: 0, text: String::from("\n"), join: false };
        self.marked(&edit);
        Some(edit)
    }

    /// The cursor to the byte `at`, at its line's first non-blank:
    /// where the Client said to begin, and the column wanted from
    /// then on, as nvim's `beginline` sets `w_set_curswant`.
    pub fn begin(&mut self, at: usize) {
        self.cursor = self.first_non_blank(at.min(self.bound()));
        self.clamp();
        self.want = self.wanted();
    }

    /// The text the Buffer held when it was opened, whole: cleared
    /// first, so two files of the same bytes are two Buffers, then
    /// `marks` too, so its index describes this empty start truly;
    /// `loaded`'s own diff over it then builds the index for what is
    /// read in, exact by construction, needing no reindex after.
    ///
    /// The `Place` this draft left behind is restored first when
    /// there is one and `line` is 0, else the cursor is put at the
    /// line the Client was started at, counted from 1, 0 being the
    /// first line, where it begins without one. Only then, and only
    /// when `arrive` (see that field) names this source, is it jumped
    /// to: a file mark elsewhere must land on the Buffer's own kept
    /// place, undo list and local marks, not before they are there.
    /// `jump_to`'s own `Land` tells a jump-list crossing from a file
    /// mark's, so 01.1's one line still does the whole of it.
    ///
    /// An empty jump list is seeded here at line 0 of this Buffer:
    /// nvim's own list already holds one entry from the start, `'`
    /// already the valid mark there too (`found_mark`'s own default
    /// answers the same position without needing an entry for it).
    /// `start` sets `source` before calling this, so the seed is
    /// named. The list is empty again only when its one entry was
    /// never named and a later `leave` forgot it, or a walk found it
    /// standing on the cursor's own line and dropped it (`Jumps::
    /// clean`) — seeding the new Buffer's own line 0 then is sound.
    pub(crate) fn opened(
        &mut self,
        bytes: &[u8],
        line: u64,
        arrive: Option<(Rd, Pos, Land)>,
    ) {
        self.text.clear();
        self.marks = Marks::default();
        self.loaded(bytes);
        let place = (line == 0)
            .then(|| self.buffer.and_then(|rd| self.places.remove(&rd)))
            .flatten();
        match place {
            Some(Place { cursor, top, undone, last_visual, marks }) => {
                self.cursor = cursor;
                self.top = top;
                self.undone = undone;
                self.last_visual = last_visual;
                self.marks.local = marks;
                self.resettled();
                // nvim's `buflist_getfpos` sets `w_set_curswant`
                // after it restores the cursor, as `begin` does for
                // a Buffer that left no `Place` behind.
                self.want = self.wanted();
            }
            None => self.begin(self.beginning(line)),
        }
        if self.jumps.list.is_empty() {
            let source = self.source;
            self.jumps.push(Jump { source, pos: Pos::default() });
        }
        if let Some((rd, pos, land)) = arrive
            && Some(rd) == self.source
        {
            self.jump_to(pos, land);
        }
        if let Some(greeting) = self.greeting.take() {
            self.message = greeting;
        }
    }

    pub(crate) fn arrived(&mut self, source: Option<Rd>) {
        if let Some(left) =
            self.left.take().filter(|left| Some(*left) != source)
        {
            self.alt = Some(left);
        }
        if self.alt == source {
            self.alt = None;
        }
    }

    pub(crate) fn greets(&mut self, message: String) {
        self.greeting = Some(message);
    }

    pub(crate) fn forgot(&mut self, draft: Rd, source: Option<Rd>) {
        self.places.remove(&draft);
        if let Some(source) = source {
            self.jumps.forget(source);
        }
    }

    /// Where `opened` begins with no kept `Place`: the byte line
    /// `line` starts at, counted from 1, or the text's start with 0.
    fn beginning(&self, line: u64) -> usize {
        if line == 0 {
            0
        } else {
            self.at_line(usize::try_from(line).unwrap_or(usize::MAX))
        }
    }

    /// A stay at the Plugin ends: `self.buffer` says which draft it
    /// was over, and what belongs to it is kept in a `Place`, by that
    /// rd, for the `opened` that finds it again. What belongs to the
    /// stay itself goes back to its defaults; what is kept goes
    /// untouched. A second call, `start` having failed before a draft
    /// was opened, finds no buffer and writes no `Place` over a good
    /// one.
    ///
    /// Every door a Buffer is left by is a jump, the cursor's own
    /// place pushed, unless the stay ends by a walk of the jump list
    /// itself: `arrive`'s own `Land::Walked` tells that door from
    /// every other one, set after the ask went out and read here
    /// before the swap's own `start` can take it. The entries naming
    /// no source at all are then dropped, nothing left to come back
    /// to a Buffer over nothing by.
    pub(crate) fn leave(&mut self) {
        let walked = matches!(self.arrive, Some((_, _, Land::Walked)));
        if self.buffer.is_some() && !walked {
            let pos = self.pos_at(self.cursor);
            self.jumps.push(Jump { source: self.source, pos });
        }
        if let Some(buffer) = self.buffer.take() {
            self.left = self.source;
            self.places.insert(
                buffer,
                Place {
                    cursor: self.cursor,
                    top: self.top,
                    undone: core::mem::take(&mut self.undone),
                    last_visual: self.last_visual.take(),
                    marks: core::mem::take(&mut self.marks.local),
                },
            );
        }
        self.jumps.forget_unnamed();
        self.mode = Mode::default();
        self.pending = Pending::default();
        self.joining = false;
        self.held = Held::default();
        self.found = Found::default();
        self.preview = None;
        self.macros.fed = false;
        self.macros.joined = false;
        self.bell = false;
        self.global = global::Global::default();
        self.keep_want = false;
        self.indent = 0;
        self.clipboard = None;
        self.put = None;
        self.anchor = 0;
        self.to_end = false;
        self.block = None;
        self.wild = None;
        self.message.clear();
        self.repeat.record.clear();
        self.repeat.first = false;
        self.prompt.at = char::default();
        self.prompt.before = Mode::default();
        self.prompt.ex.put(0, "");
        self.prompt.search.put(0, "");
        self.h = None;
        self.source = None;
        self.name = None;
    }

    /// A Buffer over nothing gains `rd` as its source: every jump it
    /// made while it had none is `rd`'s now, so it is still tracked
    /// once this Buffer is left and later found again by name.
    pub(crate) fn sourced(&mut self, rd: Rd) {
        self.jumps.named(rd);
        self.source = self.source.or(Some(rd));
    }

    /// The byte line `n` starts at, counted from 1 and clamped to the
    /// last line: the tty's `+N`, which only whoever holds the text
    /// can find.
    pub(crate) fn at_line(&self, n: usize) -> usize {
        let line = n.max(1) - 1;
        self.line_at(line.min(self.line_count() - 1)).unwrap_or(0)
    }

    /// Scroll only far enough to keep the cursor in the visible rows.
    fn scrolled(&mut self, height: usize) {
        let height = height.max(1);
        let line = self.position().0;
        if line < self.top {
            self.top = line;
        } else if line >= self.top.saturating_add(height) {
            self.top = line + 1 - height;
        }
    }

    /// What to show in a region of `rows` rows: the first byte of the
    /// first line shown, which moves to keep the cursor in it, the
    /// cursor itself and its shape, the selection and the search
    /// matches as byte spans, and the status word.
    #[must_use]
    pub fn view(&mut self, rows: usize) -> window::View {
        self.rows = rows;
        self.scrolled(rows);
        let height = rows.max(1);
        // A `/` or `?` line opened from visual draws its selection
        // still, stretched to the entry the preview shows.
        let mode = match self.mode {
            Mode::Command if self.preview.is_some() => self.prompt.before,
            mode => mode,
        };
        let selected = match mode {
            Mode::Visual(kind) => self.selection_spans(kind, self.top, height),
            _ => Vec::new(),
        };
        let room = SPANS - selected.len().min(SPANS);
        let marks = self.search_spans(self.top, height, room);
        // A match first, a selection last: the selection is on top
        // where the two overlap, whole even where it begins inside a
        // match.
        let mut spans: Vec<window::Span> =
            marks.into_iter().map(span(MATCH)).collect();
        spans.extend(selected.into_iter().map(span(SELECTION)));
        window::View {
            over: self.buffer.unwrap_or(NONE),
            top: self.line_at(self.top).unwrap_or(0) as u64,
            cursor: self.cursor as u64,
            // Insert puts the cursor between chars, as nvim draws it;
            // every other mode is on one.
            shape: if self.mode == Mode::Insert { BAR } else { BLOCK },
            spans,
            status: self.status(),
        }
    }

    /// The status word: a message, the command line being typed, or
    /// the mode.
    #[must_use]
    pub fn status(&self) -> String {
        let mut status = match self.mode {
            _ if !self.message.is_empty() => self.message.clone(),
            Mode::Command => self.prompt_status(),
            _ if self.waiting() || self.found.shown.is_some() => self.row(),
            Mode::Normal => String::new(),
            Mode::Insert => String::from("-- INSERT --"),
            Mode::Visual(Visual::Char) => String::from("-- VISUAL --"),
            Mode::Visual(Visual::Line) => String::from("-- VISUAL LINE --"),
            Mode::Visual(Visual::Block) => String::from("-- VISUAL BLOCK --"),
        };
        let separator = if status.is_empty() { "" } else { " " };
        if let Some(name) = self.macros.recording {
            status.push_str(separator);
            status.push_str("recording @");
            status.push(name.to_ascii_lowercase());
        }
        status
    }
}

/// A byte range of the text as a span of `style`.
fn span(style: u64) -> impl Fn((usize, usize)) -> window::Span {
    move |(from, to)| window::Span { from: from as u64, to: to as u64, style }
}

const fn is_digit(key: Key) -> bool {
    matches!(key, Key::Char('0'..='9'))
}

/// The place a locator names: the line, and the column where it holds
/// one, both counted from 1. A range is its start, and an end this
/// view cannot read makes the whole no locator of its own, so
/// `12-zz` is refused where `12-14` lands.
fn placed(locator: &str) -> Option<(usize, Option<usize>)> {
    let (from, to) = match locator.split_once('-') {
        Some((from, to)) => (from, Some(to)),
        None => (locator, None),
    };
    let at = numbered(from)?;
    if let Some(to) = to {
        numbered(to)?;
    }
    Some(at)
}

/// One end of a locator: `LINE`, or `LINE:COL`, digits alone.
fn numbered(text: &str) -> Option<(usize, Option<usize>)> {
    let Some((line, column)) = text.split_once(':') else {
        return Some((text.parse().ok()?, None));
    };
    Some((line.parse().ok()?, Some(column.parse().ok()?)))
}

/// `Moved` when something did.
const fn moved(yes: bool) -> Effect {
    if yes { Effect::Moved } else { Effect::Nothing }
}

/// The edits `effect` made, onto `edits`.
fn collect(edits: &mut Vec<Edit>, effect: Effect) {
    match effect {
        Effect::Edit(edit) => edits.push(edit),
        Effect::Edits(more) => edits.extend(more),
        _ => {}
    }
}

/// The edits `effect` made, borrowed and in order.
fn effect_edits(effect: &Effect) -> &[Edit] {
    match effect {
        Effect::Edit(edit) => core::slice::from_ref(edit),
        Effect::Edits(edits) => edits,
        _ => &[],
    }
}

/// `byte`, from before any of `later`'s own edits, carried through
/// them into the coordinates they leave: one past it shifts it by
/// what it put in less what it took; one at or before it moves it to
/// its own start, since whatever stood there did not survive; one
/// that reaches past it leaves it where it is.
fn carried(byte: usize, later: &[Edit]) -> usize {
    later.iter().fold(byte, |b, e| {
        if e.at + e.len <= b {
            b + e.text.len() - e.len
        } else if e.at <= b {
            e.at
        } else {
            b
        }
    })
}

/// The narrowest [`Edit`], `at` added to its own start, that turns
/// `old` into `new`: the bytes common at both ends left alone, chars
/// never split. `substitute` rebuilds a whole line to find what
/// changed in it; a mark past the changed bytes must keep its column,
/// which only holds when the edit `moved` and `batch_span` read is
/// this narrow, not the whole line.
pub(crate) fn minimal_edit(
    at: usize,
    old: &str,
    new: &str,
    join: bool,
) -> Edit {
    let prefix: usize = old
        .chars()
        .zip(new.chars())
        .take_while(|(a, b)| a == b)
        .map(|(a, _)| a.len_utf8())
        .sum();
    let suffix: usize = old[prefix..]
        .chars()
        .rev()
        .zip(new[prefix..].chars().rev())
        .take_while(|(a, b)| a == b)
        .map(|(a, _)| a.len_utf8())
        .sum();
    Edit {
        at: at + prefix,
        len: old.len() - prefix - suffix,
        text: String::from(&new[prefix..new.len() - suffix]),
        join,
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::String;

    use super::{Edit, carried};

    fn edit(at: usize, len: usize, text: &str) -> Edit {
        Edit { at, len, text: String::from(text), join: false }
    }

    #[test]
    fn carried_reads_a_byte_through_one_edit_after_it_three_ways() {
        // An edit that already ended (`reach <= b`) shifts `b` by
        // what it put in less what it took.
        let ended = &[edit(1, 2, "xyz")][..];
        assert_eq!(carried(6, ended), 7, "6 + (3 - 2)");
        // An edit that reaches `b` or starts before it collapses `b`
        // onto its own start: nothing that stood there survived.
        let reaches_it = &[edit(4, 4, "")][..];
        assert_eq!(carried(6, reaches_it), 4, "taken, to that edit's start");
        // An edit that starts after `b` has not reached it yet.
        let not_yet = &[edit(6, 1, "xy")][..];
        assert_eq!(carried(4, not_yet), 4, "ahead of it, left alone");
    }
}
