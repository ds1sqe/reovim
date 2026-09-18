//! The capabilities a Client serves, as declared apis.
//!
//! A capability is an api the Client answers itself, called through
//! the same wrapper a Module's api is called through and identified by
//! the same hash. The Client says what it provides; a Plugin opens one
//! by its hash and gets a handle or nothing, so a Plugin that wants
//! what this Client has not got simply does without.
//!
//! Inputs go the other way, as `(Hash, bytes)`: one of the api's
//! records, encoded. A key is a `keys::Press`, a resize a
//! `grid::Resized`.

use alloc::string::String;
use alloc::vec::Vec;

use reovim_kernel_interface::{Error, Handle, Hash, Rd};
use reovim_sdk::{decode, encode};

use crate::Key;

reovim_sdk::declare! {
    /// What a person did with the input the Client has. It declares no
    /// method: presses go to the Plugin as inputs.
    api keys ver 1 {}
    record Press { key: u64 }
}

reovim_sdk::declare! {
    /// A region of a grid the Client draws: the Plugin asks how big it
    /// is and shows what belongs in it. `Resized` arrives as an input
    /// when it has become another size.
    ///
    /// A `View` names the rd it is in the bytes of and is otherwise in
    /// those bytes: the first byte shown, where the cursor is and what
    /// shape it takes, styled spans, and the Plugin's own word for its
    /// region. How that becomes a picture is the Client's, which reads
    /// `over` for the text to lay the View out on.
    api grid ver 1 {
        0 size() -> Size: READ;
        1 show(view: View) -> (): WRITE;
    }
    record Size { cols: u64, rows: u64 }
    record View { over: u64, top: u64, cursor: u64, shape: u64,
                  spans: [Span; 4096], status: utf8(4096) }
    record Span { from: u64, to: u64, style: u64 }
    record Resized { cols: u64, rows: u64 }
}

reovim_sdk::declare! {
    /// The region itself: a Plugin asks its Client to show `target`
    /// here, in the Plugin `plugin` names, or, with an empty name, in
    /// the one the Client's table gives that rd's shape.
    ///
    /// The call answers nothing, the asking Plugin being on the stack:
    /// the Client records the ask and acts on it once the call it is
    /// inside has returned, which is when the Plugin that made it may
    /// be dropped. A Client with no Plugin for the ask leaves the
    /// region as it is and says so in the log.
    ///
    /// `back` is the other way: the region shows again what it showed
    /// before the last `open`, in a Plugin made afresh. A region that
    /// has taken no step yet has nothing to go back to, and the
    /// Client ends there.
    api open ver 1 {
        0 open(ask: Ask) -> (): WRITE;
        1 back() -> (): WRITE;
    }
    record Ask { target: u64, plugin: utf8(64) }
}

