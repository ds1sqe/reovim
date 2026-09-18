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

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use core::cell::RefCell;

use reovim_kernel_interface::{Calls, Error, Handle, Rd, Reply, Rights};
use reovim_sdk::walk::ROOT;
use reovim_sdk::{cell, declare, decode, ops};

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
    /// A write of the cell is one key code, eight bytes little endian;
    /// a read gives one `Press` and takes it for that handle alone.
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
    /// `/self/dev/open`: the region shows something else.
    ///
    /// A Plugin asks its Client to show `target` here, in the Plugin
    /// `plugin` names or, with an empty name, in the one the Client's
    /// table gives that rd's shape. `name` is the path a region over
    /// nothing was opened with, there being no such file, and `line`
    /// the line to begin at, counted from 1; 0 is none of either.
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
    record Ask { target: Rd, plugin: utf8(64), name: utf8(4096), line: u64 }
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
    /// revision boundary asks for one. The answer is whether the
    /// draft's bytes moved under the caller, which is the only thing
    /// it cannot see for itself: true where another run landed in
    /// front of this one, false otherwise.
    api draft ver 1 {
        0 sync() -> bool: WRITE;
    }
}

/// A span that is part of what the Plugin has selected.
pub const SELECTION: u64 = 1;
/// A span that matches what the Plugin last searched for.
pub const MATCH: u64 = 2;

/// A span drawn in bold blue, the first of the seven a span may be
/// colored: what one of these means is the color alone, and which
/// stands for what is the Plugin's.
pub const BLUE: u64 = 3;
/// A span drawn in green.
pub const GREEN: u64 = 4;
/// A span drawn in red.
pub const RED: u64 = 5;
/// A span drawn in bold red.
pub const RED_BOLD: u64 = 6;
/// A span drawn in bold.
pub const BOLD: u64 = 7;
/// A span drawn in yellow.
pub const YELLOW: u64 = 8;
/// A span drawn dim.
pub const DIM: u64 = 9;

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

