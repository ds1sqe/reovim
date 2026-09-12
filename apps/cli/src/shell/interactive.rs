//! Hosted terminal editing; history is local to this interactive process.

use super::Shell;
use rustyline::{
    CompletionType, Config, Editor, error::ReadlineError, history::MemHistory,
};
use std::io;

pub fn run(shell: Shell) -> io::Result<()> {
    let config = Config::builder()
        .max_history_size(100)
        .map_err(io::Error::other)?
        .completion_type(CompletionType::List)
        .build();
    let mut editor = Editor::<Shell, MemHistory>::with_config(config)
        .map_err(io::Error::other)?;
    editor.set_helper(Some(shell));
    loop {
        let shell = editor.helper().expect("interactive client");
        if !shell.running {
            return Ok(());
        }
        let prompt = shell.prompt();
        match editor.readline(&prompt) {
            Ok(line) => execute(&mut editor, &line)?,
            Err(ReadlineError::Interrupted) => (),
            Err(ReadlineError::Eof) => return Ok(()),
            Err(error) => return Err(io::Error::other(error)),
        }
    }
}

fn execute(
    editor: &mut Editor<Shell, MemHistory>,
    line: &str,
) -> io::Result<()> {
    if line.len() <= 16 * 1024 && !line.trim().is_empty() {
        editor.add_history_entry(line).map_err(io::Error::other)?;
    }
    let shell = editor.helper_mut().expect("interactive client");
    let fields: Vec<_> = line.split_whitespace().collect();
    if let Err(error) = shell.execute(&fields) {
        eprintln!("{error}");
    }
    Ok(())
}
