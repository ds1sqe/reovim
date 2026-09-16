mod requests;
mod shell;

use std::{
    io::{self, BufRead, IsTerminal, Read, Write},
    path::Path,
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let address = args.next().ok_or("expected ADDRESS TOKEN JOURNAL")?;
    let token = reovim_service::unhex(&args.next().ok_or("missing token")?)
        .and_then(|bytes| bytes.try_into().ok())
        .ok_or("invalid token")?;
    let journal = args.next().ok_or("missing journal path")?;
    let mut shell = shell::Shell::new(requests::Client::open(
        &address,
        token,
        Path::new(&journal),
    )?);
    let fields: Vec<_> = args.collect();
    if !fields.is_empty() {
        shell
            .execute(&fields.iter().map(String::as_str).collect::<Vec<_>>())?;
        return Ok(());
    }
    if io::stdin().is_terminal() {
        return interactive(&mut shell).map_err(Into::into);
    }
    println!("ready");
    io::stdout().flush()?;
    let mut input = io::stdin().lock();
    while let Some(line) = read_line(&mut input)? {
        if !execute(&mut shell, &line)? {
            break;
        }
    }
    Ok(())
}
fn execute(shell: &mut shell::Shell, line: &str) -> io::Result<bool> {
    check_line(line)?;
    let result = shell.execute(&line.split_whitespace().collect::<Vec<_>>());
    match result {
        Ok(running) => {
            println!("ok");
            io::stdout().flush()?;
            Ok(running)
        }
        Err(error) => {
            eprintln!("{error}");
            println!("failed");
            io::stdout().flush()?;
            Ok(true)
        }
    }
}
fn interactive(shell: &mut shell::Shell) -> io::Result<()> {
    let config = rustyline::Config::builder()
        .max_history_size(100)
        .map_err(io::Error::other)?
        .build();
    let mut editor = rustyline::DefaultEditor::with_config(config)
        .map_err(io::Error::other)?;
    loop {
        let line = match editor.readline("resource> ") {
            Ok(line) => {
                check_line(&line)?;
                editor.add_history_entry(&line).map_err(io::Error::other)?;
                line
            }
            Err(rustyline::error::ReadlineError::Interrupted) => continue,
            Err(rustyline::error::ReadlineError::Eof) => return Ok(()),
            Err(error) => return Err(io::Error::other(error)),
        };
        if !execute(shell, &line)? {
            return Ok(());
        }
    }
}

const MAX_INPUT: usize = 16 * 1024;
fn check_line(line: &str) -> io::Result<()> {
    if line.len() > MAX_INPUT {
        return Err(io::Error::other("input exceeds 16 KiB"));
    }
    Ok(())
}
fn read_line(input: &mut impl BufRead) -> io::Result<Option<String>> {
    let mut line = String::new();
    let count = input.take(MAX_INPUT as u64 + 1).read_line(&mut line)?;
    check_line(&line)?;
    Ok((count != 0).then_some(line))
}
