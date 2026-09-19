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

pub use operator::Motion;
pub use plugin::{Editor, boot};
pub use reovim_client_api::{Key, notation};
pub use visual::Visual;

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
    /// `:exp[lore][!] [NAME]`: the Client asked to show an rd in this
    /// region, in the Plugin its table gives that rd's shape. The name
    /// given, resolved as `:w NAME` is, or the directory the file is
    /// in with none; `true` for `:explore!`, which goes although the
    /// draft is unsaved.
    Explore(Option<String>, bool),
    /// `:e[dit][!] PATH`: the same ask over the file that name holds,
    /// which comes back a Vim of its own. A name that is not there is
    /// E345, where `:explore`'s is E344.
    Open(String, bool),
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
    /// `:q!`, `ZQ`: whatever the Buffer holds, exit 0.
    Force,
    /// `:wq`, `:x`, `ZZ`: written, then exit 0.
    Write,
    /// `:cq`: exit 1, so the caller uses nothing.
    Abort,
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

/// Where the cursor was before each change, for undo to put it back
/// as nvim does, and how many of those changes stand undone.
#[derive(Debug, Default)]
struct Undone {
    before: Vec<(usize, usize)>,
    count: usize,
    /// An undo (`false`) or redo (`true`) is waiting for its reload.
    walking: Option<bool>,
}

/// The text, the cursor as a byte index on a char boundary, the first
/// line shown, and the mode. In normal mode the cursor sits on a
/// char, never past the last one of its line; insert may sit past it.
#[derive(Debug, Default)]
pub struct Vim {
    text: String,
    cursor: usize,
    top: usize,
    mode: Mode,
    pending: Pending,
    /// The next insert edit joins the revision under way.
    joining: bool,
    /// The screen column a vertical motion wants, `usize::MAX` after
    /// `$`.
    want: usize,
    /// The last key kept `want` rather than setting it.
    keep_want: bool,
    /// Bytes of indent `o`, `O` or Enter just copied, dropped again if
    /// nothing is typed on the line.
    indent: usize,
    /// The unnamed register, `None` until something fills it; an
    /// empty yank fills it, an empty delete does not.
    register: Option<Register>,
    /// `"a` to `"z`.
    registers: BTreeMap<char, Register>,
    /// Text for the clipboard, until the tui takes it.
    clipboard: Option<String>,
    put: Option<Put>,
    /// What the clipboard last gave, for `.` to put again.
    last_clip: Register,
    /// The last `f`, `t`, `F` or `T`, for `;` and `,`.
    find: Option<(Find, char)>,
    repeat: Repeat,
    undone: Undone,
    /// The selection's other end in visual mode.
    anchor: usize,
    /// `$` was the last motion of a selection: it reaches every line's
    /// end.
    to_end: bool,
    last_visual: Option<Selection>,
    /// A block insert under way.
    block: Option<BlockInsert>,
    search: Search,
    prompt: Prompt,
    /// What Tab last found on the `:` line; any other key drops it.
    wild: Option<Wild>,
    /// Shown until the next key.
    message: String,
    /// Where a line of the shell's typed on the `:` line stands: the
    /// mirror of the directory the Client started in, which `:cd`
    /// moves and `:pwd` says.
    shell: Shell,
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
    fn undone_cursor(&mut self, top: usize, redo: bool) {
        let made = self.undone.before.len() - self.undone.count;
        let before = if redo {
            self.undone.count = self.undone.count.saturating_sub(1);
            self.undone.before.get(made)
        } else {
            self.undone.count = (self.undone.count + 1).min(made.max(1));
            made.checked_sub(1).and_then(|i| self.undone.before.get(i))
        };
        let line = match before {
            Some(&(was, _)) if was + 1 == top && top > 0 => was,
            _ => top,
        };
        let start = self.line_at(line).unwrap_or_else(|| {
            self.line_at(self.line_count() - 1).unwrap_or(0)
        });
        self.cursor = match before {
            Some(&(was, column)) if was == line => {
                self.at_column(start, column)
            }
            _ => self.at_screen_column(start, self.want),
        };
        self.want = self.wanted();
    }

    /// A change is being made: the cursor `before` it kept for undo;
    /// what stood undone is forgotten, as a new change ends redo.
    fn changing(&mut self, before: (usize, usize), effect: &Effect) {
        let change = match effect {
            Effect::Edit(edit) => !edit.join,
            Effect::Edits(_) => true,
            Effect::Undo => return self.undone.walking = Some(false),
            Effect::Redo => return self.undone.walking = Some(true),
            _ => false,
        };
        if change && !self.repeat.replaying {
            let made = self.undone.before.len() - self.undone.count;
            self.undone.before.truncate(made);
            self.undone.before.push(before);
            self.undone.count = 0;
        }
    }

    /// The cursor's line and column, in chars from 0.
    #[must_use]
    pub fn position(&self) -> (usize, usize) {
        self.position_of(self.cursor)
    }

