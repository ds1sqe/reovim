//! Sans-IO ANSI/VT escape-sequence parser for the framebuffer console.
//!
//! The console is fed a raw byte stream that interleaves printable text with
//! ANSI/VT control sequences (`ESC [ … m` for SGR color changes). This module
//! is the *sans-IO* state machine that separates the two: it is fed one byte
//! at a time through [`Parser::advance`] and yields a typed [`Action`] — print
//! this byte, execute this C0 control, or apply this SGR — without ever
//! touching the framebuffer or the UART. The console (the caller) owns all I/O
//! and decides what each action means; the mapping from SGR parameters to the
//! color pen is *policy* that lives in the console, not here. Keeping the
//! parser I/O-free is what makes it exhaustively testable against an action
//! stream and keeps the mechanism/policy split on the module boundary.
//!
//! Scope is the subset the boot console needs. CSI SGR (`… m`) sequences are
//! decoded into their numeric parameters; every other CSI final byte (cursor
//! moves, erases) is recognized and consumed as a no-op, since the console has
//! no cursor addressing yet, and a non-`[` escape is dropped. Critically, the
//! parser never leaks a control byte into the printable stream — an
//! unsupported or malformed CSI is swallowed, not rendered as stray text.

/// Maximum number of CSI numeric parameters retained from one sequence. The
/// widest SGR the console acts on is `38;2;r;g;b` (five), so sixteen is ample;
/// parameters past this many are parsed and discarded rather than overflowing.
const MAX_PARAMS: usize = 16;

/// The numeric parameters collected from one CSI sequence, owned by value.
///
/// Carrying the parameters as a fixed array plus a count, rather than
/// borrowing the parser's buffer, keeps [`Action`] `Copy` and lifetime-free —
/// so [`Parser::advance`]'s return type never ties the caller to a borrow of
/// the parser. The buffer is reset when each sequence begins, so the entries
/// past [`as_slice`](SgrParams::as_slice)'s length carry no stale data.
#[derive(Clone, Copy)]
pub struct SgrParams {
    values: [u16; MAX_PARAMS],
    len: usize,
}

impl SgrParams {
    /// An empty parameter set — the initial state of every fresh sequence.
    const fn empty() -> Self {
        Self {
            values: [0; MAX_PARAMS],
            len: 0,
        }
    }

    /// The collected parameters, in order. An empty slice means the sequence
    /// carried no parameters (`ESC [ m`), which SGR treats as a single `0`.
    #[must_use]
    pub fn as_slice(&self) -> &[u16] {
        // `len` is only ever incremented while strictly below `MAX_PARAMS`
        // (see `separate`), so the range is always in bounds.
        &self.values[..self.len]
    }
}

/// The parser's position within an escape sequence. Defined by the bytes seen
/// so far, not by what they mean — meaning is the console's job.
enum State {
    /// Outside any sequence: bytes are printable text or C0 controls.
    Ground,
    /// Saw `ESC`: the next byte selects the sequence kind (only `[` → CSI is
    /// handled; anything else is dropped).
    Escape,
    /// Inside `ESC [`, collecting numeric parameters until a final byte.
    Csi,
    /// Inside a CSI that turned out to be unsupported (an intermediate or
    /// private-marker byte appeared): consume bytes until the final one, then
    /// drop the whole sequence so none of it leaks as text.
    CsiIgnore,
}

/// One thing the console should do for a consumed byte. Owned and `Copy`, so a
/// caller stores or matches it without borrowing the parser.
#[derive(Clone, Copy)]
pub enum Action {
    /// Render this byte as a glyph cell (printable bytes and, deliberately,
    /// any unrecognized control byte — the console renders those blank).
    Print(u8),
    /// `\n`: advance to the next line.
    LineFeed,
    /// `\r`: return to column zero.
    CarriageReturn,
    /// A complete `ESC [ … m`: apply these SGR parameters to the pen.
    Sgr(SgrParams),
}

