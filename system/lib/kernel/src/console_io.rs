//! Bounded line input policy for the root console shell.
//!
//! Raw input mechanism stays below the bridge. This module owns only the common
//! shell-facing line discipline over injected byte-source and byte-sink
//! callbacks: collect bytes until enter, echo printable bytes, and handle
//! backspace/delete plus common line-cancel controls in a terminal-shaped way.

/// Callback that returns the next input byte, or `None` on EOF/error.
pub type ReadByte = fn() -> Option<u8>;

/// Callback that writes output bytes to the active console.
pub type WriteBytes = fn(&[u8]);

const ERASE: &[u8] = b"\x08 \x08";
const CTRL_C: u8 = 0x03;
const CTRL_U: u8 = 0x15;

enum InputEscapeState {
    Ground,
    Escape,
    Csi,
    Ss3,
}

/// Reads one edited command line into `line`.
///
/// The returned length excludes the line ending. A return of `0` means either
/// EOF before any byte or an empty line. The caller decides whether that should
/// terminate the daemon or just prompt again.
pub fn read_line(line: &mut [u8], read_byte: ReadByte, write: WriteBytes) -> usize {
    let mut len = 0usize;
    let mut escape = InputEscapeState::Ground;

    loop {
        let Some(byte) = read_byte() else {
            return len;
        };

        match escape {
            InputEscapeState::Ground => {}
            InputEscapeState::Escape => {
                escape = if byte == b'[' {
                    InputEscapeState::Csi
                } else if byte == b'O' {
                    InputEscapeState::Ss3
                } else if byte == 0x1b {
                    InputEscapeState::Escape
                } else {
                    InputEscapeState::Ground
                };
                continue;
            }
            InputEscapeState::Csi => {
                if matches!(byte, 0x40..=0x7e) {
                    escape = InputEscapeState::Ground;
                }
                continue;
            }
            InputEscapeState::Ss3 => {
                escape = InputEscapeState::Ground;
                continue;
            }
        }

        match byte {
            0x1b => {
                escape = InputEscapeState::Escape;
            }
            CTRL_C => {
                write(b"^C\n");
                if !line.is_empty() {
                    line[0] = b'\n';
                    return 1;
                }
                return 0;
            }
            CTRL_U => {
                while len > 0 {
                    len -= 1;
                    write(ERASE);
                }
            }
            b'\r' | b'\n' => {
                write(b"\n");
                if len == 0 && !line.is_empty() {
                    line[0] = b'\n';
                    return 1;
                }
                return len;
            }
            0x08 | 0x7f => {
                if len > 0 {
                    len -= 1;
                    write(ERASE);
                }
            }
            b'\t' => {
                if len < line.len() {
                    line[len] = b' ';
                    len += 1;
                    write(b" ");
                }
            }
            0x00..=0x1f => {}
            byte => {
                if len >= line.len() {
                    write(b"\n");
                    return len;
                }
                line[len] = byte;
                len += 1;
                write(&[byte]);
            }
        }
    }
}

#[cfg(feature = "selftest")]
#[path = "console_io_tests.rs"]
mod tests;
