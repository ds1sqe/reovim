//! `reovim tui [FILE]` over the link, and bare `reovim [FILE]` with
//! the server in the same process: the terminal loop, drawing in plain
//! ANSI. Keys go to the tui, the frame comes back.

use std::cell::RefCell;
use std::fmt::Write as _;
use std::io::Write;
use std::rc::Rc;

use reovim_client::dev::{self, Table, Want};
use reovim_client::{Clip, Error, Fault, Host};
use reovim_client_api::{BAR, Key};
use reovim_client_tui::{Frame, Step, Tui, width};
use reovim_host::{clipboard, term};
use reovim_sdk::path::under;
use reovim_sdk::walk::ROOT;
use reovim_sdk::{cell, ops, sgr};

use crate::Fail;
use crate::local::Local;
use crate::log::Sink;
use crate::remote::Remote;
use crate::server::{Links, boot};

/// What runs between keys: nothing over the link, the server's step in
/// the bare form.
type Serve<'a> = &'a mut dyn FnMut() -> Result<(), Fail>;

/// Which Plugin the binary makes for what a region is asked to show,
/// and the name it is loaded under at `/self`: Vim by name and for a
/// cell, the explorer by name and for a Container, the cli buffer by
/// name alone, no rd being of its shape. The names are the app's; the
/// tui knows none of them.
fn table() -> Table {
    Box::new(|want| {
        let made = match want {
            Want::Named("vim") | Want::Cell => {
                ("vim", reovim_plugin_vim::boot())
            }
            Want::Named("explorer") | Want::Container => {
                ("explorer", reovim_plugin_explorer::boot())
            }
            Want::Named("cli") => ("cli", reovim_plugin_cli::boot()),
            Want::Named(_) => return None,
        };
        Some((String::from(made.0), made.1))
    })
}

/// The clipboard of the machine this binary runs on, which is the one
/// the person sits at: the host's tool behind two closures, the only
/// std in the path from it to `/self/dev/clipboard`.
fn clip() -> Clip {
    Clip::new(
        Box::new(|| clipboard::read().map_err(mapped)),
        Box::new(|bytes| clipboard::write(bytes).map_err(mapped)),
    )
}

/// Host failures as the tree's: no tool is `Unsupported`, a tool that
/// failed is `Busy`.
const fn mapped(error: reovim_host::Error) -> Error {
    match error {
        reovim_host::Error::NotFound | reovim_host::Error::Exists => {
            Error::Invalid
        }
        reovim_host::Error::Denied => Error::Denied,
        reovim_host::Error::Io => Error::Busy,
        reovim_host::Error::Unsupported => Error::Unsupported,
    }
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
    run(crate::client::over(client)?, line, ("", file), &mut || Ok(()))
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
    let links = Rc::new(RefCell::new(Links::listen(socket)?));
    let mut sink = log.map(|path| Sink::open(&kernel, path)).transpose()?;
    let dir = under("", &reovim_host::dirs::work()?);
    let file = file.map(|file| under(&dir, file));
    let client = Local::client(&kernel, Some(&links))?;
    let host = crate::client::over(client)?;
    // The terminal is the editor's, so where it listens is said in the
    // log the `server` form prints on stderr.
    let entry = cell::resolve(&host.own(), ROOT, "/sys/log")?;
    ops::put(&host.own(), entry, format!("listen {socket}").into_bytes())?;
    run(host, line, (&dir, file.as_deref()), &mut || {
        links.borrow_mut().drain(&kernel);
        sink.as_mut().map_or(Ok(()), Sink::step)
    })
}

/// The loop in the host's terminal mode, left on every exit, a fault
/// printed after; the exit code the tui asked for.
fn run(
    host: Rc<Host>,
    line: Option<usize>,
    (dir, file): (&str, Option<&str>),
    serve: Serve,
) -> Result<u8, Fail> {
    let line = line.map(|line| u64::try_from(line).unwrap_or(u64::MAX));
    let dev = dev::boot(&host, table(), clip(), dir, file, line)?;
    // The tui takes the Host; what answers a remote holds it too.
    let served = host.clone();
    let mut tui = Tui::open(host, dev)?;
    term::enter()?;
    let result = drive(&mut tui, serve, &served);
    let _ = term::leave();
    result
}

