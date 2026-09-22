#![no_std]
#![forbid(unsafe_code)]
//! What the Client's own Driver and the Plugins over it both spell.
//!
//! `/self/dev` holds the Client's devices as cells: the keyboard is a
//! stream of [`keyboard::Press`], a region is asked how big it is with
//! [`window::Size`] and answers a [`window::View`], and
//! [`open::Ask`] is what a Plugin asks its Client to show. The
//! declarations are here, beside neither side, as modules/text/api
//! stands beside modules/text: a Plugin links this and never the
//! Client.

extern crate alloc;

pub mod notation;

use reovim_kernel_interface::Rd;
use reovim_sdk::declare;

/// The rd of nothing: what [`open::Ask`]'s `target` and
/// [`window::View`]'s `over` hold where there is no rd — a region over
/// a file that is not on disk, a Plugin that has shown nothing yet.
///
/// No tree gives it out: the server counts rds up from 0 and a Client
/// from its own base, and the root itself is `Rd(0)`, which a region
/// may perfectly well be opened on.
pub const NONE: Rd = Rd(u64::MAX);

declare! {
    /// `/self/dev/input/keyboard`: what was typed, a stream.
    ///
    /// A write of the cell is key codes, eight bytes little endian
    /// each, so a line typed from another Client is one op; a read
    /// gives one `Press` and takes it for that handle alone.
    /// `when` and `source` are the Client's and not the writer's, as
    /// `/sys/log`'s `time` and `mark` are the Kernel's: `when` is the
    /// clock the Host was made with, and `source` the rd of whoever
    /// put the key there. Neither is a cursor; they are there from the
    /// first key so a stream recorded today merges and replays later.
    api keyboard ver 1 {}
    record Press { key: u64, when: u64, source: Rd }
}

declare! {
    /// `/self/dev/windows/0`: one region of a Client's display.
    ///
    /// `size` is how big it is, in the cells of whatever draws it;
    /// `view` is what the Plugin holding the region answers. A `View`
    /// names the rd it is in the bytes of and is otherwise in those
    /// bytes: the first byte shown, where the cursor is and what shape
    /// it takes, styled spans, and the Plugin's own word for its
    /// region. How that becomes a picture is the Client's, which reads
    /// `over` for the text to lay the View out on.
    api window ver 1 {}
    record Size { cols: u64, rows: u64 }
    record View { over: Rd, top: u64, cursor: u64, shape: u64,
                  spans: [Span; 4096], status: utf8(4096) }
    record Span { from: u64, to: u64, style: u64 }
}

declare! {
    /// `/self/dev/display`: what the Client shows, a Container of
    /// however many forms the tree under it holds — `0` today.
    ///
    /// `display/0/info` is what THIS one is, a file a fact, as
    /// `/sys/info` is what the Kernel is: `rows` and `cols`, decimal
    /// text the drawer wrote, are the whole terminal (not the
    /// region's own `windows/0/size`, which is `rows - 1`, the status
    /// row taken out), and a missing file is a fact this display has
    /// not — a gui would add `pixels`, `ppi`, a length, none of it
    /// moving what is here.
    ///
    /// `display/0/text` is what it shows: the frame the drawer laid
    /// out. `lines` are its rows top to bottom, a tab already its
    /// spaces and the rows below the last not sent; `status` stays
    /// apart, as it does in a [`window::View`]. `row` and `col` are
    /// the cursor: the line, and the BYTE in that line's `text` —
    /// what a column is belongs to the reader, a tui counting cells
    /// and a gui glyphs. `rows` is carried again here, the height
    /// this frame was drawn for, so one read of `text` alone says
    /// what it was drawn for and a resize cannot land between two
    /// reads of it.
    ///
    /// A `Run` is in the bytes of its `Line`'s `text`, as a `View`'s
    /// `Span` is in the bytes of `over`, `to` one past `text`'s own
    /// length where the run runs on into the newline `text` does not
    /// carry. `style` is a look ([`reovim_sdk::look`]), the drawer's
    /// theme having resolved it already: never 0, and a line's runs
    /// never overlap and stand in order of `from`.
    api display ver 1 {}
    record Screen { rows: u64, row: u64, col: u64, shape: u64,
                    lines: [Line; 1024], status: utf8(4096) }
    record Line { text: utf8(4096), runs: [Run; 1024] }
    record Run { from: u64, to: u64, style: u64 }
}

