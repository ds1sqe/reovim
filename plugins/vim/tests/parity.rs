//! The corpus under tests/parity, each case what nvim did with its
//! keys on its text (recorded by record.sh), replayed through the tui
//! on the rig: the file after, the exit code, the cursor and the
//! unnamed register must be nvim's.

use std::fmt::Write as _;
use std::path::Path;
use std::rc::Rc;

use reovim_client::dev::{self, Table};
use reovim_client_tui::{Step, Tui};
use reovim_plugin_vim::notation::keys;
use reovim_plugin_vim::{Editor, Kind, Register, Vim};
use reovim_rig::Rig;

/// nvim's `state` file: the cursor as `line:column:cell` (the cell
/// being the screen column, which apps/reovim/parity.sh judges), the
/// register's type, then its bytes.
fn state(bytes: &[u8]) -> Option<(String, String, Vec<u8>)> {
    let text = String::from_utf8_lossy(bytes);
    let (cursor, rest) = text.split_once('\n')?;
    let (cursor, _) = cursor.rsplit_once(':')?;
    let (kind, register) = rest.split_once('\n')?;
    Some((cursor.into(), kind.into(), register.as_bytes().to_vec()))
}

/// What nvim would write for Vim's cursor and register: the register
/// as `getreg` gives it, a linewise one ending in its newline, a
/// block's type carrying its width, an empty type for a register
/// nothing has filled.
fn state_of(vim: &Vim) -> (String, String, Vec<u8>) {
    let (line, column) = vim.position();
    let Some(Register { text, kind }) = vim.register() else {
        return (format!("{}:{}", line + 1, column + 1), String::new(), vec![]);
    };
    let width = text.split('\n').map(|p| p.chars().count()).max();
    let kind = match kind {
        Kind::Line => "V".to_string(),
        Kind::Block => format!("\u{16}{}", width.unwrap_or(0)),
        Kind::Char => "v".to_string(),
    };
    let mut bytes = text.clone().into_bytes();
    if kind == "V" {
        bytes.push(b'\n');
    }
    (format!("{}:{}", line + 1, column + 1), kind, bytes)
}

/// One case: the keys run to their quit, the file and the code after.
fn replay(case: &Path) -> Result<(), String> {
    let name = case.file_name().unwrap().to_str().unwrap();
    let read = |file: &str| {
        std::fs::read(case.join(file))
            .map_err(|_| format!("{name}: no {file}; record it with record.sh"))
    };
    let (input, out) = (read("in")?, read("out")?);
    let exit: u8 =
        String::from_utf8_lossy(&read("exit")?).trim().parse().unwrap();
    let theirs = state(&read("state")?)
        .ok_or_else(|| format!("{name}: state is not nvim's"))?;
    let line = String::from_utf8(read("keys")?).unwrap();
    let keys =
        keys(line.trim_end()).ok_or_else(|| format!("{name}: bad notation"))?;

    let rig = Rig::new(name);
    rig.file("file", &input);
    let vim = Editor::new();
    let one = vim.clone();
    let table: Table =
        Box::new(move |_| Some((String::from("vim"), one.booted())));
    let host = Rc::new(rig.host());
    let dev = dev::boot(&host, table, "", Some("file"), None)
        .map_err(|error| format!("{name}: {error:?}"))?;
    let mut tui = Tui::open(host, dev).map_err(|e| format!("{name}: {e:?}"))?;
    let mut quit = None;
    for key in keys {
        let step =
            tui.key(key).map_err(|fault| format!("{name}: {fault:?}"))?;
        if let Step::Quit(code) = step {
            quit = Some(code);
            break;
        }
    }
    let code = quit.ok_or_else(|| format!("{name}: the keys did not quit"))?;
    let file = rig.disk("file");
    let ours = state_of(&vim.engine());
    if (code, &file, &ours) == (exit, &out, &theirs) {
        return Ok(());
    }
    let show = |bytes: &[u8]| String::from_utf8_lossy(bytes).into_owned();
    let mut report = format!("{name}: exit {code}, nvim {exit}");
    if file != out {
        let (a, b) = (show(&file), show(&out));
        let _ = write!(report, "\n--- ours ---\n{a}--- nvim ---\n{b}");
    }
    if ours.0 != theirs.0 {
        let _ = write!(report, "\ncursor {}, nvim {}", ours.0, theirs.0);
    }
    if (&ours.1, &ours.2) != (&theirs.1, &theirs.2) {
        let (a, b) = (show(&ours.2), show(&theirs.2));
        let (x, y) = (&ours.1, &theirs.1);
        let _ = write!(report, "\nregister {x:?} {a:?}, nvim {y:?} {b:?}");
    }
    Err(report)
}

#[test]
fn every_case_ends_as_nvim_ended() {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/parity");
    let mut cases: Vec<_> = std::fs::read_dir(&corpus)
        .unwrap()
        .map(|e| e.unwrap().path())
        .filter(|p| p.is_dir())
        .collect();
    cases.sort();
    let failed: Vec<String> =
        cases.iter().filter_map(|c| replay(c).err()).collect();
    assert!(
        failed.is_empty(),
        "{} of {} cases differ from nvim:\n\n{}",
        failed.len(),
        cases.len(),
        failed.join("\n")
    );
    assert!(cases.len() >= 10, "the corpus has {} cases", cases.len());
}
