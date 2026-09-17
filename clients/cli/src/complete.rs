//! Tab completion: commands, paths from the tree's children, method
//! names from the declarations. Candidates come from the tree; a path
//! that resolves nowhere offers nothing and is not a fault.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use reovim_client::{Client, Flags, Rd, log};

use crate::COMMANDS;

/// What may replace the word starting at byte `from` of the line.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Completion {
    pub from: usize,
    pub candidates: Vec<String>,
}

/// Candidates for the last word of `line`, which may be empty.
pub fn complete(client: &mut Client, line: &str) -> Completion {
    let from = line.rfind(' ').map_or(0, |at| at + 1);
    let word = &line[from..];
    let index = line[..from].split_whitespace().count();
    let command = line.split_whitespace().next().unwrap_or_default();
    let before = line[..from].split_whitespace().last();
    let candidates = match (command, index) {
        (_, 0) => commands(word),
        ("dmesg", _) => flag_values(client, before, word),
        ("log" | "load" | "unload", _) => Vec::new(),
        ("call", 2) => methods(client, line, word),
        // `ls` and `cat` take targets until the end of the line.
        ("ls" | "cat", _) | (_, 1) => path(client, word),
        _ => Vec::new(),
    };
    Completion { from, candidates }
}

/// What a `dmesg` flag takes: the marks the log's own entries carry
/// after `-f`, the eight level names after `-l`.
fn flag_values(
    client: &mut Client,
    before: Option<&str>,
    word: &str,
) -> Vec<String> {
    let values = match before {
        Some("-f") => marks(client),
        Some("-l") => log::LEVELS.iter().map(ToString::to_string).collect(),
        _ => Vec::new(),
    };
    values.into_iter().filter(|value| value.starts_with(word)).collect()
}

/// Every mark in the log now, each once, sorted.
fn marks(client: &mut Client) -> Vec<String> {
    let Ok(entries) = log::tail(client, 1) else { return Vec::new() };
    let mut marks: Vec<String> = entries.into_iter().map(|e| e.mark).collect();
    marks.sort();
    marks.dedup();
    marks
}

fn commands(prefix: &str) -> Vec<String> {
    COMMANDS
        .iter()
        .filter(|c| c.starts_with(prefix))
        .map(ToString::to_string)
        .collect()
}

/// The children of the part before the last `/`, named with the rest;
/// a Container gets a trailing `/`.
fn path(client: &mut Client, word: &str) -> Vec<String> {
    let Some(cut) = word.rfind('/') else { return Vec::new() };
    let (parent, prefix) = (&word[..=cut], &word[cut + 1..]);
    let Ok(rd) = client.resolve(parent) else { return Vec::new() };
    let Ok(children) = client.children(rd) else { return Vec::new() };
    children
        .into_iter()
        .filter_map(|child| named(client, child, prefix))
        .map(|(name, container)| {
            let end = if container { "/" } else { "" };
            format!("{parent}{name}{end}")
        })
        .collect()
}

/// The child's name when it starts with `prefix`, and whether it is a
/// Container.
fn named(
    client: &mut Client,
    child: Rd,
    prefix: &str,
) -> Option<(String, bool)> {
    let info = client.info(child).ok()?;
    let name = info.name?;
    name.starts_with(prefix)
        .then(|| (name, info.flags.contains(Flags::CONTAINER)))
}

/// Method names the target of `call` offers.
fn methods(client: &mut Client, line: &str, word: &str) -> Vec<String> {
    let Some(target) = line.split_whitespace().nth(1) else {
        return Vec::new();
    };
    let Ok(rd) = client.resolve(target) else { return Vec::new() };
    let Ok(decls) = client.decls(rd) else { return Vec::new() };
    decls
        .into_iter()
        .flat_map(|(_, decl)| decl.methods)
        .map(|m| m.name)
        .filter(|name| name.starts_with(word))
        .collect()
}