declare! {
    /// `/self/<plugin>/<n>/looks`: what a Plugin draws over a Buffer.
    ///
    /// `<n>` is the name the draft goes by under `/self/dev/buffers`,
    /// so one Plugin serves one node per Buffer it decorates. The
    /// drawer finds them by this declaration and not by the Plugin's
    /// name: it walks the homes under `/self`, and a node that does not
    /// offer this api is passed over.
    ///
    /// `runs` answers the looks over the bytes `from..to`, which is the
    /// viewport and no more, in whatever order the Plugin found them. A
    /// `Look` is in the bytes of the Buffer, as a [`window::Span`] is:
    /// `style` is a meaning of the theme ([`reovim_sdk::sgr`]) and
    /// never a colour, and `priority` orders the painting, the higher
    /// over the lower and the region's own spans over both. The Plugin
    /// is called and never ticked: it parses what the range holds when
    /// it is asked.
    api looks ver 1 {
        0 runs(args: Range) -> [Look; 4096]: READ;
    }
    record Range { from: u64, to: u64 }
    record Look { from: u64, to: u64, style: u64, priority: u64 }
}

declare! {
    /// `/self/dev/open`: the region shows something else.
    ///
    /// A Plugin asks its Client to show `target` here, in the Plugin
    /// `plugin` names or, with an empty name, in the one the Client's
    /// table gives that rd's shape. `name` is the path a region over
    /// nothing was opened with, there being no such file, and `line`
    /// the line to begin at, counted from 1; 0 is none of either.
    ///
    /// `locator` is where in what it opens the region is to land, in
    /// the grammar the Plugin that serves the target owns: text's is
    /// `LINE`, `LINE:COL`, `FROM-TO` and `LINE:COL-LINE:COL`, and
    /// another domain's begins with a letter that names it. Whoever
    /// makes the ask never reads it; an empty one asks for nothing,
    /// and one the Plugin cannot read is that Plugin's to answer.
    ///
    /// The call answers nothing, the asking Plugin being on the stack:
    /// the Client records the ask and acts on it at the next read of
    /// the region's view, which is after the call it is inside has
    /// returned. A Client with no Plugin for the ask leaves the region
    /// as it is and says so in the log.
    ///
    /// `back` is the other way: the region shows again what it showed
    /// before the last `open`, in a Plugin made afresh. A region that
    /// has taken no step yet has nothing to go back to, and the Client
    /// ends there.
    api open ver 1 {
        0 open(ask: Ask) -> (): WRITE;
        1 back() -> (): WRITE;
    }
    record Ask {
        target: Rd, plugin: utf8(64), name: utf8(4096), line: u64,
        locator: utf8(64)
    }
}

declare! {
    /// `/self/dev/buffers`: the Buffers this Client holds, each a
    /// draft over one of the server's.
    ///
    /// `open` takes the rd a `text.open` would take, the caller having
    /// resolved it, and a source a draft is already over answers that
    /// draft, so a region shown twice costs one Buffer. `scratch` is a
    /// draft over nothing, which gains a source at a `save_as`.
    /// `close` takes the draft out of the tree with the handle it held
    /// on its Buffer; what was pending goes with it.
    api buffers ver 1 {
        0 open(source: Rd) -> Rd: WRITE;
        1 scratch() -> Rd: WRITE;
        2 close(draft: Rd) -> (): WRITE;
    }
}

declare! {
    /// `/self/dev/buffers/N`: the draft's own api, beside the `text`,
    /// `history` and `publish` every Buffer offers.
    ///
    /// A `replace` on a draft joins the pending revision and `sync` is
    /// what makes it a state on the server, which is the one place a
    /// draft and a server Buffer differ: a caller that wants a
    /// revision boundary asks for one. Once the server refuses a
    /// flush because another Client's run landed in front of this
    /// one, the draft is stale from then on: `sync`, an undo or redo,
    /// a save and any further edit all answer `Stale` too, with no
    /// wire message, keeping what was typed and what was pending
    /// alike, until `reload` takes the server's head in their place.
    api draft ver 1 {
        0 sync() -> (): WRITE;
        1 reload() -> (): WRITE;
    }
}

