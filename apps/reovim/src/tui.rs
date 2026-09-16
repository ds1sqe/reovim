//! `reovim tui FILE` over the link, and bare `reovim FILE` with the
//! server in the same process: the terminal loop, drawing in plain
//! ANSI. Keys go to the tui, the frame comes back.

use std::fmt::Write as _;
use std::io::Write;
use std::time::Duration;

use reovim_client::{Client, Fault, Key};
use reovim_client_tui::{Frame, Step, Tui};
use reovim_host::term;

use crate::Fail;
use crate::local::Local;
use crate::log::Sink;
use crate::remote::Remote;
use crate::server::{Links, boot};

/// What runs between keys: nothing over the link, the server's step in
/// the bare form.
type Serve<'a> = &'a mut dyn FnMut() -> Result<(), Fail>;

/// `reovim tui`: a Client over the link.
pub fn remote(socket: &str, file: &str) -> Result<(), Fail> {
    let client = Remote::client(socket)?;
    run(client, file, &mut || Ok(()))
}

/// Bare `reovim`: the server on this thread, served between keys.
pub fn bare(
    socket: &str,
    root: &str,
    log: Option<&str>,
    file: &str,
) -> Result<(), Fail> {
    let (kernel, _modules) = boot(root)?;
    let mut links = Links::listen(socket)?;
    let mut sink = log.map(|path| Sink::open(&kernel, path)).transpose()?;
    let client = Local::client(&kernel)?;
    run(client, file, &mut || {
        while links.step(&kernel, Some(Duration::ZERO)) {}
        sink.as_mut().map_or(Ok(()), Sink::step)
    })
}

/// The loop under raw mode on the alternate screen; both undone on
/// every exit, a fault printed after.
fn run(client: Client, file: &str, serve: Serve) -> Result<(), Fail> {
    let mut tui = Tui::open(client, file)?;
    term::enter()?;
    let result = out(b"\x1b[?1049h\x1b[?25l")
        .map_err(Fail::from)
        .and_then(|()| drive(&mut tui, serve));
    let _ = out(b"\x1b[?25h\x1b[?1049l");
    let _ = term::leave();
    result
}

fn drive(tui: &mut Tui, serve: Serve) -> Result<(), Fail> {
    draw(tui)?;
    loop {
        serve()?;
        let step = match term::poll(20)? {
            Some(key) => tui.key(map(key))?,
            None => tui.tick()?,
        };
        match step {
            Step::Same => {}
            Step::Changed => draw(tui)?,
            Step::Quit => return Ok(()),
        }
    }
}

/// The whole frame: text rows, the status row inverted, the cursor.
fn draw(tui: &mut Tui) -> Result<(), Fail> {
    let (cols, rows) = term::size()?;
    let Frame { lines, cursor, status } = tui.frame(cols, rows);
    let text_rows = usize::from(rows.saturating_sub(1));
    let blank = String::new();
    let rest =
        std::iter::repeat_n(&blank, text_rows.saturating_sub(lines.len()));
    let mut screen = String::from("\x1b[H");
    for line in lines.iter().chain(rest) {
        let _ = write!(screen, "\x1b[2K{line}\r\n");
    }
    let status: String = status.chars().take(usize::from(cols)).collect();
    let _ = write!(screen, "\x1b[2K\x1b[7m{status}\x1b[0m");
    let (row, col) = (cursor.0 + 1, cursor.1 + 1);
    let _ = write!(screen, "\x1b[{row};{col}H\x1b[?25h");
    Ok(out(screen.as_bytes())?)
}

fn out(bytes: &[u8]) -> Result<(), Fault> {
    let mut stdout = std::io::stdout();
    stdout
        .write_all(bytes)
        .and_then(|()| stdout.flush())
        .map_err(|_| Fault::Carrier)
}

const fn map(key: term::Key) -> Key {
    match key {
        term::Key::Char(c) => Key::Char(c),
        term::Key::Enter => Key::Enter,
        term::Key::Tab => Key::Tab,
        term::Key::Backspace => Key::Backspace,
        term::Key::Left => Key::Left,
        term::Key::Right => Key::Right,
        term::Key::Up => Key::Up,
        term::Key::Down => Key::Down,
        term::Key::Home => Key::Home,
        term::Key::End => Key::End,
        term::Key::Ctrl(c) => Key::Ctrl(c),
        term::Key::Esc => Key::Esc,
    }
}
