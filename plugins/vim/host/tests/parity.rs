//! The corpus under the engine crate's tests/parity, each case what
//! nvim did with its keys on its text (recorded by record.sh),
//! replayed through the tui on the rig: the file after, the exit
//! code, the cursor and the unnamed register must be nvim's. A case
//! that holds a `count` asks one thing more: the row's two numbers,
//! which are nvim's `searchcount()` at exit.

use std::fmt::Write as _;
use std::path::Path;

use reovim_client::dev::{self, Clip, Table};
use reovim_client_tui::{Step, Tui};
use reovim_plugin_vim::notation::keys;
use reovim_plugin_vim::{Kind, Register};
use reovim_plugin_vim_host::Editor;
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
fn state_of(vim: &Editor) -> (String, String, Vec<u8>) {
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
    let mut bytes = text.into_bytes();
    if kind == "V" {
        bytes.push(b'\n');
    }
    (format!("{}:{}", line + 1, column + 1), kind, bytes)
}

/// What a case that holds a `count` was recorded saying: nvim's
/// `searchcount()` at exit, `current/total`.
fn counted(case: &Path) -> Option<String> {
    let bytes = std::fs::read(case.join("count")).ok()?;
    Some(String::from_utf8_lossy(&bytes).trim().into())
}

/// The same two numbers off the engine. A list that is not whole has
/// none, and the case fails on it: a case whose keys leave the fill
/// half done is no comparison with nvim, which blocks.
fn count_of(vim: &Editor) -> String {
    vim.counted().map_or_else(
        || String::from("none, the list not being whole"),
        |(current, total)| format!("{current}/{total}"),
    )
}

/// The other files a case carries, `in.NAME` beside `in`: `NAME`, in
/// the order `record.sh` writes `out.NAME`.
fn others(case: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(case)
        .unwrap()
        .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
        .filter_map(|name| name.strip_prefix("in.").map(String::from))
        .collect();
    names.sort();
    names
}

/// One case: the keys run to their quit, `file` and every `NAME` a
/// second file, and the code after.
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
    let count = counted(case);
    let others = others(case);
    let expected = others
        .iter()
        .map(|other| read(&format!("out.{other}")))
        .collect::<Result<Vec<Vec<u8>>, _>>()?;

    let rig = Rig::new(name);
    rig.file("file", &input);
    for other in &others {
        rig.file(other, &read(&format!("in.{other}"))?);
    }
    let vim = Editor::new();
    let one = vim.clone();
    let table: Table =
        Box::new(move |_| Some((String::from("vim"), one.booted())));
    let host = rig.searching(None);
    let dev = dev::boot(&host, table, Clip::none(), "", Some("file"), None, "")
        .map_err(|error| format!("{name}: {error:?}"))?;
    let mut tui = Tui::open(host, dev).map_err(|e| format!("{name}: {e:?}"))?;
    let mut quit = None;
    for key in keys {
        let step =
            tui.key(key).map_err(|fault| format!("{name}: {fault:?}"))?;
        // No frame of a corpus case ends with a search still waiting:
        // every case here is over a text one slice covers, and a case
        // whose end state depended on a later frame would be no
        // comparison with nvim, which blocks.
        if vim.waiting() {
            return Err(format!("{name}: a frame ended waiting"));
        }
        if let Step::Quit(code) = step {
            quit = Some(code);
            break;
        }
    }
    let code = quit.ok_or_else(|| format!("{name}: the keys did not quit"))?;
    let file = rig.disk("file");
    let ours = state_of(&vim);
    let ours_count = count.as_ref().map(|_| count_of(&vim));
    let theirs_of: Vec<Vec<u8>> =
        others.iter().map(|other| rig.disk(other)).collect();
    let matched = (code, &file, &ours) == (exit, &out, &theirs)
        && theirs_of == expected
        && ours_count == count;
    if matched {
        return Ok(());
    }
    let show = |bytes: &[u8]| String::from_utf8_lossy(bytes).into_owned();
    let mut report = format!("{name}: exit {code}, nvim {exit}");
    if file != out {
        let (a, b) = (show(&file), show(&out));
        let _ = write!(report, "\n--- ours ---\n{a}--- nvim ---\n{b}");
    }
    for ((other, ours), theirs) in others.iter().zip(&theirs_of).zip(&expected)
    {
        if ours != theirs {
            let (a, b) = (show(ours), show(theirs));
            let _ =
                write!(report, "\n--- ours {other} ---\n{a}--- nvim ---\n{b}");
        }
    }
    if ours.0 != theirs.0 {
        let _ = write!(report, "\ncursor {}, nvim {}", ours.0, theirs.0);
    }
    if let (Some(ours), Some(theirs)) = (&ours_count, &count)
        && ours != theirs
    {
        let _ = write!(report, "\ncount {ours}, nvim {theirs}");
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
    // The corpus is the engine crate's own directory still: every
    // case recorded from nvim, which no split of the code touches,
    // and which apps/reovim/parity.sh reads from the same place.
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../tests/parity");
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