declare! {
    use crate::keyboard::Press;

    /// The resolver's own records: what a Client hands whatever turns
    /// input into editing decisions, and what it is handed back.
    ///
    /// A resolver is neither a Driver nor a Resource — the Rust path
    /// calls one with typed calls of its own — so this api declares
    /// data and no method: the records are what crosses where the
    /// other side is not Rust, and the shape both sides agree on
    /// where it is.
    ///
    /// Five records are what resolving is, and none of them is the
    /// text. [`Input`] is one key on its way in, a [`Press`] kept
    /// whole with an `origin` ([`origin`](crate::origin)) saying
    /// whose it is — the user's, a macro's or a `.`'s — and
    /// `released` saying whether a held prefix that matched nothing
    /// let it go, which is a second fact and not another origin: a
    /// macro's released key is still the macro's. A synthetic key
    /// carries the `Press` of the key that made it, so no clock and
    /// no source is invented and a recorded stream replays. [`Fed`]
    /// is the keys a macro or a `.` replays, which the host pushes
    /// back as `Input`s rather than the resolver feeding itself.
    /// [`Command`] is a line for the shell, whether it was typed or a
    /// binding resolved to it: a name on PATH and its arguments, the
    /// grammar a resolver reads off its own front — Vim's address and
    /// bang — already flags on the line. [`Reply`] answers a wait:
    /// `kind` ([`reply`](crate::reply)) says which answer it is,
    /// `text` what the clipboard held or which operation was refused,
    /// and `error` the `Error` as the Kernel numbers it, read for a
    /// `REFUSED` alone; a continuation no longer waited for is stale
    /// and means nothing. [`State`] is what a frame reads once: the
    /// `mode` ([`mode`](crate::mode)), the `showcmd` a pending count
    /// and operator spell (`3d`), and `waiting` the continuation
    /// still unanswered, 0 for none. The host reads no other field of
    /// a resolver.
    ///
    /// What a resolver decides beyond a line for the shell is its
    /// own host's, in that host's own types: an edit belongs to the
    /// api of the region it is made in, a jump to the Plugin that
    /// serves what is jumped to, a search to the search Module's own
    /// records. Nothing of them is declared here, and nothing is
    /// declared here for a consumer that does not exist: when one
    /// does, it brings its own declaration with it.
    api resolver ver 1 {}
    record Input { press: Press, origin: u64, released: bool }
    record Fed { keys: [u64; 4096], repeat: bool }
    record Command { line: utf8(4096) }
    record Reply { continuation: u64, kind: u64, text: utf8(65536),
                   error: u64 }
    record State { mode: u64, showcmd: utf8(64), waiting: u64 }
}

/// Whose a [`resolver::Input`] is, its `origin`. Whether a failed
/// prefix let that key go is `released` beside it, not an origin of
/// its own: a key keeps its maker through the release.
pub mod origin {
    /// A key the user typed, which is every key with a clock and a
    /// source of its own.
    pub const TYPED: u64 = 0;
    /// A key a macro replays, carrying the `Press` of the `@a`.
    pub const FED: u64 = 1;
    /// A key a `.` repeats, carrying the `Press` of the `.`.
    pub const REPEAT: u64 = 2;
}

/// Which answer a [`resolver::Reply`] is, its `kind`.
pub mod reply {
    /// The clipboard's own text, for a put that waits on it.
    pub const PASTED: u64 = 0;
    /// The operation did not happen; `text` names it as the row
    /// would and `error` says what stopped it.
    pub const REFUSED: u64 = 1;
}

