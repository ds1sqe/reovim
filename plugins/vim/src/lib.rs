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
pub use visual::Visual;

use jumps::{Jump, Jumps};
use marks::Marks;
use operator::{Find, Operator};
use reovim_client_api::{BAR, BLOCK, MATCH, NONE, SELECTION, window};
use reovim_client_sh::{Line, Shell};
use reovim_kernel_interface::{Handle, Rd};
use search::Search;
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
    /// Register keys for the Client queue, never keyboard writes.
    ///
    /// ```
    /// use reovim_plugin_vim::{Effect, Key, Vim, notation};
    /// let mut vim = Vim::default();
    /// vim.loaded(b"abc\n");
    /// for key in notation::keys("qaxq@").unwrap() { vim.key(key); }
    /// assert_eq!(vim.key(Key::Char('a')),
    ///     Effect::Replay(vec![Key::Char('x')]));
    /// ```
    Replay(Vec<Key>),
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
    /// use reovim_plugin_vim::{Effect, Key, Switch, Vim};
    /// for key in [Key::Ctrl('^'), Key::Ctrl('6')] {
    ///     assert_eq!(Vim::default().key(key),
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
    cursor: usize,
    top: usize,
    undone: Undone,
    last_visual: Option<Selection>,
    /// This Buffer's local marks, `a`-`z` and `' [ ] < > .`, and the
    /// index that moves them.
    marks: Marks,

    // Kept.
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
    /// the marks in order, [`Self::marked`] being the one door,
    /// unless a replay is putting them through `key` a second time.
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
        if self.repeat.replaying {
            return;
        }
        let own = effect_edits(effect).first().is_some_and(|e| !e.join);
        self.continued(effect);
        let edits = effect_edits(effect);
        let starting = edits.first().is_some_and(|e| !e.join);
        if starting {
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
    fn motion(&mut self, motion: Motion, n: usize) -> Effect {
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

    /// `.`: the last change's keys fed back in, its count replaced
    /// by `count` when there is one; every edit they make, in order.
    fn replay(&mut self, count: usize) -> Effect {
        self.repeat.record.clear();
        let mut keys: Vec<Key> = self.repeat.last.clone();
        if count > 0 {
            let digits = keys.iter().take_while(|k| is_digit(**k)).count();
            keys.drain(..digits);
            let count = alloc::format!("{count}");
            keys.splice(0..0, count.chars().map(Key::Char));
        }
        let mut edits = Vec::new();
        for key in keys {
            let effect = self.fed(key, true);
            collect(&mut edits, effect);
        }
        if edits.is_empty() { Effect::Nothing } else { Effect::Edits(edits) }
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
        self.text.replace_range(at..self.cursor, &text);
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
        self.text.replace_range(at..self.cursor, "");
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
        self.text.insert_str(at, text);
        self.cursor += text.len();
        self.edit(at, 0, String::from(text))
    }

    fn backspace(&mut self) -> Effect {
        self.indent = 0;
        let Some(c) = self.text[..self.cursor].chars().next_back() else {
            return Effect::Nothing;
        };
        self.cursor -= c.len_utf8();
        self.text.remove(self.cursor);
        self.edit(self.cursor, c.len_utf8(), String::new())
    }

    /// An edit already applied to the text, joined to the revision
    /// under way when an insert is.
    const fn edit(&mut self, at: usize, len: usize, text: String) -> Effect {
        let join = core::mem::replace(&mut self.joining, true);
        Effect::Edit(Edit { at, len, text, join })
    }

    /// `len` bytes at `at` become `text`, as a revision of their own.
    fn replace(&mut self, at: usize, len: usize, text: String) -> Effect {
        self.text.replace_range(at..at + len, &text);
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
    fn resettled(&mut self) {
        while !self.text.is_char_boundary(self.cursor) {
            self.cursor -= 1;
        }
        if self.mode == Mode::Normal {
            self.clamp();
        }
    }

    /// One key in the current mode; a message showing or shown goes
    /// with it, so the View is shown again.
    pub fn key(&mut self, key: Key) -> Effect {
        self.macros.fed = false;
        self.macros.joined = false;
        if self.macros.recording.is_some() {
            self.macros.record.push(key);
        }
        self.processed(key)
    }

    /// A replay key. Nested repeats restore their caller's context.
    ///
    /// ```
    /// use reovim_plugin_vim::{Key, Vim};
    /// let mut vim = Vim::default();
    /// vim.loaded(b"abc\n");
    /// vim.fed(Key::Char('x'), false);
    /// assert_eq!(vim.text(), "bc\n");
    /// ```
    pub fn fed(&mut self, key: Key, repeat: bool) -> Effect {
        let fed = core::mem::replace(&mut self.macros.fed, true);
        let was = self.repeat.replaying;
        self.repeat.replaying = repeat;
        let effect = self.processed(key);
        self.repeat.replaying = was;
        self.macros.fed = fed;
        effect
    }

    fn processed(&mut self, key: Key) -> Effect {
        if !self.repeat.replaying {
            self.repeat.record.push(key);
        }
        let shown = !self.message.is_empty();
        self.message.clear();
        self.undone.walking = None;
        let before = self.position();
        // Caught before the mode this key leaves: a block insert's
        // Esc is still Insert here, so `finish_block`'s own edits, on
        // the way out, still read as typed; a `c` that enters Insert
        // is still Normal here, so its own delete does not.
        let typed = self.mode == Mode::Insert;
        let mut effect = match self.mode {
            Mode::Normal => self.normal(key),
            Mode::Insert => self.insert(key),
            Mode::Command => self.command(key),
            Mode::Visual(kind) => {
                let effect = self.visual(kind, key);
                self.settle(&effect);
                effect
            }
        };
        self.changing(before, &mut effect, typed);
        if !core::mem::take(&mut self.keep_want) {
            self.want = self.wanted();
        }
        if matches!(self.mode, Mode::Visual(_)) {
            self.to_end = self.want == usize::MAX;
        }
        let message = shown || !self.message.is_empty();
        if message && effect == Effect::Nothing {
            Effect::Moved
        } else {
            effect
        }
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
    /// where the Client said to begin.
    pub fn begin(&mut self, at: usize) {
        self.cursor = self.first_non_blank(at.min(self.bound()));
        self.clamp();
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
        let selected = match self.mode {
            Mode::Visual(kind) => self.selection_spans(kind, self.top, height),
            _ => Vec::new(),
        };
        let marks = self.search_spans(self.top, height);
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
            Mode::Normal => String::new(),
            Mode::Insert => String::from("-- INSERT --"),
            Mode::Command => self.prompt_status(),
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
