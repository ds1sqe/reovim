//! Completion queries names only; it never reads info or invokes a provider.

use super::Shell;
use rustyline::{
    Context, Helper,
    completion::{Completer, Pair},
    highlight::Highlighter,
    hint::Hinter,
    validate::Validator,
};
use std::{collections::BTreeSet, io};

impl Helper for Shell {}
impl Highlighter for Shell {}
impl Validator for Shell {}
impl Hinter for Shell {
    type Hint = String;
}

impl Completer for Shell {
    type Candidate = Pair;

    fn complete(
        &self,
        line: &str,
        pos: usize,
        _: &Context<'_>,
    ) -> rustyline::Result<(usize, Vec<Pair>)> {
        let head = &line[..pos];
        let start = head
            .char_indices()
            .rev()
            .find(|(_, c)| c.is_whitespace())
            .map_or(0, |(p, c)| p + c.len_utf8());
        let token = &head[start..];
        let words: Vec<_> = head[..start].split_whitespace().collect();
        let result = self.candidates(token, command_position(&words));
        match result {
            Ok(values) => Ok((start, values)),
            Err(error) => {
                eprintln!("\r\ncompletion: {error}\r");
                Ok((start, Vec::new()))
            }
        }
    }
}

fn command_position(mut words: &[&str]) -> bool {
    while let Some(flag) = words.first() {
        let count = match *flag {
            "--contract" => 3,
            "--hex" | "--refs" => 2,
            _ => return false,
        };
        if words.len() < count {
            return false;
        }
        words = &words[count..];
    }
    true
}

impl Shell {
    fn candidates(&self, token: &str, command: bool) -> io::Result<Vec<Pair>> {
        if token.starts_with(['@', '-']) {
            return Ok(Vec::new());
        }
        if command && !token.contains('/') {
            return self.command_candidates(token);
        }
        let (prefix, name) =
            token.rsplit_once('/').map_or(("", token), |(parent, name)| {
                (&token[..=parent.len()], name)
            });
        let cwd =
            if token.starts_with('/') { String::new() } else { self.cwd()? };
        let directory = reovim_call::path(&cwd, prefix);
        let names = self.names(&directory)?;
        Ok(matches(names, name, prefix))
    }

    fn command_candidates(&self, prefix: &str) -> io::Result<Vec<Pair>> {
        let mut names = BTreeSet::new();
        for directory in self.directories()? {
            match self.names(&directory) {
                Ok(values) => names.extend(values),
                Err(error) if error.kind() == io::ErrorKind::NotFound => (),
                Err(error) => return Err(error),
            }
            capacity(names.len())?;
        }
        Ok(matches(names.into_iter().collect(), prefix, ""))
    }

    fn names(&self, directory: &str) -> io::Result<Vec<String>> {
        let reply =
            self.exchange(&["ls".into(), reovim_call::path("", directory)])?;
        listing(&reply)
    }
}

fn capacity(size: usize) -> io::Result<()> {
    if size > 256 {
        return Err(io::Error::other("completion candidate limit"));
    }
    Ok(())
}

fn listing(reply: &str) -> io::Result<Vec<String>> {
    let fields = reply
        .strip_prefix("ok\t")
        .ok_or_else(|| io::Error::other("invalid listing reply"))?;
    let mut names = Vec::new();
    for entry in fields.split('\t').filter(|entry| !entry.is_empty()) {
        let (name, _) = entry
            .rsplit_once("=@")
            .ok_or_else(|| io::Error::other("invalid listing entry"))?;
        if !name.chars().any(char::is_whitespace)
            && !name.chars().any(char::is_control)
        {
            names.push(name.into());
        }
        capacity(names.len())?;
    }
    Ok(names)
}

fn matches(names: Vec<String>, token: &str, prefix: &str) -> Vec<Pair> {
    names
        .into_iter()
        .filter(|name| name.starts_with(token))
        .map(|name| Pair {
            display: name.clone(),
            replacement: format!("{prefix}{name}"),
        })
        .collect()
}
