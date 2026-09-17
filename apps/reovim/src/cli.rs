//! `reovim cli`: one link and one Client. One command runs and exits;
//! none reads commands until EOF or `quit`, as lines from a pipe or as
//! an edited line with completion on a terminal.

use std::io::{Read, Write};

use reovim_client::{Client, Fault, Key};
use reovim_client_cli::{Act, Line, Miss, complete, help, input, plain};
use reovim_host::term;

use crate::Fail;
use crate::remote::Remote;

const PROMPT: &str = "> ";

/// Run `command`, or an interactive session when it is empty. `help`
/// and `--help` are answered without a link; anything else needs one.
pub fn run(socket: &str, command: &[String]) -> Result<(), Fail> {
    if let Some(text) = help(command) {
        return Ok(write(text?.as_bytes())?);
    }
    let mut client = linked(socket)?;
    if !command.is_empty() {
        let out = input(&mut client, &filled(command)?)?;
        return Ok(write(&out)?);
    }
    if term::is_tty() { terminal(&mut client) } else { piped(&mut client) }
}

/// A Client on the server at `socket`; a link that never opened says
/// so with the path, which only the app knows.
fn linked(socket: &str) -> Result<Client, Fail> {
    match Remote::client(socket, Vec::new()) {
        Err(Fail::Host(_) | Fail::Client(Fault::Carrier)) => {
            Err(plain(&Fault::Carrier, Some(socket)).into())
        }
        other => other,
    }
}

/// `write <target>` takes its bytes from stdin.
fn filled(words: &[String]) -> Result<Vec<String>, Fail> {
    let mut words = words.to_vec();
    if words.len() == 2 && words[0] == "write" {
        let mut bytes = Vec::new();
        std::io::stdin().read_to_end(&mut bytes).map_err(|_| Fault::Carrier)?;
        let text = String::from_utf8(bytes)
            .map_err(|_| Miss::line("stdin is not UTF-8"))?;
        words.push(text);
    }
    Ok(words)
}

/// To stdout, flushed; a closed stdout is `Carrier`.
fn write(bytes: &[u8]) -> Result<(), Fault> {
    let mut out = std::io::stdout();
    out.write_all(bytes).and_then(|()| out.flush()).map_err(|_| Fault::Carrier)
}

/// One command per stdin line until EOF or `quit`; a fault ends the
/// line, not the session.
fn piped(client: &mut Client) -> Result<(), Fail> {
    let stdin = std::io::stdin();
    let mut line = String::new();
    while stdin.read_line(&mut line).map_err(|_| Fault::Carrier)? != 0 {
        let words = split(&line);
        line.clear();
        match words.first().map(String::as_str) {
            None => {}
            Some("quit") => break,
            Some(_) => report(
                input(client, &words)
                    .and_then(|o| write(&o).map_err(|f| plain(&f, None))),
            ),
        }
    }
    Ok(())
}

fn split(line: &str) -> Vec<String> {
    line.split_whitespace().map(str::to_string).collect()
}

fn report(result: Result<(), Miss>) {
    if let Err(miss) = result {
        eprintln!("reovim: {miss}");
    }
}

/// The line editor under raw mode; back to cooked on every exit.
fn terminal(client: &mut Client) -> Result<(), Fail> {
    term::enter()?;
    let result = edit(client);
    let _ = term::leave();
    result
}

fn edit(client: &mut Client) -> Result<(), Fail> {
    let mut line = Line::default();
    draw(&line)?;
    loop {
        let Some(key) = term::poll(1000)? else { continue };
        let go_on = match line.key(map(key)) {
            Act::Nothing => continue,
            Act::Redraw => true,
            Act::Complete => offer(client, &mut line)?,
            Act::Submit(text) => submit(client, &text)?,
            Act::Quit => false,
        };
        if !go_on {
            return Ok(raw(b"\n")?);
        }
        draw(&line)?;
    }
}

/// Tab: one candidate goes into the line, several are shown.
fn offer(client: &mut Client, line: &mut Line) -> Result<bool, Fail> {
    let completion = complete(client, line.text());
    if completion.candidates.len() > 1 {
        let shown = completion.candidates.join("  ");
        raw(format!("\n{shown}\n").as_bytes())?;
    }
    line.apply(&completion);
    Ok(true)
}

/// Enter: the command runs; false on `quit`.
fn submit(client: &mut Client, text: &str) -> Result<bool, Fail> {
    raw(b"\n")?;
    let words = split(text);
    match words.first().map(String::as_str) {
        None => Ok(true),
        Some("quit") => Ok(false),
        Some(_) => {
            match input(client, &words) {
                Ok(out) => shown(&out)?,
                Err(miss) => raw(format!("reovim: {miss}\n").as_bytes())?,
            }
            Ok(true)
        }
    }
}

/// The prompt and the line on the current row, cursor placed.
fn draw(line: &Line) -> Result<(), Fault> {
    let (text, column) = line.render(PROMPT);
    let right =
        if column > 0 { format!("\x1b[{column}C") } else { String::new() };
    write(format!("\r\x1b[2K{text}\r{right}").as_bytes())
}

/// Output on its own lines: bytes that do not end in one get a newline,
/// or the prompt would draw over them.
fn shown(bytes: &[u8]) -> Result<(), Fault> {
    raw(bytes)?;
    if bytes.is_empty() || bytes.ends_with(b"\n") {
        return Ok(());
    }
    raw(b"\n")
}

/// Bytes to the terminal in raw mode: every newline needs a return.
fn raw(bytes: &[u8]) -> Result<(), Fault> {
    let mut out = Vec::with_capacity(bytes.len());
    for &b in bytes {
        if b == b'\n' {
            out.push(b'\r');
        }
        out.push(b);
    }
    write(&out)
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