fn drive(tui: &mut Tui, serve: Serve, host: &Host) -> Result<u8, Fail> {
    // The shape the terminal was last told of: it is told again only
    // when the Plugin has changed it.
    let mut shape = None;
    draw(tui, &mut shape)?;
    loop {
        serve()?;
        let step = match term::poll(20)? {
            Some(term::Event::Key(key)) => tui.key(map(key))?,
            // A resize is a reason to draw: `draw` reads the terminal
            // again, so the frame the Plugin makes is the new shape's.
            Some(term::Event::Resize) => Step::Changed,
            None if term::stopped() => return Err(Fail::Signal),
            None => tui.tick()?,
        };
        // The step's own `Since` brought over what a remote asked of
        // this Client; it is answered here, at the one depth where
        // nothing of the Client's own is on the stack.
        host.serve();
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
fn draw(tui: &mut Tui, shape: &mut Option<u64>) -> Result<(), Fail> {
    let (cols, rows) = term::size()?;
    let frame = tui.frame(cols, rows)?;
    let Frame { lines, cursor, shape: wanted, status, .. } = &frame;
    let wanted = *wanted;
    let text_rows = usize::from(rows.saturating_sub(1));
    let blank = String::new();
    let rest =
        std::iter::repeat_n(&blank, text_rows.saturating_sub(lines.len()));
    let mut screen = String::from("\x1b[?25l\x1b[H");
    for (row, line) in lines.iter().chain(rest).enumerate() {
        let shown = painted(line, &runs(&frame, row));
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

/// What is drawn on the row `row`, in the order the runs start: the
/// selection inverted, a search match underlined, and every other span
/// in the color its style stands for.
fn runs(frame: &Frame, row: usize) -> Vec<(u16, u16, &'static str)> {
    let here = |at: u16| usize::from(at) == row;
    let mut runs: Vec<(u16, u16, &'static str)> = Vec::new();
    let selected = frame.selection.iter().filter(|s| here(s.0));
    runs.extend(selected.map(|s| (s.1, s.2, "7")));
    let marked = frame.marks.iter().filter(|s| here(s.0));
    runs.extend(marked.map(|s| (s.1, s.2, "4")));
    let styled = frame.styled.iter().filter(|s| here(s.0));
    runs.extend(styled.map(|s| (s.1, s.2, sgr::code(s.3))));
    runs.sort_by_key(|run| run.0);
    runs
}

/// `line` with each of `runs` in its SGR style, a run that overlaps
/// the one before it left out; a cell past the line's end is the
/// newline's, shown as a space.
fn painted(line: &str, runs: &[(u16, u16, &str)]) -> String {
    let cells: usize = line.chars().map(width).sum();
    let mut out = String::new();
    let mut at = 0;
    for &(from, to, code) in runs {
        let (from, end) = (usize::from(from), usize::from(to));
        let (from, end) = (byte_at_cell(line, from), byte_at_cell(line, end));
        if from < at {
            continue;
        }
        let newline = if usize::from(to) > cells { " " } else { "" };
        let (head, mid) = (&line[at..from], &line[from..end]);
        let _ = write!(out, "{head}\x1b[{code}m{mid}{newline}\x1b[0m");
        at = end;
    }
    out.push_str(&line[at..]);
    out
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

#[cfg(test)]
mod tests {
    use reovim_client_api::{BLUE, GREEN};
    use reovim_client_tui::Frame;

    use super::{painted, runs};

    #[test]
    fn every_run_of_a_row_is_drawn_in_the_color_its_style_stands_for() {
        let styled = vec![(0, 0, 3, BLUE), (0, 5, 7, GREEN), (1, 0, 1, BLUE)];
        let frame = Frame { styled, ..Frame::default() };
        let drawn = painted("bin  ls", &runs(&frame, 0));
        assert_eq!(drawn, "\x1b[1;34mbin\x1b[0m  \x1b[32mls\x1b[0m");
    }

    #[test]
    fn a_selection_through_the_newline_shows_that_cell_as_a_space() {
        let frame = Frame { selection: vec![(0, 0, 4)], ..Frame::default() };
        assert_eq!(painted("abc", &runs(&frame, 0)), "\x1b[7mabc \x1b[0m");
    }

    #[test]
    fn a_run_that_overlaps_the_one_before_it_is_left_out() {
        let frame = Frame {
            selection: vec![(0, 0, 7)],
            styled: vec![(0, 0, 3, BLUE)],
            ..Frame::default()
        };
        let drawn = painted("bin  ls", &runs(&frame, 0));
        assert_eq!(drawn, "\x1b[7mbin  ls\x1b[0m", "the selection stands");
    }
}
