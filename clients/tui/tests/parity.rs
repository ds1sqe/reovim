//! The corpus under tests/parity, each case what nvim did with its
//! keys on its text (recorded by record.sh), replayed through the tui
//! on the rig: the file after and the exit code must be nvim's.

use std::path::Path;

use reovim_client_tui::{Step, Tui};
use reovim_client_vim::notation::keys;
use reovim_rig::Rig;

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
    let line = String::from_utf8(read("keys")?).unwrap();
    let keys =
        keys(line.trim_end()).ok_or_else(|| format!("{name}: bad notation"))?;

    let rig = Rig::new(name);
    rig.file("file", &input);
    let mut tui = Tui::open(rig.client(), "file").unwrap();
    let mut quit = None;
    for key in keys {
        let step = tui.key(key).map_err(|fault| format!("{name}: {fault}"))?;
        if let Step::Quit(code) = step {
            quit = Some(code);
            break;
        }
    }
    let code = quit.ok_or_else(|| format!("{name}: the keys did not quit"))?;
    let file = rig.disk("file");
    if (code, &file) == (exit, &out) {
        return Ok(());
    }
    let (ours, theirs) =
        (String::from_utf8_lossy(&file), String::from_utf8_lossy(&out));
    Err(format!(
        "{name}: exit {code}, nvim {exit}\n\
         --- ours ---\n{ours}--- nvim ---\n{theirs}"
    ))
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