    /// A digit onto the count: `0` only after another, being a motion
    /// else.
    const fn digit(&mut self, key: Key) -> bool {
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
            return self.lines(op, first * n);
        }
        if let Some(motion) = Motion::of(key, count) {
            return self.motion(motion, n);
        }
        let effect = match key {
            Key::Char(first @ ('g' | 'f' | 't' | 'F' | 'T' | '"')) => {
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
            'g' if c == 'g' => Motion::Line(count.saturating_sub(1)),
            'i' | 'a' => Motion::Object { inner: first == 'i', kind: c },
            'f' => Motion::Find(Find::Forward, c),
            't' => Motion::Find(Find::ForwardTo, c),
            'F' => Motion::Find(Find::Backward, c),
            'T' => Motion::Find(Find::BackwardTo, c),
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
                self.lines(op, count * n)
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
            Some((op, count)) => self.operate(op, count * n, motion),
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
        self.repeat.replaying = true;
        let mut edits = Vec::new();
        for key in keys {
            let effect = self.key(key);
            collect(&mut edits, effect);
        }
        self.repeat.replaying = false;
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
            Key::Ctrl(_) => Effect::Nothing,
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
        moved(count > 0)
    }
}

/// The engine's own calls: what a key does, and what the Plugin at
/// the bottom of this file needs of it.
impl Vim {
    /// `bytes` as the text, lossily; the cursor stays where it can.
    /// True when the text differs from before.
    pub fn loaded(&mut self, bytes: &[u8]) -> bool {
        let text = String::from_utf8_lossy(bytes);
        let walking = self.undone.walking.take();
        if text == self.text {
            return false;
        }
        let common =
            self.text.bytes().zip(text.bytes()).take_while(|(a, b)| a == b);
        let top = self.text[..common.count()].matches('\n').count();
        self.text = text.into_owned();
        match walking {
            Some(redo) => self.undone_cursor(top, redo),
            None => self.cursor = self.cursor.min(self.bound()),
        }
        while !self.text.is_char_boundary(self.cursor) {
            self.cursor -= 1;
        }
        if self.mode == Mode::Normal {
            self.clamp();
        }
        true
    }

    /// One key in the current mode; a message showing or shown goes
    /// with it, so the View is shown again.
    pub fn key(&mut self, key: Key) -> Effect {
        if !self.repeat.replaying {
            self.repeat.record.push(key);
        }
        let shown = !self.message.is_empty();
        self.message.clear();
        self.undone.walking = None;
        let before = self.position();
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
        self.changing(before, &effect);
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
        let effect = if let Some(text) = text {
            self.put_text(&text, put)
        } else {
            self.message = String::from("E353: Nothing in register +");
            Effect::Moved
        };
        self.changing(before, &effect);
        self.settle(&effect);
        effect
    }

    /// The `\n` Vim writes at the end of a last line that has none, as
    /// `fixendofline` does.
    pub fn fixed_eol(&mut self) -> Option<Edit> {
        if self.text.is_empty() || self.text.ends_with('\n') {
            return None;
        }
        let at = self.text.len();
        self.text.push('\n');
        Some(Edit { at, len: 0, text: String::from("\n"), join: false })
    }

    /// The cursor to the byte `at`, at its line's first non-blank:
    /// where the Client said to begin.
    pub fn begin(&mut self, at: usize) {
        self.cursor = self.first_non_blank(at.min(self.bound()));
        self.clamp();
    }

    /// The text the Buffer held when it was opened, and the cursor at
    /// the line the Client was started at, counted from 1; 0 is the
    /// first line, where it begins without one.
    pub(crate) fn opened(&mut self, bytes: &[u8], line: u64) {
        self.loaded(bytes);
        let at = if line == 0 {
            0
        } else {
            self.at_line(usize::try_from(line).unwrap_or(usize::MAX))
        };
        self.begin(at);
    }

    /// The byte line `n` starts at, counted from 1 and clamped to the
    /// last line: the tty's `+N`, which only whoever holds the text
    /// can find.
    pub(crate) fn at_line(&self, n: usize) -> usize {
        let line = n.max(1) - 1;
        self.line_at(line.min(self.line_count() - 1)).unwrap_or(0)
    }

    /// What to show in a region of `rows` rows: the first byte of the
    /// first line shown, which moves to keep the cursor in it, the
    /// cursor itself and its shape, the selection and the search
    /// matches as byte spans, and the status word.
    #[must_use]
    pub fn view(&mut self, rows: usize) -> window::View {
        let height = rows.max(1);
        let line = self.position().0;
        if line < self.top {
            self.top = line;
        } else if line >= self.top + height {
            self.top = line + 1 - height;
        }
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
        match self.mode {
            _ if !self.message.is_empty() => self.message.clone(),
            Mode::Normal => String::new(),
            Mode::Insert => String::from("-- INSERT --"),
            Mode::Command => self.prompt_status(),
            Mode::Visual(Visual::Char) => String::from("-- VISUAL --"),
            Mode::Visual(Visual::Line) => String::from("-- VISUAL LINE --"),
            Mode::Visual(Visual::Block) => String::from("-- VISUAL BLOCK --"),
        }
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
