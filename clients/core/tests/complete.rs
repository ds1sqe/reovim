//! `paths` over the rig: the one path completion, read from a tree
//! with an fs Module mirroring a directory of its own.

use reovim_client::{Rd, Tree, paths};
use reovim_rig::Rig;

/// The mirror of the rig's own directory, where its files are.
fn root(tree: &mut Tree) -> Rd {
    tree.resolve("/module/fs/root").unwrap()
}

#[test]
fn a_word_is_completed_from_the_children_of_its_directory() {
    let rig = Rig::new("paths");
    std::fs::create_dir_all(rig.dir().join("dir/deep")).unwrap();
    rig.file("note", b"");
    rig.file("number", b"");
    rig.file("other", b"");
    rig.file("dir/inner", b"");
    let mut client = rig.client();
    let tree = client.tree();
    let here = root(tree);
    let found = |tree: &mut Tree, word: &str| paths(tree, here, word);

    // The children of the cwd that begin as the word does, in name
    // order, a Container with the `/` the next Tab goes on under.
    assert_eq!(found(tree, "no"), ["note"]);
    assert_eq!(found(tree, "n"), ["note", "number"]);
    assert_eq!(found(tree, ""), ["dir/", "note", "number", "other"]);

    // The part before the last `/` is where it looks, and the whole
    // word as typed comes back with the name on the end.
    assert_eq!(found(tree, "dir/"), ["dir/deep/", "dir/inner"]);
    assert_eq!(found(tree, "dir/i"), ["dir/inner"]);
    assert_eq!(found(tree, "./dir/i"), ["./dir/inner"]);
    assert_eq!(found(tree, "dir/../no"), ["dir/../note"]);

    // A leading `/` is the tree's own root, whatever `under` is.
    assert_eq!(found(tree, "/mod"), ["/module/"]);
    assert_eq!(found(tree, "/module/f"), ["/module/fs/"]);

    // A word that resolves nowhere offers nothing, and so does one
    // under something that has no children of its own; neither is a
    // fault.
    let none: [&str; 0] = [];
    assert_eq!(found(tree, "nowhere/x"), none);
    assert_eq!(found(tree, "note/x"), none);
    assert_eq!(found(tree, "zz"), none);

    // Above the root is the root, as `cd /..` in a shell is `/`.
    let up = paths(tree, here, "../../../../mod");
    assert_eq!(up, ["../../../../module/"]);
}
