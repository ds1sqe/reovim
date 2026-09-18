//! Walking the tree by name, and the one path completion above it:
//! the names that may stand where a partly typed one does, read from
//! the tree itself.
//!
//! The shell walks a target with [`walk`] and completes one with
//! [`paths`], both from where its `Shell` stands; a Plugin does the
//! same from where the Plugin works. Neither knows a Module: these
//! are the children of a Container, whatever put them there.

use alloc::format;
use alloc::string::{String, ToString};
use alloc::vec::Vec;

use reovim_kernel_interface::{Flags, Rd};
use reovim_sdk::walk::Step;

use crate::client::Tree;
use crate::fault::Fault;

/// The names that may replace `word`, in name order.
///
/// They are the children of the Container `word`'s directory part
/// names whose own names begin as its last part does, a Container
/// with a `/` after it so the next Tab goes on under it.
///
/// `a/b` looks under `a`, a word with no `/` under `under` itself, and
/// a word beginning with `/` from the tree's root, as the shell reads
/// a target; `.` stays where it is and `..` goes to the parent. A word
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

/// The rd `path` names, walked segment by segment: the one walk every
/// user of a path takes, which is [`reovim_sdk::walk::walk`] over this
/// Client's tree.
///
/// # Errors
/// The name that stopped the walk beside the fault, for whoever says
/// where it stopped; [`Tree::resolve`] is this walk from the root.
pub fn walk(
    tree: &mut Tree,
    from: Rd,
    path: &str,
) -> Result<Rd, (String, Fault)> {
    reovim_sdk::walk::walk(from, path, |step| match step {
        Step::Child { rd, name } => {
            tree.lookup(rd, name).map_err(|f| (name.to_string(), f))
        }
        Step::Parent { rd } => {
            tree.parent(rd).map_err(|f| ("..".to_string(), f))
        }
    })
}

/// The child's name when it begins as `prefix` does, and whether it is
/// a Container.
fn named(tree: &mut Tree, child: Rd, prefix: &str) -> Option<(String, bool)> {
    let info = tree.info(child).ok()?;
    let name = info.name?;
    name.starts_with(prefix)
        .then(|| (name, info.flags.contains(Flags::CONTAINER)))
}