/// A sans-IO ANSI/VT escape-sequence parser. Fed one byte at a time via
/// [`advance`](Parser::advance); see the module docs for scope.
pub struct Parser {
    state: State,
    /// Parameters of the CSI sequence currently being collected.
    params: SgrParams,
    /// Set once a CSI has more parameters than [`MAX_PARAMS`]; further
    /// parameter bytes are then dropped rather than corrupting the last slot.
    overflow: bool,
}

impl Default for Parser {
    fn default() -> Self {
        Self::new()
    }
}

impl Parser {
    /// A parser in the ground state, ready for the first byte.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            state: State::Ground,
            params: SgrParams::empty(),
            overflow: false,
        }
    }

    /// Consumes one byte and returns the [`Action`] it completes, if any.
    /// Bytes that only advance an in-progress sequence (the `ESC`, the `[`, each
    /// parameter digit) return `None`; the action surfaces on the byte that
    /// finishes the unit.
    pub fn advance(&mut self, byte: u8) -> Option<Action> {
        match self.state {
            State::Ground => self.ground(byte),
            State::Escape => {
                self.escape(byte);
                None
            }
            State::Csi => self.csi(byte),
            State::CsiIgnore => {
                self.csi_ignore(byte);
                None
            }
        }
    }

    /// Ground state: `ESC` opens a sequence, `\n`/`\r` are C0 controls, every
    /// other byte is printable text.
    const fn ground(&mut self, byte: u8) -> Option<Action> {
        match byte {
            0x1B => {
                self.state = State::Escape;
                None
            }
            b'\n' => Some(Action::LineFeed),
            b'\r' => Some(Action::CarriageReturn),
            _ => Some(Action::Print(byte)),
        }
    }

    /// Just saw `ESC`: `[` begins a CSI; a second `ESC` restarts the sequence;
    /// any other byte is a non-CSI escape we do not handle, so it is dropped
    /// and we fall back to ground (never rendered as text).
    const fn escape(&mut self, byte: u8) {
        match byte {
            b'[' => {
                self.params = SgrParams::empty();
                self.overflow = false;
                self.state = State::Csi;
            }
            0x1B => {}
            _ => self.state = State::Ground,
        }
    }

    /// Collecting a CSI: digits accumulate, `;` starts the next parameter, a
    /// final byte (`0x40..=0x7E`) ends the sequence — `m` yields the SGR, any
    /// other final is a recognized-but-unsupported control consumed silently.
    /// An intermediate or private-marker byte diverts to [`State::CsiIgnore`].
    fn csi(&mut self, byte: u8) -> Option<Action> {
        match byte {
            b'0'..=b'9' => {
                self.digit(u16::from(byte - b'0'));
                None
            }
            b';' => {
                self.separate();
                None
            }
            0x40..=0x7E => {
                self.state = State::Ground;
                if byte == b'm' {
                    Some(Action::Sgr(self.params))
                } else {
                    None
                }
            }
            _ => {
                self.state = State::CsiIgnore;
                None
            }
        }
    }

    /// Inside an unsupported CSI: swallow bytes until the final one, then drop
    /// the sequence whole.
    const fn csi_ignore(&mut self, byte: u8) {
        if matches!(byte, 0x40..=0x7E) {
            self.state = State::Ground;
        }
    }

    /// Folds a decimal digit into the current parameter, saturating so a
    /// pathologically long run cannot overflow `u16` or panic.
    const fn digit(&mut self, d: u16) {
        if self.overflow {
            return;
        }
        if self.params.len == 0 {
            self.params.len = 1;
        }
        let slot = &mut self.params.values[self.params.len - 1];
        *slot = slot.saturating_mul(10).saturating_add(d);
    }

    /// Ends the current parameter and opens the next. An empty leading
    /// parameter (`ESC [ ; …`) still counts as a zero. Past [`MAX_PARAMS`] the
    /// count stops growing and further parameters are dropped.
    const fn separate(&mut self) {
        if self.params.len == 0 {
            self.params.len = 1;
        }
        if self.params.len < MAX_PARAMS {
            self.params.len += 1;
        } else {
            self.overflow = true;
        }
    }
}

#[cfg(feature = "selftest")]
#[path = "escape_tests.rs"]
mod escape_tests;