reovim_sdk::declare! {
    /// The program the Client is: a Plugin ends it with the code the
    /// caller gets.
    api process ver 1 {
        0 exit(code: u64) -> (): WRITE;
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

/// Where the named keys start, past the last scalar a char can be.
const NAMED: u64 = 0x11_0000;
/// Where Control and a char start, past the named keys.
const CTRL: u64 = 0x12_0000;

impl Key {
    /// The key as `keys::Press` carries it: a char as its scalar
    /// value, a named key above every char, Control and a char above
    /// those.
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

/// What a Client serves and what its Plugin's calls left behind.
///
/// One entry is one region: the handle names it, so a Client with two
/// grid regions serves `grid` twice and each Plugin holds its own
/// handle. This one holds the state of the four capabilities that
/// land with it.
#[derive(Debug, Default)]
pub struct Caps {
    /// What the Client serves, and the handle it gives for each.
    served: Vec<(Hash, Handle)>,
    cols: u64,
    rows: u64,
    /// The last `grid::show`, and whether it arrived since the Client
    /// last asked.
    view: Option<grid::View>,
    shown: bool,
    /// What `open::open` asked to be shown here, until it is taken.
    asked: Option<(Rd, String)>,
    /// Whether `open::back` was called, until it is taken.
    back: bool,
    /// The code `process::exit` gave.
    exit: Option<u8>,
}

impl Caps {
    /// The Client serves `api`; the handle a Plugin opening it gets.
    pub fn serve(&mut self, api: Hash) -> Handle {
        let h = Handle(self.served.len() as u64 + 1);
        self.served.push((api, h));
        h
    }

    /// The handle for `api`, or nothing when this Client has not got
    /// it.
    #[must_use]
    pub fn open(&self, api: Hash) -> Option<Handle> {
        self.served.iter().find(|(a, _)| *a == api).map(|(_, h)| *h)
    }

    /// The region is `cols` by `rows`, as `grid::size` answers.
    pub const fn sized(&mut self, cols: u64, rows: u64) {
        (self.cols, self.rows) = (cols, rows);
    }

    /// The last View the Plugin showed, if it has shown one.
    #[must_use]
    pub const fn view(&self) -> Option<&grid::View> {
        self.view.as_ref()
    }

    /// Whether a `show` arrived since this was last asked.
    pub const fn shown(&mut self) -> bool {
        core::mem::replace(&mut self.shown, false)
    }

    /// What a Plugin asked to be shown in this region, and in which
    /// Plugin by name, once: the Client acts on it after the call it
    /// was asked inside, so it is handed over and forgotten.
    pub const fn asked(&mut self) -> Option<(Rd, String)> {
        self.asked.take()
    }

    /// Whether a Plugin asked for the step back, once: the Client acts
    /// on it where it acts on an ask, and forgets it the same way.
    pub const fn backed(&mut self) -> bool {
        core::mem::replace(&mut self.back, false)
    }

    /// The code a Plugin asked to end with.
    #[must_use]
    pub const fn exit(&self) -> Option<u8> {
        self.exit
    }

    /// `grid` on the one region: its size, or the View it is shown.
    fn grid(&mut self, number: u16, input: &[u8]) -> Result<Vec<u8>, Error> {
        match number {
            0 => Ok(encode(&grid::Size { cols: self.cols, rows: self.rows })),
            1 => {
                self.view = Some(decode(input)?);
                self.shown = true;
                Ok(Vec::new())
            }
            _ => Err(Error::Unsupported),
        }
    }

    /// `open`: the rd to show here and the Plugin to show it in, or
    /// the step back, recorded for the Client to act on.
    fn opening(&mut self, number: u16, input: &[u8]) -> Result<Vec<u8>, Error> {
        match number {
            0 => {
                let ask: open::Ask = decode(input)?;
                self.asked = Some((Rd(ask.target), ask.plugin));
                Ok(Vec::new())
            }
            1 => {
                self.back = true;
                Ok(Vec::new())
            }
            _ => Err(Error::Unsupported),
        }
    }

    /// `process`: the code to end with.
    fn process(&mut self, number: u16, input: &[u8]) -> Result<Vec<u8>, Error> {
        if number != 0 {
            return Err(Error::Unsupported);
        }
        let code: u64 = decode(input)?;
        self.exit = Some(u8::try_from(code).unwrap_or(u8::MAX));
        Ok(Vec::new())
    }
}

/// The Client answers its own capabilities, so a Plugin calls one the
/// way it calls a Module: by hash and method number, on the handle the
/// table gave it.
impl reovim_sdk::Call for Caps {
    type Error = Error;

    fn call(
        &mut self,
        h: Handle,
        api: Hash,
        number: u16,
        input: Vec<u8>,
    ) -> Result<Vec<u8>, Error> {
        if self.open(api) != Some(h) {
            return Err(Error::Denied);
        }
        if api == grid::hash() {
            return self.grid(number, &input);
        }
        if api == open::hash() {
            return self.opening(number, &input);
        }
        if api == process::hash() {
            return self.process(number, &input);
        }
        Err(Error::Unsupported)
    }
}

#[cfg(test)]
mod tests {
    use alloc::string::String;
    use alloc::vec;

    use super::{Caps, grid, keys, open, process};
    use crate::{Error, Key, Rd};

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

    #[test]
    fn a_plugin_opens_what_the_client_serves() {
        let mut caps = Caps::default();
        let grid = caps.serve(grid::hash());
        assert_eq!(caps.open(grid::hash()), Some(grid));
        assert_eq!(caps.open(keys::hash()), None, "not served");
        assert_eq!(caps.open(process::hash()), None, "not served either");
    }

    #[test]
    fn the_client_answers_its_own_capabilities() {
        let mut caps = Caps::default();
        let grid = caps.serve(grid::hash());
        let process = caps.serve(process::hash());
        caps.sized(80, 23);
        assert_eq!(
            grid::Call(&mut caps, grid).size(),
            Ok(grid::Size { cols: 80, rows: 23 })
        );
        assert!(!caps.shown(), "nothing shown yet");
        let view = grid::View {
            over: 12,
            top: 0,
            cursor: 3,
            shape: super::BAR,
            spans: vec![grid::Span { from: 0, to: 2, style: super::SELECTION }],
            status: "-- VISUAL --".into(),
        };
        assert_eq!(grid::Call(&mut caps, grid).show(view.clone()), Ok(()));
        assert!(caps.shown(), "and once only");
        assert!(!caps.shown());
        assert_eq!(caps.view(), Some(&view));
        assert_eq!(caps.view().map(|v| v.over), Some(12), "the rd it is in");
        assert_eq!(caps.exit(), None);
        assert_eq!(process::Call(&mut caps, process).exit(3), Ok(()));
        assert_eq!(caps.exit(), Some(3));
        let wrong = grid::Call(&mut caps, process).size();
        assert_eq!(wrong, Err(Error::Denied));
    }

    #[test]
    fn what_a_plugin_asked_to_be_shown_is_handed_over_once() {
        let mut caps = Caps::default();
        let opening = caps.serve(open::hash());
        assert_eq!(caps.asked(), None, "nothing asked yet");
        let ask = open::Ask { target: 7, plugin: "vim".into() };
        assert_eq!(open::Call(&mut caps, opening).open(ask), Ok(()));
        assert_eq!(caps.asked(), Some((Rd(7), "vim".into())));
        assert_eq!(caps.asked(), None, "the Client acts on it once");

        // An empty name is the ask that leaves the Plugin to the
        // Client's table; the last ask is the one that stands.
        for target in [8, 9] {
            let ask = open::Ask { target, plugin: String::new() };
            assert_eq!(open::Call(&mut caps, opening).open(ask), Ok(()));
        }
        assert_eq!(caps.asked(), Some((Rd(9), String::new())));
    }

    #[test]
    fn the_step_back_is_handed_over_once_too() {
        let mut caps = Caps::default();
        let opening = caps.serve(open::hash());
        assert!(!caps.backed(), "nothing asked yet");
        assert_eq!(open::Call(&mut caps, opening).back(), Ok(()));
        assert!(caps.backed(), "the Client acts on it once");
        assert!(!caps.backed());
    }
}
