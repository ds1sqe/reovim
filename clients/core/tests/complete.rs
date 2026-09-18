//! `paths` over the rig: the one path completion, read through the one
//! door from a tree with an fs Module mirroring a directory of its own.

use reovim_kernel_interface::{Calls, Rd};
use reovim_rig::Rig;
use reovim_sdk::walk::ROOT;
use reovim_sdk::{cell, ops};

/// The mirror of the rig's own directory, where its files are.
fn root(ctx: &dyn Calls) -> Rd {
    cell::resolve(ctx, ROOT, "/module/fs/root").unwrap()
}

#[test]
fn a_word_is_completed_from_the_children_of_its_directory() {
    let rig = Rig::new("paths");
    std::fs::create_dir_all(rig.dir().join("dir/deep")).unwrap();
    rig.file("note", b"");
    rig.file("number", b"");
    rig.file("other", b"");
    rig.file("dir/inner", b"");
    let host = rig.host();
    let ctx = &host.own();
    let here = root(ctx);
    let found = |ctx: &dyn Calls, word: &str| ops::paths(ctx, here, word);

    // The children of the cwd that begin as the word does, in name
    // order, a Container with the `/` the next Tab goes on under.
    assert_eq!(found(ctx, "no"), ["note"]);
    assert_eq!(found(ctx, "n"), ["note", "number"]);
    assert_eq!(found(ctx, ""), ["dir/", "note", "number", "other"]);

    // The part before the last `/` is where it looks, and the whole
    // word as typed comes back with the name on the end.
    assert_eq!(found(ctx, "dir/"), ["dir/deep/", "dir/inner"]);
    assert_eq!(found(ctx, "dir/i"), ["dir/inner"]);
    assert_eq!(found(ctx, "./dir/i"), ["./dir/inner"]);
    assert_eq!(found(ctx, "dir/../no"), ["dir/../note"]);

    // A leading `/` is the tree's own root, whatever `under` is.
    assert_eq!(found(ctx, "/mod"), ["/module/"]);
    assert_eq!(found(ctx, "/module/f"), ["/module/fs/"]);

    // A word that resolves nowhere offers nothing, and so does one
    // under something that has no children of its own; neither is a
    // fault.
    let none: [&str; 0] = [];
    assert_eq!(found(ctx, "nowhere/x"), none);
    assert_eq!(found(ctx, "note/x"), none);
    assert_eq!(found(ctx, "zz"), none);

    // Above the root is the root, as `cd /..` in a shell is `/`.
    let up = ops::paths(ctx, here, "../../../../mod");
    assert_eq!(up, ["../../../../module/"]);
}