/// Which mode a [`resolver::State`] is in, its `mode`.
///
/// A mode is the Client's side of the line and never crosses to the
/// Kernel or to a Module: what a resolver calls its modes it numbers
/// here, so the Client that draws the frame and the `mode` cell under
/// its home read the field without knowing the resolver's own enum.
/// The names are Vim's, which is the resolver reovim ships; a
/// resolver with fewer uses fewer, and one with more adds them here
/// beside its own consumer.
pub mod mode {
    /// Keys are commands: the mode an editor opens in.
    pub const NORMAL: u64 = 0;
    /// Keys are text.
    pub const INSERT: u64 = 1;
    /// A line is being composed for the shell — `:` — which is the
    /// bridge out of the editor and a mode like any other.
    pub const COMMAND: u64 = 2;
    /// A run of characters is selected.
    pub const VISUAL: u64 = 3;
    /// Whole lines are selected.
    pub const VISUAL_LINE: u64 = 4;
    /// A block of columns is selected.
    pub const VISUAL_BLOCK: u64 = 5;
}

/// The nine a span may mean, the sdk's own theme: [`SELECTION`] and
/// [`MATCH`] are the Plugin's own, and no Driver writes them; the rest
/// stand for the same colors a Driver under `/bin` prints in. A
/// theme resolves each to a look ([`reovim_sdk::look`]) in one place,
/// clients/tui's own layout.
pub use reovim_sdk::sgr::{
    BLUE, BOLD, DIM, GREEN, MATCH, RED, RED_BOLD, SELECTION, YELLOW,
};

/// A look: what a run of text is drawn in, bits and never a theme's
/// name for one — [`display::Run`]'s `style`.
pub use reovim_sdk::look;

/// A cursor drawn over the cell it is on.
pub const BLOCK: u64 = 0;
/// A cursor drawn as a bar before the cell it is on, which is where a
/// char begins however many cells it takes.
pub const BAR: u64 = 1;

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

/// The cell [`display::Screen`]'s own cursor stands at, in its
/// `row`'s own text: the same width [`width`] counts every char in.
///
/// `row` past `lines` is 0, there being no text to stand in; `col`
/// past its text is its end. A `col` on no char's own boundary — a
/// byte a corrupted or a stale frame named, and not one the drawer
/// ever meant — falls back to the nearest boundary below it and not
/// above: the boundary above may open a wholly different char, and
/// counting into one `col` never pointed at overstates where it
/// stands.
#[must_use]
pub fn column(screen: &display::Screen) -> usize {
    let row = usize::try_from(screen.row).unwrap_or(usize::MAX);
    let Some(line) = screen.lines.get(row) else { return 0 };
    let mut at =
        usize::try_from(screen.col).unwrap_or(usize::MAX).min(line.text.len());
    while !line.text.is_char_boundary(at) {
        at -= 1;
    }
    line.text[..at].chars().map(width).sum()
}

/// Where the named keys start, past the last scalar a char can be.
const NAMED: u64 = 0x11_0000;
/// Where Control and a char start, past the named keys.
const CTRL: u64 = 0x12_0000;
/// Where Alt and a char start, past Control and a char: the first
/// code past `CTRL + 0x10_ffff`. No code a build before this one
/// wrote changes meaning.
const ALT: u64 = 0x23_0000;

/// One key as the terminal gave it, already decoded by the host.
///
/// Raw keys never cross the wire: what does is the code of
/// [`Self::code`], which is what a write of the keyboard carries.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Key {
    Char(char),
    Enter,
    Tab,
    Backspace,
    Left,
    Right,
    Up,
    Down,
    Home,
    End,
    /// Control and a letter, lower case.
    Ctrl(char),
    Esc,
    /// Alt and a char, as typed: a terminal tells `<M-x>` from
    /// `<M-X>` as it cannot for Control.
    Alt(char),
}

impl Key {
    /// The key as [`keyboard::Press`] carries it.
    ///
    /// A char is its scalar value, a named key stands above every
    /// char, and Control and a char above those.
    #[must_use]
    pub fn code(self) -> u64 {
        match self {
            Self::Char(c) => u64::from(c),
            Self::Enter => NAMED,
            Self::Tab => NAMED + 1,
            Self::Backspace => NAMED + 2,
            Self::Left => NAMED + 3,
            Self::Right => NAMED + 4,
            Self::Up => NAMED + 5,
            Self::Down => NAMED + 6,
            Self::Home => NAMED + 7,
            Self::End => NAMED + 8,
            Self::Esc => NAMED + 9,
            Self::Ctrl(c) => CTRL + u64::from(c),
            Self::Alt(c) => ALT + u64::from(c),
        }
    }

