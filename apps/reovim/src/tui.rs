//! `reovim tui [FILE]` over the link, and bare `reovim [FILE]` with
//! the server in the same process: the terminal loop, drawing in plain
//! ANSI. Keys go to the tui, the frame comes back.

use std::cell::RefCell;
use std::fmt::Write as _;
use std::io::Write;
use std::rc::Rc;

use reovim_client::dev::{self, Table, Want};
use reovim_client::{Calls, Clip, Error, Fault, Host};
use reovim_client_api::{BAR, Key, display};
use reovim_client_region::drafted;
use reovim_client_tui::{Frame, Step, Tui};
use reovim_host::{clipboard, term};
use reovim_sdk::look;
use reovim_sdk::path::{split, under};
use reovim_sdk::walk::ROOT;
use reovim_sdk::{cell, ops};

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
///
/// One `Editor` across every swap of the region (01.0): what a stay
/// keeps of Vim's state outlives it, and a second stay is a second
/// `Stay` over the one engine, not a fresh one.
fn table(init: Option<String>) -> Table {
    use reovim_plugin_vim::Editor;
    let vim = init.map_or_else(Editor::new, Editor::with_init);
    Box::new(move |want| {
        let made = match want {
            Want::Named("vim") | Want::Cell => ("vim", vim.booted()),
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
    files: &[String],
) -> Result<u8, Fail> {
    let credential = format!("tui:{}", std::process::id()).into_bytes();
    let client = Remote::client(socket, credential)?;
    run(crate::client::over(client)?, line, ("", files), &mut || Ok(()))
}

/// Bare `reovim`: the server on this thread over `/`, listening at
/// `socket` and served between keys, so a cli or another tui reaches
/// this editor.
pub fn bare(
    socket: &str,
    log: Option<&str>,
    sys: &[(String, String)],
    line: Option<usize>,
    files: &[String],
) -> Result<u8, Fail> {
    let (kernel, _modules) = boot("/", sys)?;
    // Before the terminal is taken: the hook `enter` installs wraps
    // this one, so a panic gives the terminal back, then says where
    // the core went.
    crate::dump::watch(&kernel);
    let links = Rc::new(RefCell::new(Links::listen(socket)?));
    let mut sink = log.map(|path| Sink::open(&kernel, path)).transpose()?;
    let dir = under("", &reovim_host::dirs::work()?);
    let files: Vec<String> =
        files.iter().map(|file| under(&dir, file)).collect();
    let client = Local::client(&kernel, Some(&links))?;
    let host = crate::client::over(client)?;
    // The terminal is the editor's, so where it listens is said in the
    // log the `server` form prints on stderr.
    let entry = cell::resolve(&host.own(), ROOT, "/sys/log")?;
    ops::put(&host.own(), entry, format!("listen {socket}").into_bytes())?;
    run(host, line, (&dir, &files), &mut || {
        links.borrow_mut().drain(&kernel);
        sink.as_mut().map_or(Ok(()), Sink::step)
    })
}

/// The loop in the host's terminal mode, left on every exit, a fault
/// printed after; the exit code the tui asked for.
fn run(
    host: Rc<Host>,
    line: Option<usize>,
    (dir, files): (&str, &[String]),
    serve: Serve,
) -> Result<u8, Fail> {
    let init = captured_config(&host.own())?;
    let line = line.map(|line| u64::try_from(line).unwrap_or(u64::MAX));
    // The one place a name is split: what comes off the first file is
    // the view's to read, never this binary's.
    let mut names: Vec<&str> = files.iter().map(String::as_str).collect();
    let mut locator = "";
    if let Some(first) = names.first_mut() {
        (*first, locator) = located(&host.own(), first);
    }
    let dev = dev::boot(
        &host,
        table(init),
        clip(),
        dir,
        names.first().copied(),
        line,
        locator,
    )?;
    drafts(&host.own(), &names)?;
    // The tui takes the Host; what answers a remote holds it too.
    let served = host.clone();
    let mut tui = Tui::open(host, dev)?;
    term::enter()?;
    let result = drive(&mut tui, serve, &served);
    let _ = term::leave();
    result
}

/// Every name in `files` opened as a draft, in the order given, through
/// [`drafted`]: `dev::boot`'s ask is the first, already read into
/// `windows/0/ask`'s target, so its own open here answers that same
/// draft and costs no new one, which is why it stands as draft `0`. A
/// name not on disk, or a source `open` refuses (a directory), is
/// skipped; from the second name on, that is said in `/sys/log`. The
/// first is not said: not on disk it is the new file the ask already
/// names, and a directory is the explorer's, `table` answering it a
/// `Want::Container`.
///
/// The first name having no room in the Buffers is the one refusal
/// this stops at: it is the name the region was opened on, so nothing
/// opens at all, and [`Fail::Big`] names that file, where `no space`
/// alone names none. A later name that is too big is skipped and
/// said, as any other the open refuses.
fn drafts(ctx: &dyn Calls, files: &[&str]) -> Result<(), Fail> {
    for (index, file) in files.iter().enumerate() {
        let path = format!("/module/fs/root/{file}");
        let opened = cell::resolve(ctx, ROOT, &path)
            .and_then(|source| drafted(ctx, Some(source)));
        let Err(error) = opened else { continue };
        if index == 0 && error == Error::NoSpace {
            return Err(Fail::Big(String::from(*file)));
        }
        if index > 0 {
            warned(ctx, &format!("open {file}: {}", Fault::Kernel(error)))?;
        }
    }
    Ok(())
}

/// Capture the config directory's startup file before any Plugin starts.
fn captured_config(ctx: &dyn Calls) -> Result<Option<String>, Error> {
    let Ok(directory) = reovim_host::dirs::config() else { return Ok(None) };
    let path = format!("{directory}/reovim/init");
    match captured(&path) {
        Ok(text) => Ok(text),
        Err(cause) => {
            warned(ctx, &format!("init: {path}: {cause}"))?;
            Ok(None)
        }
    }
}

/// One read to EOF, decoded whole before the caller can run any line.
fn captured(path: &str) -> Result<Option<String>, &'static str> {
    let bytes = match reovim_host::fs::read_all(path) {
        Ok(bytes) => bytes,
        Err(reovim_host::Error::NotFound) => return Ok(None),
        Err(_) => return Err("unreadable"),
    };
    String::from_utf8(bytes).map(Some).map_err(|_| "not UTF-8")
}

/// `FILE[:LOCATOR]` as the editor opens it: the whole name while a
/// file of it is there, so `a:b` on disk opens `a:b`, else the path
/// [`split`] names while a file of *that* is there, and the rest with
/// it. A name no file holds either way is whole, and is the new file
/// it asks for.
///
/// The rest is handed on unread: which locators mean anything is the
/// view's, not this binary's, and a new domain adds a view, never a
/// line here.
fn located<'a>(ctx: &dyn Calls, file: &'a str) -> (&'a str, &'a str) {
    let there = |name: &str| {
        cell::resolve(ctx, ROOT, &format!("/module/fs/root/{name}")).is_ok()
    };
    if there(file) {
        return (file, "");
    }
    match split(file) {
        (path, Some(locator)) if there(path) => (path, locator),
        _ => (file, ""),
    }
}

/// A startup failure in the shared diagnostic stream.
fn warned(ctx: &dyn Calls, text: &str) -> Result<(), Error> {
    let entry = cell::resolve(ctx, ROOT, "/sys/log")?;
    ops::put(ctx, entry, format!("<4>{text}").into_bytes()).map(|_| ())
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
    let Frame { lines, cursor, shape: wanted, status, runs } = &frame;
    let wanted = *wanted;
    let text_rows = usize::from(rows.saturating_sub(1));
    let blank = String::new();
    let rest =
        std::iter::repeat_n(&blank, text_rows.saturating_sub(lines.len()));
    let mut screen = String::from("\x1b[?25l\x1b[H");
    let none = Vec::new();
    for (row, line) in lines.iter().chain(rest).enumerate() {
        let tuples = tupled(runs.get(row).unwrap_or(&none));
        let shown = look::painted(line, &tuples);
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

/// `runs`, already looks and already bytes, as [`look::painted`] takes
/// them; the viewer's own painting reuses this too.
pub fn tupled(runs: &[display::Run]) -> Vec<(u64, u64, u64)> {
    runs.iter().map(|run| (run.from, run.to, run.style)).collect()
}

/// Bytes to stdout, flushed; the viewer's own drawing reuses this too.
pub fn out(bytes: &[u8]) -> Result<(), Fault> {
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
        term::Key::Alt(c) => Key::Alt(c),
    }
}

#[cfg(test)]
mod tests {
    use reovim_client_api::display;
    use reovim_sdk::{look, sgr};

    use super::tupled;

    /// A row's runs, already resolved to looks as `on_row` leaves
    /// them: what the app paints from, and no more.
    fn run(from: u64, to: u64, meaning: u64) -> display::Run {
        display::Run { from, to, style: sgr::look(meaning) }
    }

    #[test]
    fn every_run_of_a_row_is_drawn_in_the_look_it_holds() {
        let runs = vec![run(0, 3, sgr::BLUE), run(5, 7, sgr::GREEN)];
        let drawn = look::painted("bin  ls", &tupled(&runs));
        assert_eq!(drawn, "\x1b[1;34mbin\x1b[0m  \x1b[32mls\x1b[0m");
    }

    #[test]
    fn a_run_past_the_text_shows_that_byte_as_a_space() {
        let runs = vec![run(0, 4, sgr::SELECTION)];
        assert_eq!(look::painted("abc", &tupled(&runs)), "\x1b[7mabc \x1b[0m");
    }

    use reovim_client::dev;
    use reovim_client::{Calls, Rd};
    use reovim_sdk::decode;
    use reovim_sdk::log;
    use reovim_sdk::walk::ROOT;
    use reovim_sdk::{cell, ops};

    use super::{clip, drafts, located, table};
    use crate::client;
    use crate::local::Local;
    use crate::server;

    #[test]
    fn a_capture_is_the_text_or_why_not() {
        let dir = std::env::temp_dir()
            .join(format!("reovim-capture-{}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        let file = dir.join("init");
        let path = file.to_str().unwrap();
        assert_eq!(super::captured(path), Ok(None));
        std::fs::write(&file, b"set ic\n").unwrap();
        assert_eq!(super::captured(path), Ok(Some("set ic\n".into())));
        std::fs::write(&file, [0xff]).unwrap();
        assert_eq!(super::captured(path), Err("not UTF-8"));
        assert_eq!(super::captured(dir.to_str().unwrap()), Err("unreadable"));
        std::fs::remove_dir_all(dir).unwrap();
    }

    /// `a`, `b` and a directory `d` on disk, `missing` not: `drafts`
    /// over the four, the ask (`dev::boot`'s own `file`) among them,
    /// makes two, in order, and says the other two once each, but not
    /// the first, which is the ask's own case.
    #[test]
    fn the_files_are_drafts_in_the_order_given() {
        let dir = std::env::temp_dir()
            .join(format!("reovim-paths-{}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(dir.join("a"), "alpha").unwrap();
        std::fs::write(dir.join("b"), "beta").unwrap();
        std::fs::create_dir(dir.join("d")).unwrap();
        let root = dir.to_str().unwrap();

        let Ok((kernel, _modules)) = server::boot(root, &[]) else {
            panic!("the server did not boot")
        };
        let client = Local::client(&kernel, None).unwrap();
        let Ok(host) = client::over(client) else { panic!("no host") };
        dev::boot(&host, table(None), clip(), "", Some("a"), None, "").unwrap();

        let own = host.own();
        let ctx: &dyn Calls = &own;
        let Ok(()) = drafts(ctx, &["a", "b", "missing", "d"]) else {
            panic!("a name the opening should pass over stopped it")
        };

        let list = cell::resolve(ctx, ROOT, "/self/dev/buffers").unwrap();
        let made = ops::children(ctx, list).unwrap();
        assert_eq!(made.len(), 2, "a and b are on disk, missing and d not");
        let a = cell::resolve(ctx, ROOT, "/module/fs/root/a").unwrap();
        let b = cell::resolve(ctx, ROOT, "/module/fs/root/b").unwrap();
        let origin = |draft: Rd| -> Rd {
            let source = cell::resolve(ctx, draft, "source").unwrap();
            decode(&ops::bytes(ctx, source).unwrap()).unwrap()
        };
        let over: Vec<Rd> = made.iter().map(|&draft| origin(draft)).collect();
        assert_eq!(over, [a, b], "a and b, in the order given");

        let entry = cell::resolve(ctx, ROOT, "/sys/log").unwrap();
        let bytes = ops::bytes(ctx, entry).unwrap();
        let said: Vec<String> =
            log::parse(&bytes).iter().map(log::Entry::text).collect();
        let holds = |text: &str| said.iter().any(|line| line.contains(text));
        assert!(holds("open missing:"), "the log: {said:?}");
        assert!(holds("open d:"), "the log: {said:?}");
        assert!(!holds("open a:"), "the log: {said:?}");

        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// The first file having no room in the Buffers: the opening
    /// stops there and the word names that file, which `no space` on
    /// its own does not. Behind a name that opened, the same file is
    /// skipped and said, as any other name the open refuses.
    #[test]
    fn a_first_file_past_what_the_editor_holds_is_named() {
        let dir = std::env::temp_dir()
            .join(format!("reovim-too-big-{}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(dir.join("a"), "alpha").unwrap();
        // Sparse: no byte of it is written, and the read of it is
        // what a Buffer of 64 MiB refuses.
        let big = std::fs::File::create(dir.join("big")).unwrap();
        big.set_len(65 << 20).unwrap();
        drop(big);
        let root = dir.to_str().unwrap();

        let Ok((kernel, _modules)) = server::boot(root, &[]) else {
            panic!("the server did not boot")
        };
        let client = Local::client(&kernel, None).unwrap();
        let Ok(host) = client::over(client) else { panic!("no host") };
        let table = table(None);
        dev::boot(&host, table, clip(), "", Some("big"), None, "").unwrap();

        let own = host.own();
        let ctx: &dyn Calls = &own;
        let Err(fail) = drafts(ctx, &["big"]) else {
            panic!("a file the editor cannot hold was opened")
        };
        assert_eq!(fail.to_string(), "big: more than the editor holds");

        let Ok(()) = drafts(ctx, &["a", "big"]) else {
            panic!("a name behind one that opened stopped the opening")
        };
        let entry = cell::resolve(ctx, ROOT, "/sys/log").unwrap();
        let said = ops::bytes(ctx, entry).unwrap();
        let said: Vec<String> =
            log::parse(&said).iter().map(log::Entry::text).collect();
        let holds = |text: &str| said.iter().any(|line| line.contains(text));
        assert!(holds("open big: no space"), "the log: {said:?}");

        std::fs::remove_dir_all(&dir).unwrap();
    }

    /// `FILE:LOCATOR` as the app reads it, which is as little as it
    /// can: the whole name while a file of it is there, else the path
    /// before the first colon while that is, and the rest handed on
    /// unread.
    #[test]
    fn a_name_splits_only_where_no_file_holds_it_whole() {
        let dir = std::env::temp_dir()
            .join(format!("reovim-locator-{}", std::process::id()));
        std::fs::create_dir(&dir).unwrap();
        std::fs::write(dir.join("README.md"), "one\n").unwrap();
        std::fs::write(dir.join("a:b"), "colon\n").unwrap();
        // `a` too, so `a:b` is a name the split could take apart.
        std::fs::write(dir.join("a"), "plain\n").unwrap();
        let root = dir.to_str().unwrap();

        let Ok((kernel, _modules)) = server::boot(root, &[]) else {
            panic!("the server did not boot")
        };
        let client = Local::client(&kernel, None).unwrap();
        let Ok(host) = client::over(client) else { panic!("no host") };
        let own = host.own();
        let ctx: &dyn Calls = &own;

        assert_eq!(located(ctx, "README.md:12"), ("README.md", "12"));
        assert_eq!(located(ctx, "README.md:12:5"), ("README.md", "12:5"));
        assert_eq!(located(ctx, "a:b"), ("a:b", ""), "the whole name wins");
        assert_eq!(located(ctx, "README.md"), ("README.md", ""));
        assert_eq!(located(ctx, "x:"), ("x:", ""), "an empty rest");
        assert_eq!(located(ctx, "new:file"), ("new:file", ""), "no path");

        std::fs::remove_dir_all(&dir).unwrap();
    }
}
