//! Tab completion: commands, targets from where the line stands, the
//! flags a word takes, method names from the declarations. Candidates
//! come from the tree; a path that resolves nowhere offers nothing and
//! is not a fault.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use reovim_client::{Flags, Rd, Tree, log};

use crate::{Shell, Words, locate};

/// What may replace the word starting at byte `from` of the line.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Completion {
    pub from: usize,
    pub candidates: Vec<String>,
}

/// The flags each word takes, for a word that begins with `-`.
const FLAGS: [(&str, &[&str]); 4] = [
    ("ls", &["-l", "-a", "-w"]),
    ("cat", &["-f", "-n", "-w"]),
    ("tree", &["-L"]),
    (
        "dmesg",
        &[
            "-t", "-T", "-H", "-x", "-d", "-r", "-k", "-w", "-W", "-f", "-l",
            "-L", "--color",
        ],
    ),
];

/// Candidates for the last word of `line`, which may be empty, from
/// where `shell` stands.
pub fn complete(tree: &mut Tree, shell: &Shell, line: &str) -> Completion {
    let from = line.rfind(' ').map_or(0, |at| at + 1);
    let word = &line[from..];
    let index = line[..from].split_whitespace().count();
    let command = line.split_whitespace().next().unwrap_or_default();
    let before = line[..from].split_whitespace().last();
    let candidates = match (command, index) {
        (_, 0) => commands(word),
        _ if word.starts_with('-') => flags(command, word),
        ("dmesg", _) => flag_values(tree, before, word),
        ("log" | "load" | "unload", _) => Vec::new(),
        ("call", 2) => methods(tree, shell, line, word),
        // `ls`, `cat` and `tree` take targets past their flags.
        ("ls" | "cat" | "tree", _) | (_, 1) => path(tree, shell, word),
        _ => Vec::new(),
    };
    Completion { from, candidates }
}

/// The flags of `command` that begin as `word` does.
fn flags(command: &str, word: &str) -> Vec<String> {
    let Some((_, flags)) = FLAGS.iter().find(|(name, _)| *name == command)
    else {
        return Vec::new();
    };
    flags
        .iter()
        .filter(|flag| flag.starts_with(word))
        .map(ToString::to_string)
        .collect()
}

/// What a `dmesg` flag takes: the marks the log's own entries carry
/// after `-f`, the eight level names after `-l`.
fn flag_values(
    tree: &mut Tree,
    before: Option<&str>,
    word: &str,
) -> Vec<String> {
    let values = match before {
        Some("-f") => marks(tree),
        Some("-l") => log::LEVELS.iter().map(ToString::to_string).collect(),
        _ => Vec::new(),
    };
    values.into_iter().filter(|value| value.starts_with(word)).collect()
}

/// Every mark in the log now, each once, sorted.
fn marks(tree: &mut Tree) -> Vec<String> {
    let Ok(entries) = log::tail(tree, 1) else { return Vec::new() };
    let mut marks: Vec<String> = entries.into_iter().map(|e| e.mark).collect();
    marks.sort();
    marks.dedup();
    marks
}

/// The cli's words that begin as `prefix` does, in the table's order.
fn commands(prefix: &str) -> Vec<String> {
    Words::cli()
        .starting(prefix)
        .into_iter()
        .map(|word| word.name.to_string())
        .collect()
}

/// The children of the part before the last `/`, named with the rest;
/// without a `/`, the children of the cwd. A Container gets a trailing
/// `/`, so the next Tab goes on under it.
fn path(tree: &mut Tree, shell: &Shell, word: &str) -> Vec<String> {
    let (dir, prefix) = word
        .rfind('/')
        .map_or(("", word), |cut| (&word[..=cut], &word[cut + 1..]));
    let Ok(rd) = locate(tree, shell, dir) else { return Vec::new() };
    let Ok(children) = tree.children(rd) else { return Vec::new() };
    children
        .into_iter()
        .filter_map(|child| named(tree, child, prefix))
        .map(|(name, container)| {
            let end = if container { "/" } else { "" };
            format!("{dir}{name}{end}")
        })
        .collect()
}

/// The child's name when it starts with `prefix`, and whether it is a
/// Container.
fn named(tree: &mut Tree, child: Rd, prefix: &str) -> Option<(String, bool)> {
    let info = tree.info(child).ok()?;
    let name = info.name?;
    name.starts_with(prefix)
        .then(|| (name, info.flags.contains(Flags::CONTAINER)))
}

/// Method names the target of `call` offers.
fn methods(
    tree: &mut Tree,
    shell: &Shell,
    line: &str,
    word: &str,
) -> Vec<String> {
    let Some(target) = line.split_whitespace().nth(1) else {
        return Vec::new();
    };
    let Ok(rd) = locate(tree, shell, target) else { return Vec::new() };
    let Ok(decls) = tree.decls(rd) else { return Vec::new() };
    decls
        .into_iter()
        .flat_map(|(_, decl)| decl.methods)
        .map(|m| m.name)
        .filter(|name| name.starts_with(word))
        .collect()
}
