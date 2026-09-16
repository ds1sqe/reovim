//! The Plugin of 02-client.md: it lives in the Client, turns keys
//! into effects over the text it holds, and draws its own frame; it
//! sees no rd. The Client turns each effect into calls.

use alloc::string::String;
use alloc::vec::Vec;

use crate::Key;

/// One change to the text, already applied in the Plugin: `len` bytes
/// at `at` became `text`. With `join` the edit belongs to the head
/// revision, not a new one: one insert is one undo step.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Edit {
    pub at: usize,
    pub len: usize,
    pub text: String,
    pub join: bool,
}

/// What a key did.
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
    Save,
    /// `:w NAME`: the draft written to that file, which the Client
    /// resolves; a Buffer over nothing becomes the file's.
    SaveAs(String),
    Quit(Quit),
    /// A put wants the clipboard's text: `pasted` finishes it.
    Clipboard,
}

/// How a quit was asked for; what it does with unsaved changes is
/// the Client's to decide, since only it knows whether there are any.
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

/// What to draw, in screen cells.
///
/// `rows - 1` text rows as the terminal shows them (a tab already
/// the spaces to its stop, a wide char two cells), the cursor as
/// (row, cell) on them, the selected cells as `(row, from, to)` with
/// `to` exclusive, and the Plugin's own word for the status row: a
/// message, the command line, the mode, or nothing.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Frame {
    pub lines: Vec<String>,
    pub cursor: (u16, u16),
    pub selection: Vec<(u16, u16, u16)>,
    /// Search matches, in the selection's shape.
    pub marks: Vec<(u16, u16, u16)>,
    pub status: String,
}

/// Keys in, effects and frames out.
///
/// The Client feeds every key through here and every reload of the
/// text; it never reads the text back, as the Plugin's copy and the
/// Buffer agree while every edit is sent on.
pub trait Plugin {
    /// `bytes` as the text, after a reload: an undo, or a change by
    /// another Client. True when it differs from before.
    fn loaded(&mut self, bytes: &[u8]) -> bool;

    /// One key.
    fn key(&mut self, key: Key) -> Effect;

    /// The clipboard's answer to `Effect::Clipboard`; `None` when
    /// there is none.
    fn pasted(&mut self, text: Option<String>) -> Effect;

    /// Text a yank left for the clipboard, once.
    fn take_clipboard(&mut self) -> Option<String>;

    /// The Plugin's last word before a save: an edit to make first,
    /// if any (Vim's `fixendofline`).
    fn saving(&mut self) -> Option<Edit>;

    /// The cursor to line `n`, counted from 1 and clamped.
    fn goto_line(&mut self, n: usize);

    /// The cursor's line and column, in chars from 0.
    fn position(&self) -> (usize, usize);

    /// A message in the status row until the next key: the Client's
    /// own word, such as E37.
    fn notice(&mut self, message: &str);

    /// What to draw for `cols` by `rows`.
    fn frame(&mut self, cols: u16, rows: u16) -> Frame;
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
