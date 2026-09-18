//! The walk over a tree of names the test holds itself: where each
//! shape of path begins and which moves it asks for.

use std::collections::BTreeMap;

use reovim_kernel_interface::Rd;
use reovim_sdk::walk::{ROOT, Step, walk};

/// A tree of `(parent, name) -> child` with every parent remembered,
/// and the moves a walk asked of it in order.
#[derive(Default)]
struct Tree {
    children: BTreeMap<(u64, String), Rd>,
    parents: BTreeMap<u64, Rd>,
    asked: Vec<String>,
}

impl Tree {
    /// `name` under `parent` is the rd `id`.
    fn add(&mut self, parent: Rd, name: &str, id: u64) -> Rd {
        let rd = Rd(id);
        self.children.insert((parent.0, name.to_string()), rd);
        self.parents.insert(id, parent);
        rd
    }

    /// The rd `path` names from `from`, or the name that stopped it.
    fn walk(&mut self, from: Rd, path: &str) -> Result<Rd, String> {
        let (children, parents) = (&self.children, &self.parents);
        let asked = &mut self.asked;
        walk(from, path, |step| match step {
            Step::Child { rd, name } => {
                asked.push(name.to_string());
                children
                    .get(&(rd.0, name.to_string()))
                    .copied()
                    .ok_or_else(|| name.to_string())
            }
            Step::Parent { rd } => {
                asked.push("..".to_string());
                parents.get(&rd.0).copied().ok_or_else(|| "..".to_string())
            }
        })
    }
}

/// `/module/text` and `/module/fs` over a root of id 0.
fn tree() -> Tree {
    let mut tree = Tree::default();
    let module = tree.add(ROOT, "module", 4);
    tree.add(module, "text", 20);
    tree.add(module, "fs", 21);
    tree
}

#[test]
fn a_path_begins_at_the_root_at_from_or_at_an_rd() {
    let mut tree = tree();

    // Absolute from the root, whatever `from` is.
    assert_eq!(tree.walk(Rd(20), "/module/text"), Ok(Rd(20)));
    // Relative from where the walk was told to stand.
    assert_eq!(tree.walk(Rd(4), "text"), Ok(Rd(20)));
    assert_eq!(tree.walk(Rd(4), "nowhere"), Err("nowhere".to_string()));
    // An rd names itself, alone or as the first segment.
    assert_eq!(tree.walk(ROOT, "rd:20"), Ok(Rd(20)));
    assert_eq!(tree.walk(ROOT, "rd:4/fs"), Ok(Rd(21)));
    // A `rd:` that is no number is an ordinary name.
    assert_eq!(tree.walk(Rd(4), "rd:x"), Err("rd:x".to_string()));
}

#[test]
fn a_dot_stands_still_and_dot_dot_goes_up_from_anywhere_but_the_root() {
    let mut tree = tree();

    assert_eq!(tree.walk(Rd(20), "."), Ok(Rd(20)));
    assert_eq!(tree.walk(Rd(20), "./././"), Ok(Rd(20)));
    assert_eq!(tree.walk(Rd(20), ".."), Ok(Rd(4)));
    assert_eq!(tree.walk(Rd(20), "../fs"), Ok(Rd(21)));
    // Inside an absolute path they are moves, not names to look up.
    assert_eq!(tree.walk(ROOT, "/module/text/../fs"), Ok(Rd(21)));
}

#[test]
fn the_root_is_its_own_parent_and_is_never_asked_who_that_is() {
    let mut tree = tree();

    // As `cd /..` in a shell is `/`.
    assert_eq!(tree.walk(ROOT, "../../.."), Ok(ROOT));
    assert!(tree.asked.is_empty(), "asked above the root: {:?}", tree.asked);
}

#[test]
fn an_empty_segment_is_no_move_at_all() {
    let mut tree = tree();

    assert_eq!(tree.walk(Rd(20), ""), Ok(Rd(20)));
    assert_eq!(tree.walk(ROOT, "/"), Ok(ROOT));
    assert_eq!(tree.walk(ROOT, "//module//text//"), Ok(Rd(20)));
    assert_eq!(tree.asked, ["module", "text"], "one lookup a name");
}