/// Where the named keys start, past the last scalar a char can be.
const NAMED: u64 = 0x11_0000;
/// Where Control and a char start, past the named keys.
const CTRL: u64 = 0x12_0000;

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
        }
    }

    /// The key `code` stands for, or nothing for a code that is none.
    #[must_use]
    pub fn of(code: u64) -> Option<Self> {
        let scalar = |base: u64| {
            u32::try_from(code - base).ok().and_then(char::from_u32)
        };
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

#[cfg(test)]
mod tests {
    use super::Key;

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
}

/// Bytes asked for per read of a cell of `/self/dev`, each of which
/// holds one record or one short line of text.
const CHUNK: u32 = 64 * 1024;

/// The region a Plugin holds: the cells of `/self/dev` it reads and
/// writes, found once when the Client starts it.
///
/// A Plugin is handed `ctx` and its `home` and nothing else, so this
/// is how it reaches the Client's devices: what the region was opened
/// with, where the Client stands, what has been typed, how big the
/// region is, and the two cells that end this Plugin — `open`, which
/// gives the region away, and `exit`, which ends the Client.
#[derive(Debug)]
pub struct Region {
    /// The keyboard, opened at the end of the stream: keys typed at
    /// another Plugin are not replayed.
    keyboard: Handle,
    cwd: Rd,
    ask: Rd,
    size: Rd,
    open: Rd,
    exit: Rd,
}

impl Region {
    /// Every cell of `/self/dev`, resolved, and the keyboard opened.
    ///
    /// # Errors
    /// What the Client says; `Invalid` where there is no `/self/dev`,
    /// which is a Client this Plugin is not for.
    pub fn of(ctx: &dyn Calls) -> Result<Self, Error> {
        let at = |path: &str| cell::resolve(ctx, ROOT, path);
        Ok(Self {
            keyboard: ctx
                .open(at("/self/dev/input/keyboard")?, Rights::READ)?,
            cwd: at("/self/dev/cwd")?,
            ask: at("/self/dev/windows/0/ask")?,
            size: at("/self/dev/windows/0/size")?,
            open: at("/self/dev/open")?,
            exit: at("/self/dev/exit")?,
        })
    }

    /// What the region was opened with.
    ///
    /// # Errors
    /// What the read says; `Invalid` for bytes that are no `Ask`.
    pub fn ask(&self, ctx: &dyn Calls) -> Result<open::Ask, Error> {
        decode(&ops::bytes(ctx, self.ask)?)
    }

    /// Where the Client stands, under the root.
    ///
    /// # Errors
    /// What the read says.
    pub fn cwd(&self, ctx: &dyn Calls) -> Result<String, Error> {
        let held = ops::bytes(ctx, self.cwd)?;
        Ok(String::from_utf8_lossy(&held).into_owned())
    }

    /// How big the region is now.
    ///
    /// # Errors
    /// What the read says; `Invalid` for bytes that are no `Size`.
    pub fn size(&self, ctx: &dyn Calls) -> Result<window::Size, Error> {
        decode(&ops::bytes(ctx, self.size)?)
    }

    /// Every key this Plugin has not taken, in order: what one frame
    /// runs. A read with nothing to take is `Busy`, the stream not
    /// being over, so an empty answer is a frame with nothing typed.
    ///
    /// # Errors
    /// What the read says, `Busy` apart.
    pub fn keys(&self, ctx: &dyn Calls) -> Result<Vec<Key>, Error> {
        let mut keys = Vec::new();
        while let Some(press) = self.pressed(ctx)? {
            keys.extend(Key::of(press.key));
        }
        Ok(keys)
    }

    /// One press, or nothing where the stream has none for this
    /// handle: a read with nothing to take is `Busy`.
    fn pressed(
        &self,
        ctx: &dyn Calls,
    ) -> Result<Option<keyboard::Press>, Error> {
        match ops::read(ctx, self.keyboard, 0, CHUNK) {
            Ok(bytes) => decode(&bytes).map(Some),
            Err(Error::Busy) => Ok(None),
            Err(error) => Err(error),
        }
    }

    /// The Client asked to show `target` here, in the Plugin `plugin`
    /// names or, with an empty name, in the one its table gives that
    /// rd's shape. This is the end of this Plugin, once the call it
    /// was asked inside has returned.
    ///
    /// # Errors
    /// What the call says.
    pub fn opens(
        &self,
        mut ctx: &dyn Calls,
        target: Rd,
        plugin: &str,
    ) -> Result<(), Error> {
        let ask = open::Ask {
            target,
            plugin: String::from(plugin),
            name: String::new(),
            line: 0,
        };
        let h = ctx.open(self.open, Rights::WRITE)?;
        let asked = open::Call(&mut ctx, h).open(ask);
        ops::close(ctx, h)?;
        asked
    }

    /// The region back to what it showed before this Plugin.
    ///
    /// # Errors
    /// As [`Self::opens`].
    pub fn backs(&self, mut ctx: &dyn Calls) -> Result<(), Error> {
        let h = ctx.open(self.open, Rights::WRITE)?;
        let asked = open::Call(&mut ctx, h).back();
        ops::close(ctx, h)?;
        asked
    }

    /// The code the Client leaves with.
    ///
    /// # Errors
    /// What the write says.
    pub fn ends(&self, ctx: &dyn Calls, code: u8) -> Result<(), Error> {
        ops::put(ctx, self.exit, format!("{code}").into_bytes()).map(|_| ())
    }
}

/// The `view` cell a Plugin serves: what a whole read of it answers.
///
/// One whole read is one frame. The read at 0 runs the frame and keeps
/// what it answered; every read past those bytes serves their tail, so
/// the reader gets one View and then the empty read that ends it.
#[derive(Debug, Default)]
pub struct Frames(RefCell<Vec<u8>>);

impl Frames {
    /// The `Driver::read` of a `view` cell: `frame` at the head of a
    /// read, its bytes from `at`.
    pub fn read<F>(&self, at: u64, len: usize, frame: F) -> Reply<Vec<u8>>
    where
        F: FnOnce() -> Result<Vec<u8>, Error>,
    {
        self.reading(at, len, frame).into()
    }

    fn reading<F>(
        &self,
        at: u64,
        len: usize,
        frame: F,
    ) -> Result<Vec<u8>, Error>
    where
        F: FnOnce() -> Result<Vec<u8>, Error>,
    {
        if at == 0 {
            // The borrow is taken only once the frame has answered: a
            // frame reads cells of its own and may not find this one
            // held.
            let shown = frame()?;
            *self.0.borrow_mut() = shown;
        }
        Ok(part(&self.0.borrow(), at, len))
    }
}

/// Up to `len` bytes of `bytes` from `at`, or as many as there are:
/// what a cell answers a read at an offset with.
#[must_use]
pub fn part(bytes: &[u8], at: u64, len: usize) -> Vec<u8> {
    let start = usize::try_from(at).unwrap_or(usize::MAX).min(bytes.len());
    let end = start.saturating_add(len).min(bytes.len());
    bytes[start..end].to_vec()
}

/// A count the Client gave, as far as this machine counts.
#[must_use]
pub fn count(n: u64) -> usize {
    usize::try_from(n).unwrap_or(usize::MAX)
}
