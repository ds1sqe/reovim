//! The one path completion: the names that may stand where a partly
//! typed one does, read from the tree itself.
//!
//! The cli completes a target with it from where its `Shell` stands,
//! and a Plugin the argument of a word from where the Plugin works.
//! Neither knows a Module: these are the children of a Container,
//! whatever put them there.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;

use reovim_kernel_interface::{Flags, Rd};

use crate::client::{ROOT, Tree};
use crate::fault::Fault;

/// The names that may replace `word`, in name order.
///
/// They are the children of the Container `word`'s directory part
/// names whose own names begin as its last part does, a Container
/// with a `/` after it so the next Tab goes on under it.
///
/// `a/b` looks under `a`, a word with no `/` under `under` itself, and
/// a word beginning with `/` from the tree's root, as the cli reads a
/// target; `.` stays where it is and `..` goes to the parent. A word
/// whose directory part resolves nowhere offers nothing, which is not
/// a fault: a name that is not there yet is the ordinary case.
#[must_use]
pub fn paths(tree: &mut Tree, under: Rd, word: &str) -> Vec<String> {
    let (dir, prefix) = word
        .rfind('/')
        .map_or(("", word), |cut| (&word[..=cut], &word[cut + 1..]));
    let Ok(rd) = walk(tree, under, dir) else { return Vec::new() };
    let Ok(children) = tree.children(rd) else { return Vec::new() };
    let mut names: Vec<String> = children
        .into_iter()
        .filter_map(|child| named(tree, child, prefix))
        .map(|(name, container)| {
            let end = if container { "/" } else { "" };
            format!("{dir}{name}{end}")
        })
        .collect();
    names.sort();
    names
}

/// The Container `dir` names from `under`, or from the root where it
/// begins with `/`.
fn walk(tree: &mut Tree, under: Rd, dir: &str) -> Result<Rd, Fault> {
    let mut rd = if dir.starts_with('/') { ROOT } else { under };
    let step = |name: &&str| !name.is_empty() && *name != ".";
    for name in dir.split('/').filter(step) {
        rd = if name == ".." { up(tree, rd)? } else { tree.lookup(rd, name)? };
    }
    Ok(rd)
}

/// The parent of `rd`; the root is its own parent, as `cd /..` in a
/// shell is `/`.
fn up(tree: &mut Tree, rd: Rd) -> Result<Rd, Fault> {
    if rd == ROOT {
        return Ok(ROOT);
    }
    tree.parent(rd)
}

/// The child's name when it begins as `prefix` does, and whether it is
/// a Container.
fn named(tree: &mut Tree, child: Rd, prefix: &str) -> Option<(String, bool)> {
    let info = tree.info(child).ok()?;
    let name = info.name?;
    name.starts_with(prefix)
        .then(|| (name, info.flags.contains(Flags::CONTAINER)))
}
