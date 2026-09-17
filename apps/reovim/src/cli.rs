//! `reovim cli`: one link and one Client. One command runs and exits;
//! none reads commands until EOF or `quit`, as lines from a pipe or as
//! an edited line with completion on a terminal.

use std::io::{Read, Write};
use std::thread::sleep;
use std::time::Duration;

use reovim_client::{Client, Fault, Key};
use reovim_client_cli::{
    Act, Follow, Line, Miss, Out, Paint, Shell, complete, following, help,
    input, laid_out, plain,
};
use reovim_host::term;

use crate::Fail;
use crate::remote::Remote;

/// Ctrl-L, as a shell answers it: the screen empty and the line drawn
/// again at the top of it.
const CLEAR: &[u8] = b"\x1b[2J\x1b[H";

/// Between one `since` and the next. The wire is request and reply, so
/// a following word asks; when the link learns to push, this wait
/// becomes a wake (a Series B number).
const TICK: u64 = 50;

/// Where this run's output goes: the terminal's columns when stdout is
/// one, and the color the app decided on.
#[must_use]
pub fn out(color: bool) -> Out {
    let paint = if color { Paint::on() } else { Paint::off() };
    Out { width: width(), paint }
}

/// Run `command`, or an interactive session when it is empty. `help`
/// and `--help` are answered without a link; anything else needs one.
pub fn run(socket: &str, command: &[String], out: Out) -> Result<(), Fail> {
    if let Some(text) = help(command) {
        return Ok(write(text?.as_bytes())?);
    }
    let mut client = linked(socket)?;
    if !command.is_empty() {
        // One command has no session to stand in: it runs at the root.
        let mut shell = Shell::default();
        let words = filled(command)?;
        return once(&mut client, &mut shell, &words, out, Mode::Cooked);
    }
    if term::is_tty() {
        terminal(&mut client, out)
    } else {
        piped(&mut client, out)
    }
}

/// The terminal's columns, which `ls` lays its names out across; `None`
/// when stdout is a pipe, and the library writes one name per line.
fn width() -> Option<u16> {
    if !term::is_tty_out() {
        return None;
    }
    term::size().ok().map(|(columns, _)| columns)
}

/// The columns again, in case the terminal was resized between two
/// commands; the color is the run's.
fn sized(out: Out) -> Out {
    Out { width: width(), paint: out.paint }
}

/// How a following word waits and writes: a cooked terminal or a pipe,
/// where Ctrl-C arrives as SIGINT and a newline is a newline; or raw
/// mode, where Ctrl-C is a key and every newline needs its return.
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

/// One command: a word that follows the tree runs the loop, and
/// anything else prints once.
fn once(
    client: &mut Client,
    shell: &mut Shell,
    words: &[String],
    out: Out,
    mode: Mode,
) -> Result<(), Fail> {
    if let Some(plan) = following(client, shell, words, out)? {
        return follow(client, plan, mode);
    }
    Ok(mode.show(&input(client, shell, words, out)?)?)
}

/// The one watch loop: what the word prints now, then what it makes of
/// every event until Ctrl-C or the count. The handles it watched are
/// closed on the way out; Ctrl-C is not a failure.
fn follow(
    client: &mut Client,
    mut plan: Follow,
    mode: Mode,
) -> Result<(), Fail> {
    if mode == Mode::Cooked {
        term::catch_interrupt();
    }
    mode.show(&plan.first)?;
    let mut left = plan.limit;
    while left != Some(0) && mode.wait()? {
        left = step(client, &mut plan, mode, left)?;
    }
    for h in &plan.handles {
        let _ = client.close(*h);
    }
    Ok(())
}

/// Everything that happened since the last ask, and how many prints
/// are left after it.
fn step(
    client: &mut Client,
    plan: &mut Follow,
    mode: Mode,
    mut left: Option<usize>,
) -> Result<Option<usize>, Fail> {
    for event in client.since()? {
        if left == Some(0) {
            break;
        }
        let Some(bytes) = plan.event(client, event)? else { continue };
        mode.show(&bytes)?;
        left = left.map(|n| n.saturating_sub(1));
    }
    Ok(left)
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
fn piped(client: &mut Client, out: Out) -> Result<(), Fail> {
    let stdin = std::io::stdin();
    let mut shell = Shell::default();
    let mut line = String::new();
    while stdin.read_line(&mut line).map_err(|_| Fault::Carrier)? != 0 {
        let words = split(&line);
        line.clear();
        match words.first().map(String::as_str) {
            None => {}
            Some("quit") => break,
            Some(_) => {
                report(once(client, &mut shell, &words, out, Mode::Cooked));
            }
        }
    }
    Ok(())
}

fn split(line: &str) -> Vec<String> {
    line.split_whitespace().map(str::to_string).collect()
}

/// One line's fault ends the line, not the session.
fn report(result: Result<(), Fail>) {
    if let Err(fail) = result {
        eprintln!("reovim: {fail}");
    }
}

/// The line editor under raw mode; back to cooked on every exit.
fn terminal(client: &mut Client, out: Out) -> Result<(), Fail> {
    term::enter()?;
    let result = edit(client, out);
    let _ = term::leave();
    result
}

fn edit(client: &mut Client, out: Out) -> Result<(), Fail> {
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
        let go_on = match line.key(map(key)) {
            Act::Nothing => continue,
            Act::Redraw => true,
            Act::Clear => write(CLEAR).map(|()| true)?,
            Act::Complete => offer(client, &shell, &mut line, sized(out))?,
            Act::Submit(text) => submit(client, &mut shell, &text, out)?,
            Act::Quit => false,
        };
        if !go_on {
            return Ok(raw(b"\n")?);
        }
        draw(&line, &shell)?;
    }
}

/// Tab: one candidate goes into the line, several are shown as `ls`
/// shows a listing.
fn offer(
    client: &mut Client,
    shell: &Shell,
    line: &mut Line,
    out: Out,
) -> Result<bool, Fail> {
    let completion = complete(client, shell, line.text());
    if completion.candidates.len() > 1 {
        let shown = laid_out(&completion.candidates, out.width);
        raw(format!("\n{shown}").as_bytes())?;
    }
    line.apply(&completion);
    Ok(true)
}

/// Enter: the command runs; false on `quit`. A word that follows runs
/// until Ctrl-C, which is a key here, and the line comes back after.
fn submit(
    client: &mut Client,
    shell: &mut Shell,
    text: &str,
    out: Out,
) -> Result<bool, Fail> {
    raw(b"\n")?;
    let words = split(text);
    match words.first().map(String::as_str) {
        None => Ok(true),
        Some("quit") => Ok(false),
        Some(_) => {
            let ran = once(client, shell, &words, sized(out), Mode::Raw);
            if let Err(fail) = ran {
                raw(format!("reovim: {fail}\n").as_bytes())?;
            }
            Ok(true)
        }
    }
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
