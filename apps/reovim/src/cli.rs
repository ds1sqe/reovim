//! `reovim cli`: one link and one Client. One typed line runs and
//! exits; none reads lines until the end of the input, as lines from a
//! pipe or as an edited line with completion on a terminal.

use std::io::Write as _;
use std::thread::sleep;
use std::time::Duration;

use reovim_client::{Calls, Fault, Host, Key};
use reovim_client_sh::{Act, Line, Miss, Open, Ran, Shell, complete};
use reovim_host::{clock, term};

use crate::Fail;
use crate::remote::Remote;

/// Ctrl-L, as a shell answers it: the screen empty and the line drawn
/// again at the top of it.
const CLEAR: &[u8] = b"\x1b[2J\x1b[H";

/// Between one read of an open run's cell and the next. A Driver that
/// follows does its work inside that read, so the tick is how often one
/// is asked; when the link learns to push, this wait becomes a wake.
const TICK: u64 = 50;

/// Run `line`, or an interactive session when it is empty.
pub fn run(socket: &str, line: &[String], color: bool) -> Result<(), Fail> {
    let host = linked(socket)?;
    let ctx = host.own();
    if !line.is_empty() {
        // One line has no session to stand in: it runs at the root.
        let mut shell = Shell::default();
        let text = line.join(" ");
        return once(&ctx, &mut shell, &text, color, Mode::Cooked);
    }
    if term::is_tty() { terminal(&ctx, color) } else { piped(&ctx, color) }
}

/// The terminal's columns, which a listing lays its names out across; 0
/// when stdout is a pipe, and then one name goes per line.
fn width() -> u64 {
    if !term::is_tty_out() {
        return 0;
    }
    term::size().map_or(0, |(columns, _)| u64::from(columns))
}

/// How an open run writes and waits: a cooked terminal or a pipe, where
/// Ctrl-C arrives as SIGINT and a newline is a newline; or raw mode,
/// where Ctrl-C is a key and every newline needs its return.
#[derive(Clone, Copy, Eq, PartialEq)]
enum Mode {
    Cooked,
    Raw,
}

impl Mode {
    fn show(self, bytes: &[u8]) -> Result<(), Fault> {
        match self {
            Self::Cooked => write(bytes),
            Self::Raw => shown(bytes),
        }
    }

    /// Up to `TICK` milliseconds for Ctrl-C; false once it has come.
    fn wait(self) -> Result<bool, Fail> {
        if self == Self::Cooked {
            sleep(Duration::from_millis(TICK));
            return Ok(!term::interrupted() && !term::stopped());
        }
        let key = term::poll(TICK)?;
        Ok(!term::stopped() && key != Some(term::Key::Ctrl('c')))
    }
}

/// One typed line: what the run printed, and the loop for one that came
/// back open.
fn once(
    ctx: &dyn Calls,
    shell: &mut Shell,
    text: &str,
    color: bool,
    mode: Mode,
) -> Result<(), Fail> {
    // Asked again each time, in case the terminal was resized.
    shell.screen(width(), color);
    let Ran { out, open } = reovim_client_sh::run(ctx, shell, text)?;
    mode.show(&out)?;
    let Some(open) = open else { return Ok(()) };
    follow(ctx, open, mode)
}

/// A run still open, read on every tick: what the cell holds now is
/// printed and nothing is printed between, the empty read that ends the
/// cell ends the loop, and Ctrl-C closes the cell and is no failure.
fn follow(ctx: &dyn Calls, mut open: Open, mode: Mode) -> Result<(), Fail> {
    if mode == Mode::Cooked {
        term::catch_interrupt();
    }
    while mode.wait()? {
        let (bytes, ended) = open.more(ctx)?;
        mode.show(&bytes)?;
        if ended {
            return Ok(());
        }
    }
    Ok(open.close(ctx)?)
}

/// A Client on the server at `socket`, behind the one door; a link
/// that never opened says so with the path, which only the app knows.
fn linked(socket: &str) -> Result<Host, Fail> {
    match Remote::client(socket, Vec::new()) {
        Err(Fail::Host(_) | Fail::Client(Fault::Carrier)) => {
            Err(Miss::line(&format!("no server at {socket}")).into())
        }
        other => Ok(Host::over(other?, clock::now)),
    }
}

/// To stdout, flushed; a closed stdout is `Carrier`.
fn write(bytes: &[u8]) -> Result<(), Fault> {
    let mut out = std::io::stdout();
    out.write_all(bytes).and_then(|()| out.flush()).map_err(|_| Fault::Carrier)
}

/// One typed line per stdin line until the end of the input, which ends
/// the session; a fault ends the line, not the session.
fn piped(ctx: &dyn Calls, color: bool) -> Result<(), Fail> {
    let stdin = std::io::stdin();
    let mut shell = Shell::default();
    let mut line = String::new();
    while stdin.read_line(&mut line).map_err(|_| Fault::Carrier)? != 0 {
        if !line.trim().is_empty() {
            report(once(ctx, &mut shell, &line, color, Mode::Cooked));
        }
        line.clear();
    }
    Ok(())
}

/// One line's fault ends the line, not the session.
fn report(result: Result<(), Fail>) {
    if let Err(fail) = result {
        eprintln!("reovim: {fail}");
    }
}

/// The line editor under raw mode; back to cooked on every exit.
fn terminal(ctx: &dyn Calls, color: bool) -> Result<(), Fail> {
    term::enter()?;
    let result = edit(ctx, color);
    let _ = term::leave();
    result
}

fn edit(ctx: &dyn Calls, color: bool) -> Result<(), Fail> {
    let mut line = Line::default();
    let mut shell = Shell::default();
    draw(&line, &shell)?;
    loop {
        // A signal or a terminal that went away ends the wait for
        // good; without this the loop would spin on an empty poll.
        if term::stopped() {
            return Err(Fail::Signal);
        }
        let Some(key) = term::poll(1000)? else { continue };
        match line.key(map(key)) {
            Act::Nothing => continue,
            Act::Redraw => {}
            Act::Clear => write(CLEAR)?,
            Act::Complete => offer(ctx, &shell, &mut line)?,
            Act::Submit(text) => submit(ctx, &mut shell, &text, color)?,
            Act::Quit => return Ok(raw(b"\n")?),
        }
        draw(&line, &shell)?;
    }
}

/// Tab: one candidate goes into the line, several are printed one a
/// line above it.
fn offer(ctx: &dyn Calls, shell: &Shell, line: &mut Line) -> Result<(), Fault> {
    let completion = complete(ctx, shell, line.text());
    if completion.candidates.len() > 1 {
        raw(format!("\n{}\n", completion.candidates.join("\n")).as_bytes())?;
    }
    line.apply(&completion);
    Ok(())
}

/// Enter: the line runs. A run that follows runs until Ctrl-C, which is
/// a key here, and the line comes back after.
fn submit(
    ctx: &dyn Calls,
    shell: &mut Shell,
    text: &str,
    color: bool,
) -> Result<(), Fault> {
    raw(b"\n")?;
    if text.trim().is_empty() {
        return Ok(());
    }
    if let Err(fail) = once(ctx, shell, text, color, Mode::Raw) {
        raw(format!("reovim: {fail}\n").as_bytes())?;
    }
    Ok(())
}

/// The prompt and the line on the current row, cursor placed.
fn draw(line: &Line, shell: &Shell) -> Result<(), Fault> {
    let (text, column) = line.render(&shell.prompt());
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
