//! Tab: the names a line may begin with, the flags the Driver it names
//! takes, and the names under the cwd. Candidates come from the tree; a
//! path that resolves nowhere offers nothing and is not a fault.

use alloc::string::{String, ToString};
use alloc::vec::Vec;

use reovim_kernel_interface::Calls;
use reovim_sdk::ops;

use crate::{BUILTINS, Shell, decode, run};

/// What may replace the token starting at byte `from` of the line.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Completion {
    pub from: usize,
    pub candidates: Vec<String>,
}

/// Candidates for the last token of `line`, which may be empty, from
/// where `shell` stands.
pub fn complete(ctx: &dyn Calls, shell: &Shell, line: &str) -> Completion {
    let from = line.rfind(' ').map_or(0, |at| at + 1);
    let token = &line[from..];
    let first = line[..from].split_whitespace().count() == 0;
    let candidates = if first {
        names(ctx, token)
    } else if let Some(letters) = token.strip_prefix('-') {
        dashes(ctx, line, letters)
    } else {
        ops::paths(ctx, shell.cwd, token)
    };
    Completion { from, candidates }
}

/// What a line may begin with: the names under PATH, then the builtins.
fn names(ctx: &dyn Calls, prefix: &str) -> Vec<String> {
    let under = run::names(ctx).into_iter();
    let builtins = BUILTINS.iter().map(|entry| entry.name.to_string());
    under.chain(builtins).filter(|name| name.starts_with(prefix)).collect()
}

/// The flag fields of the Driver the line names; a name PATH does not
/// hold offers nothing.
fn dashes(ctx: &dyn Calls, line: &str, letters: &str) -> Vec<String> {
    let Some(name) = line.split_whitespace().next() else {
        return Vec::new();
    };
    let Some(fields) = run::fields(ctx, name) else {
        return Vec::new();
    };
    let begins = |flag: &String| flag[1..].starts_with(letters);
    decode::dashes(&fields).into_iter().filter(begins).collect()
}
