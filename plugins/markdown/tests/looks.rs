//! The markdown Plugin over a README of the test's own, shown through
//! the tui with Vim holding the region.
//!
//! What a case judges is the runs the display carries: the construct's
//! meaning over its text, and its markers gone off the cursor's row and
//! back on it. What a key means is Vim's and its own corpus judges it;
//! how a look is composed is the tui's and clients/tui judges that.

use std::rc::Rc;

use reovim_client::dev::{self, Clip, Table, Want};
use reovim_client::{Host, ROOT};
use reovim_client_api::display;
use reovim_client_tui::Tui;
use reovim_kernel_interface::Error;
use reovim_plugin_vim_host::Editor;
use reovim_rig::{Rig, frame, listed, opened, typed};
use reovim_sdk::{cell, ops, sgr};

/// A rig with the markdown Plugin loaded and the tui over `file`, Vim
/// holding the region as the app gives it a cell.
struct Scene {
    rig: Rig,
    host: Rc<Host>,
    tui: Tui,
}

/// The table the app has in the small: Vim for a cell and nothing else,
/// this Plugin serving no region of its own.
fn table() -> Table {
    let editor = Editor::new();
    Box::new(move |want| match want {
        Want::Cell => Some((String::from("vim"), editor.booted())),
        Want::Container | Want::Named(_) => None,
    })
}

/// The Plugin loaded under `/self/markdown`, as the app loads it at
/// start.
fn loaded(host: &Rc<Host>) {
    let booted = reovim_plugin_markdown::boot();
    host.load("markdown", booted.module, booted.drivers).unwrap();
}

impl Scene {
    /// A rig holding `file` with `text`, the Plugin loaded, and the tui
    /// opened over that file.
    fn new(name: &str, file: &str, text: &str) -> Self {
        let rig = Rig::new(name);
        rig.file(file, text.as_bytes());
        let host = rig.host();
        loaded(&host);
        let tui =
            opened(&host, table(), Clip::none(), "", Some(file), None).unwrap();
        Self { rig, host, tui }
    }

    /// The rows the frame shows.
    fn lines(&mut self) -> Vec<String> {
        frame(&mut self.tui).lines
    }

    /// The runs of row `row`, in the bytes that row is drawn in.
    fn runs(&mut self, row: usize) -> Vec<display::Run> {
        frame(&mut self.tui).runs[row].clone()
    }

    /// Those keys, in Vim's notation.
    fn go(&mut self, keys: &str) {
        typed(&mut self.tui, keys);
    }

    /// What `/self/markdown` answers for the name `name`.
    fn asked(&self, name: &str) -> Result<(), Error> {
        let ctx = self.host.own();
        let home = cell::resolve(&ctx, ROOT, "/self/markdown").unwrap();
        ops::lookup(&ctx, home, name).map(|_| ())
    }
}

/// The run a meaning draws as, in the bytes of the row it is on.
fn drawn(from: u64, to: u64, meaning: u64) -> display::Run {
    display::Run { from, to, style: sgr::look(meaning) }
}

/// A README of two lines, the second the one under test: the cursor
/// starts on the first, so the second is a row the cursor is off.
fn over(name: &str, second: &str) -> Scene {
    Scene::new(name, "README.md", &format!("plain\n{second}\n"))
}

#[test]
fn a_heading_is_the_heading_look() {
    let mut s = over("md-heading", "# Heading");
    assert_eq!(s.lines()[1], "Heading", "the hashes are markers");
    assert_eq!(s.runs(1), [drawn(0, 7, sgr::HEADING)]);
    s.go("j");
    assert_eq!(s.lines()[1], "# Heading", "and they are back under it");
    assert_eq!(s.runs(1), [drawn(2, 9, sgr::HEADING)]);
}

#[test]
fn strong_and_emphasis() {
    let mut s = over("md-strong", "**bold** and *soft*");
    assert_eq!(s.lines()[1], "bold and soft");
    let off = [drawn(0, 4, sgr::STRONG), drawn(9, 13, sgr::EMPHASIS)];
    assert_eq!(s.runs(1), off);
    s.go("j");
    assert_eq!(s.lines()[1], "**bold** and *soft*");
    let on = [drawn(2, 6, sgr::STRONG), drawn(14, 18, sgr::EMPHASIS)];
    assert_eq!(s.runs(1), on);
}

#[test]
fn a_code_span() {
    let mut s = over("md-code", "use `cargo test` now");
    assert_eq!(s.lines()[1], "use cargo test now");
    assert_eq!(s.runs(1), [drawn(4, 14, sgr::CODE)]);
    s.go("j");
    assert_eq!(s.lines()[1], "use `cargo test` now");
    assert_eq!(s.runs(1), [drawn(5, 15, sgr::CODE)]);
}

