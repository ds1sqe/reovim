//! `reovim tui [FILE]` over the link, and bare `reovim [FILE]` with
//! the server in the same process: the terminal loop, drawing in plain
//! ANSI. Keys go to the tui, the frame comes back.

use std::fmt::Write as _;
use std::io::Write;

use reovim_client::{BAR, Client, Fault, Key, Plugin, under};
use reovim_client_tui::{Frame, Step, Table, Tui, Want, width};
use reovim_host::term;
use reovim_plugin_cli::Cli;
use reovim_plugin_explorer::Explorer;
use reovim_plugin_vim::Vim;

use crate::Fail;
use crate::local::Local;
use crate::log::Sink;
use crate::remote::Remote;
use crate::server::{Links, boot};

/// What runs between keys: nothing over the link, the server's step in
/// the bare form.
type Serve<'a> = &'a mut dyn FnMut() -> Result<(), Fail>;

/// The tui over whichever Plugin the table gives: the one editor the
/// binary offers.
type Editor = Tui<Box<dyn Plugin>>;

/// Which Plugin the binary makes for what a region is asked to show:
/// Vim by name and for a cell, the explorer by name and for a
/// Container, the cli buffer by name alone, no rd being of its shape.
/// The names are the app's; the tui knows none of them.
fn table() -> Table<Box<dyn Plugin>> {
    Box::new(|want| match want {
        Want::Named("vim") | Want::Cell => Some(Box::new(Vim::default())),
        Want::Named("explorer") | Want::Container => {
            Some(Box::new(Explorer::default()))
        }
        Want::Named("cli") => Some(Box::new(Cli::default())),
        Want::Named(_) => None,
    })
}

/// `reovim tui`: a Client over the link, named so a cut link can be
/// taken up again.
pub fn remote(
    socket: &str,
    line: Option<usize>,
    file: Option<&str>,
) -> Result<u8, Fail> {
    let credential = format!("tui:{}", std::process::id()).into_bytes();
    let client = Remote::client(socket, credential)?;
    run(client, line, ("", file), &mut || Ok(()))
}

/// Bare `reovim`: the server on this thread over `/`, listening at
/// `socket` and served between keys, so a cli or another tui reaches
/// this editor.
pub fn bare(
    socket: &str,
    log: Option<&str>,
    sys: &[(String, String)],
    line: Option<usize>,
    file: Option<&str>,
) -> Result<u8, Fail> {
    let (kernel, _modules) = boot("/", sys)?;
    // Before the terminal is taken: the hook `enter` installs wraps
    // this one, so a panic gives the terminal back, then says where
    // the core went.
    crate::dump::watch(&kernel);
    let mut links = Links::listen(socket)?;
    let mut sink = log.map(|path| Sink::open(&kernel, path)).transpose()?;
    let dir = under("", &reovim_host::dirs::work()?);
    let file = file.map(|file| under(&dir, file));
    let mut client = Local::client(&kernel)?;
    // The terminal is the editor's, so where it listens is said in the
    // log the `server` form prints on stderr.
    let entry = client.resolve("/sys/log")?;
    client.write_all(entry, format!("listen {socket}").into_bytes())?;
    run(client, line, (&dir, file.as_deref()), &mut || {
        links.drain(&kernel);
        sink.as_mut().map_or(Ok(()), Sink::step)
    })
}

/// The loop in the host's terminal mode, left on every exit, a fault
/// printed after; the exit code the tui asked for.
fn run(
    client: Client,
    line: Option<usize>,
    (dir, file): (&str, Option<&str>),
    serve: Serve,
) -> Result<u8, Fail> {
    let mut tui = Tui::open(client, dir, file, line, table())?;
    term::enter()?;
    let result = drive(&mut tui, serve);
    let _ = term::leave();
    result
}

fn drive(tui: &mut Editor, serve: Serve) -> Result<u8, Fail> {
    // The shape the terminal was last told of: it is told again only
    // when the Plugin has changed it.
    let mut shape = None;
    draw(tui, &mut shape)?;
    loop {
        serve()?;
        let step = match term::poll(20)? {
            Some(key) => tui.key(map(key))?,
            None if term::stopped() => return Err(Fail::Signal),
            None => tui.tick()?,
        };
        match step {
            Step::Same => {}
            Step::Changed => draw(tui, &mut shape)?,
            Step::Quit(code) => return Ok(code),
        }
    }
}

/// The whole frame: text rows, the status row inverted, the cursor
/// hidden while it is drawn, placed after and given its shape when
/// that is not the shape `shape` says the terminal already has.
fn draw(tui: &mut Editor, shape: &mut Option<u64>) -> Result<(), Fail> {
    let (cols, rows) = term::size()?;
    let Frame { lines, cursor, shape: wanted, selection, marks, status } =
        tui.frame(cols, rows)?;
    let text_rows = usize::from(rows.saturating_sub(1));
    let blank = String::new();
    let rest =
        std::iter::repeat_n(&blank, text_rows.saturating_sub(lines.len()));
    let mut screen = String::from("\x1b[?25l\x1b[H");
    for (row, line) in lines.iter().chain(rest).enumerate() {
        let on = |cells: &[(u16, u16, u16)]| {
            cells.iter().find(|s| usize::from(s.0) == row).map(|s| (s.1, s.2))
        };
        // A selection shows inverted, a search match underlined.
        let shown = match (on(&selection), on(&marks)) {
            (Some((from, to)), _) => styled(line, from, to, "7"),
            (None, Some((from, to))) => styled(line, from, to, "4"),
            (None, None) => line.clone(),
        };
        let _ = write!(screen, "\x1b[2K{shown}\r\n");
    }
    let status: String = status.chars().take(usize::from(cols)).collect();
    let _ = write!(screen, "\x1b[2K\x1b[7m{status}\x1b[0m");
    let (row, col) = (cursor.0 + 1, cursor.1 + 1);
    let _ = write!(screen, "\x1b[{row};{col}H");
    if *shape != Some(wanted) {
        // DECSCUSR: a steady bar or a steady block.
        screen.push_str(if wanted == BAR { "\x1b[6 q" } else { "\x1b[2 q" });
        *shape = Some(wanted);
    }
    screen.push_str("\x1b[?25h");
    Ok(out(screen.as_bytes())?)
}

/// `line` with the cells `from..to` in the SGR style `code`; a cell
/// past the line's end is the newline's, shown as a space.
fn styled(line: &str, from: u16, to: u16, code: &str) -> String {
    let (from, to) = (usize::from(from), usize::from(to));
    let (head, rest) = line.split_at(byte_at_cell(line, from));
    let (mid, tail) =
        rest.split_at(byte_at_cell(rest, to.saturating_sub(from)));
    let cells: usize = line.chars().map(width).sum();
    let newline = if to > cells { " " } else { "" };
    format!("{head}\x1b[{code}m{mid}{newline}\x1b[0m{tail}")
}

/// The byte index in `line` where screen cell `cell` starts, or the
/// end.
fn byte_at_cell(line: &str, cell: usize) -> usize {
    let mut cells = 0;
    for (at, c) in line.char_indices() {
        if cells >= cell {
            return at;
        }
        cells += width(c);
    }
    line.len()
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