    /// The key `code` stands for, or nothing for a code that is none.
    #[must_use]
    pub fn of(code: u64) -> Option<Self> {
        let scalar = |base: u64| {
            u32::try_from(code - base).ok().and_then(char::from_u32)
        };
        if code >= ALT {
            return scalar(ALT).map(Self::Alt);
        }
        if code >= CTRL {
            return scalar(CTRL).map(Self::Ctrl);
        }
        if code < NAMED {
            return scalar(0).map(Self::Char);
        }
        Some(match code - NAMED {
            0 => Self::Enter,
            1 => Self::Tab,
            2 => Self::Backspace,
            3 => Self::Left,
            4 => Self::Right,
            5 => Self::Up,
            6 => Self::Down,
            7 => Self::Home,
            8 => Self::End,
            9 => Self::Esc,
            _ => return None,
        })
    }
}

/// Up to `len` bytes of `bytes` from `at`, which is how a Plugin's own
/// cell answers a read; the Kernel's, so it is one and not two.
pub use reovim_kernel_interface::part;

#[cfg(test)]
mod tests {
    use alloc::string::{String, ToString};
    use alloc::vec;

    use super::notation::keys;
    use super::{Key, column, display};

    /// Every key there is, in the order `Key` declares them.
    fn every() -> alloc::vec::Vec<Key> {
        let named = [
            Key::Enter,
            Key::Tab,
            Key::Backspace,
            Key::Left,
            Key::Right,
            Key::Up,
            Key::Down,
            Key::Home,
            Key::End,
            Key::Esc,
        ];
        let chars = ['a', '0', ' ', 'é', '한', '\u{10ffff}'];
        let mut keys: alloc::vec::Vec<Key> = named.into();
        keys.extend(chars.map(Key::Char));
        keys.extend(chars.map(Key::Ctrl));
        keys.extend(chars.map(Key::Alt));
        keys
    }

    #[test]
    fn a_key_is_its_code_and_back() {
        for key in every() {
            assert_eq!(Key::of(key.code()), Some(key), "{key:?}");
        }
        assert_eq!(Key::of(u64::from('x')), Some(Key::Char('x')));
        assert_eq!(Key::of(0x11_000a), None, "no such named key");
        assert_eq!(Key::of(0xd800), None, "not a char");
        assert_eq!(Key::of(u64::MAX), None, "not a char either");
    }

    #[test]
    fn notation_names_the_keys_vim_names() {
        let parsed =
            keys("a<Esc>:wq<CR><C-r><BS><lt>é<M-x><M-X><Space>").unwrap();
        let expected = [
            Key::Char('a'),
            Key::Esc,
            Key::Char(':'),
            Key::Char('w'),
            Key::Char('q'),
            Key::Enter,
            Key::Ctrl('r'),
            Key::Backspace,
            Key::Char('<'),
            Key::Char('é'),
            Key::Alt('x'),
            Key::Alt('X'),
            Key::Char(' '),
        ];
        assert_eq!(parsed, expected);
        assert_eq!(keys("<Nope>"), None);
        assert_eq!(keys("<C-"), None);
        assert_eq!(keys("<M->"), None);
        assert_eq!(keys("<M-xy>"), None);
        assert_eq!(keys("<A-x>"), None);
    }

    /// A `col` inside a multi-byte char (a byte no drawer ever named,
    /// a stale or a corrupted frame's own) does not panic on the
    /// slice, and falls back to that char's own start: `'한'` is bytes
    /// 1..4 of "a한b", so byte 2 (inside it) counts no further than
    /// `'a'` alone.
    #[test]
    fn column_inside_a_multibyte_char_does_not_panic() {
        let line = display::Line { text: "a한b".to_string(), runs: vec![] };
        let screen = display::Screen {
            rows: 1,
            row: 0,
            col: 2,
            shape: 0,
            lines: vec![line],
            status: String::new(),
        };
        assert_eq!(column(&screen), 1);
    }
}