#[test]
fn a_link() {
    let mut s = over("md-link", "see [the docs](http://x) now");
    assert_eq!(s.lines()[1], "see the docs now", "the url is a marker");
    assert_eq!(s.runs(1), [drawn(4, 12, sgr::LINK)]);
    s.go("j");
    assert_eq!(s.lines()[1], "see [the docs](http://x) now");
    assert_eq!(s.runs(1), [drawn(5, 13, sgr::LINK)]);
}

#[test]
fn a_quote() {
    let mut s = over("md-quote", "> quoted");
    assert_eq!(s.lines()[1], "quoted");
    assert_eq!(s.runs(1), [drawn(0, 6, sgr::QUOTE)]);
    s.go("j");
    assert_eq!(s.lines()[1], "> quoted");
    assert_eq!(s.runs(1), [drawn(2, 8, sgr::QUOTE)]);
}

#[test]
fn a_rule() {
    let mut s = over("md-rule", "---");
    assert_eq!(s.lines()[1], "---", "a rule is all text and no marker");
    assert_eq!(s.runs(1), [drawn(0, 3, sgr::RULE)]);
    s.go("j");
    assert_eq!(s.lines()[1], "---");
    assert_eq!(s.runs(1), [drawn(0, 3, sgr::RULE)]);
}

#[test]
fn a_fence_is_tinted_and_nothing_inside_is_parsed() {
    let text = "plain\n```rust\nlet x = **not bold**;\n```\n";
    let mut s = Scene::new("md-fence", "README.md", text);
    let shown = ["plain", "rust", "let x = **not bold**;", ""];
    assert_eq!(s.lines()[..4], shown, "the fences close up, the tint stays");
    assert_eq!(s.runs(1), [drawn(0, 4, sgr::FENCE)], "the language");
    let inside = [drawn(0, 21, sgr::FENCE)];
    assert_eq!(s.runs(2), inside, "no strong inside a fence");
    assert_eq!(s.runs(3), Vec::new(), "the closing line is its marker");
    s.go("j");
    assert_eq!(s.lines()[1], "```rust");
    // The marker stands over the tint, so the row the cursor is on
    // shows the backticks in no look at all, as the drawer has it.
    assert_eq!(s.runs(1), [drawn(3, 7, sgr::FENCE)]);
}

#[test]
fn a_draft_that_is_no_markdown_has_no_looks() {
    let mut s = Scene::new("md-plain", "a.txt", "plain\n**bold**\n");
    s.rig.file("b.markdown", b"plain\n**bold**\n");
    assert_eq!(s.lines()[1], "**bold**", "the bytes stand as they are");
    assert_eq!(s.runs(1), Vec::new());
    assert_eq!(s.asked("0"), Err(Error::Invalid), "no draft of ours");
    // The long suffix is Markdown too, and the same draft's neighbour.
    s.go(":e b.markdown<CR>");
    assert_eq!(s.lines()[1], "bold", "and this one is ours");
}

#[test]
fn the_child_comes_when_asked() {
    let rig = Rig::new("md-comes");
    rig.file("README.md", b"# Heading\n");
    let host = rig.host();
    loaded(&host);
    let dev = dev::boot(
        &host,
        table(),
        Clip::none(),
        "",
        Some("README.md"),
        None,
        "",
    )
    .unwrap();
    assert_eq!(listed(&host, "/self/markdown"), Vec::<String>::new());
    let mut tui = Tui::open(host.clone(), dev).unwrap();
    frame(&mut tui);
    assert_eq!(listed(&host, "/self/markdown"), ["0"], "the walk made it");
}

#[test]
fn a_draft_that_goes_takes_its_child() {
    let mut s = over("md-gone", "# Heading");
    s.rig.file("b.md", b"# Second\n");
    s.rig.file("c.md", b"# Third\n");
    s.lines();
    s.go(":e b.md<CR>");
    s.lines();
    assert_eq!(listed(&s.host, "/self/markdown"), ["0", "1"]);
    s.go(":bd 0<CR>:e c.md<CR>");
    s.lines();
    assert_eq!(listed(&s.host, "/self/markdown"), ["1", "2"], "0 went");
}

#[test]
fn a_save_writes_the_bytes_unchanged() {
    let text = "# Heading\n\n**bold** and `code`\n\n```rust\nlet x = 1;\n```\n";
    let mut s = Scene::new("md-save", "README.md", text);
    s.lines();
    s.go(":w<CR>");
    assert_eq!(s.rig.disk("README.md"), text.as_bytes(), "a look edits none");
}
