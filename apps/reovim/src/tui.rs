//! `reovim tui FILE` over the link, and bare `reovim FILE` with the
//! server in the same process: the terminal loop, drawing in plain
//! ANSI. Keys go to the tui, the frame comes back.

use std::fmt::Write as _;
use std::io::Write;

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

/// `reovim tui`: a Client over the link, named so a cut link can be
/// taken up again.
pub fn remote(
    socket: &str,
    line: Option<usize>,
    file: &str,
) -> Result<u8, Fail> {
    let credential = format!("tui:{}", std::process::id()).into_bytes();
    let client = Remote::client(socket, credential)?;
    run(client, line, file, &mut || Ok(()))
}

/// Bare `reovim`: the server on this thread over `/`, listening only
/// at a `socket` given, served between keys.
pub fn bare(
    socket: Option<&str>,
    log: Option<&str>,
    line: Option<usize>,
    file: &str,
) -> Result<u8, Fail> {
    let (kernel, _modules) = boot("/")?;
    let mut links = socket.map(Links::listen).transpose()?;
    let mut sink = log.map(|path| Sink::open(&kernel, path)).transpose()?;
    let file = absolute(&reovim_host::dirs::work()?, file);
    let client = Local::client(&kernel)?;
    run(client, line, &file, &mut || {
        if let Some(links) = &mut links {
            links.drain(&kernel);
        }
        sink.as_mut().map_or(Ok(()), Sink::step)
    })
}

/// `path` under `work` unless it is absolute, `.` and `..` resolved
/// lexically, without the leading slash: its name under the fs root.
fn absolute(work: &str, path: &str) -> String {
    let joined = if path.starts_with('/') {
        path.to_string()
    } else {
        format!("{work}/{path}")
    };
    let mut parts: Vec<&str> = Vec::new();
    for part in joined.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop();
            }
            part => parts.push(part),
        }
    }
    parts.join("/")
}

/// The loop in the host's terminal mode, left on every exit, a fault
/// printed after; the exit code the tui asked for.
fn run(
    client: Client,
    line: Option<usize>,
    file: &str,
    serve: Serve,
) -> Result<u8, Fail> {
    let mut tui = Tui::open(client, file)?;
    if let Some(line) = line {
        tui.goto_line(line);
    }
    term::enter()?;
    let result = drive(&mut tui, serve);
    let _ = term::leave();
    result
}

fn drive(tui: &mut Tui, serve: Serve) -> Result<u8, Fail> {
    draw(tui)?;
    loop {
        serve()?;
        let step = match term::poll(20)? {
            Some(key) => tui.key(map(key))?,
            None if term::stopped() => return Err(Fail::Signal),
            None => tui.tick()?,
        };
        match step {
            Step::Same => {}
            Step::Changed => draw(tui)?,
            Step::Quit(code) => return Ok(code),
        }
    }
}

/// The whole frame: text rows, the status row inverted, the cursor
/// hidden while it is drawn and placed after.
fn draw(tui: &mut Tui) -> Result<(), Fail> {
    let (cols, rows) = term::size()?;
    let Frame { lines, cursor, status } = tui.frame(cols, rows);
    let text_rows = usize::from(rows.saturating_sub(1));
    let blank = String::new();
    let rest =
        std::iter::repeat_n(&blank, text_rows.saturating_sub(lines.len()));
    let mut screen = String::from("\x1b[?25l\x1b[H");
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

#[cfg(test)]
mod tests {
    use super::absolute;

    #[test]
    fn absolute_names_the_file_under_the_root() {
        assert_eq!(absolute("/home/me", "note"), "home/me/note");
        assert_eq!(absolute("/home/me", "./a/../b/"), "home/me/b");
        assert_eq!(absolute("/home/me", "/tmp/x"), "tmp/x");
        assert_eq!(absolute("/", "../../etc/hosts"), "etc/hosts");
        assert_eq!(absolute("/home/me", "/"), "");
    }
}
