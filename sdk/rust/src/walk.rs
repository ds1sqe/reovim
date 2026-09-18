//! The one walk from a path to an rd, spelled once for every side.
//!
//! A Client takes it over the wire and a Driver over its `Calls`, so
//! it knows only the moves a path asks for and leaves each of them to
//! a closure, linking nothing of either side.

use reovim_kernel_interface::Rd;

/// The tree's root, where an absolute path starts.
pub const ROOT: Rd = Rd(0);

/// One move of a walk, for the closure that makes it.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Step<'a> {
    /// The child of `rd` named `name`.
    Child {
        /// Where the walk stands.
        rd: Rd,
        /// The name to look up under it.
        name: &'a str,
    },
    /// The parent of `rd`; never asked of the root.
    Parent {
        /// Where the walk stands.
        rd: Rd,
    },
}

/// The rd `path` names, one `step` per segment.
///
/// From [`ROOT`] when `path` begins with `/`, from the rd a leading
/// `rd:<n>` names, and from `from` otherwise; `.` stays where it is
/// and `..` goes to the parent, so a `.` or a `..` inside an absolute
/// path is a move and never a name to look up. The root is its own
/// parent, as `cd /..` in a shell is `/`.
///
/// ```
/// use reovim_kernel_interface::Rd;
/// use reovim_sdk::walk::{Step, walk};
///
/// let named = |step: Step<'_>| match step {
///     Step::Child { name: "module", .. } => Ok(Rd(4)),
///     Step::Child { name, .. } => Err(name.to_string()),
///     Step::Parent { .. } => Ok(Rd(0)),
/// };
/// assert_eq!(walk(Rd(9), "/module", named), Ok(Rd(4)));
/// assert_eq!(walk(Rd(9), "rd:7/..", named), Ok(Rd(0)));
/// assert_eq!(walk(Rd(9), ".", named), Ok(Rd(9)));
/// ```
///
/// # Errors
/// Whatever the step that stopped the walk failed with.
pub fn walk<E>(
    from: Rd,
    path: &str,
    mut step: impl FnMut(Step<'_>) -> Result<Rd, E>,
) -> Result<Rd, E> {
    let (mut rd, rest) = start(from, path);
    let moves = |name: &&str| !name.is_empty() && *name != ".";
    for name in rest.split('/').filter(moves) {
        rd = moved(rd, name, &mut step)?;
    }
    Ok(rd)
}

/// Where the walk begins and what is left of the path: the root for an
/// absolute one, the rd a leading `rd:<n>` names, else `from`. A
/// `rd:<n>` that is no number is an ordinary name, which the lookup
/// answers for.
fn start(from: Rd, path: &str) -> (Rd, &str) {
    if let Some(rest) = path.strip_prefix('/') {
        return (ROOT, rest);
    }
    let (first, rest) = path.split_once('/').unwrap_or((path, ""));
    let id = first.strip_prefix("rd:").and_then(|n| n.parse::<u64>().ok());
    id.map_or((from, path), |id| (Rd(id), rest))
}

/// One segment: the parent for `..`, else the child of that name.
fn moved<E>(
    rd: Rd,
    name: &str,
    step: &mut impl FnMut(Step<'_>) -> Result<Rd, E>,
) -> Result<Rd, E> {
    match name {
        ".." if rd == ROOT => Ok(ROOT),
        ".." => step(Step::Parent { rd }),
        _ => step(Step::Child { rd, name }),
    }
}
