//! `tree` over the rig: the drawing one run's output cell holds of the
//! tree under its target, and the counts under it.

use reovim_client::{Error, Fault, ROOT};
use reovim_module_coreutils::bins::tree::Args;
use reovim_rig::Rig;
use reovim_rig::bins::{run, text};

/// A run of `target`, as a shell fills the record: every level, no
/// color, from the root.
fn args(target: &str) -> Args {
    Args { L: 0, cwd: ROOT, color: false, target: target.to_string() }
}

#[test]
fn a_drawing_puts_containers_first_and_counts_what_it_drew() {
    let rig = Rig::new("tree-draw");
    rig.file("note", b"hi\n");
    std::fs::create_dir(rig.dir().join("sub")).unwrap();
    std::fs::write(rig.dir().join("sub/deep"), b"x").unwrap();
    let mut client = rig.client();
    let root = "/module/fs/root";

    let drawn = text(&mut client, "tree", &args(root));
    assert_eq!(
        drawn,
        "/module/fs/root\n├── sub\n│   └── deep\n└── note\n\n\
         1 containers, 2 cells\n"
    );

    // `L` stops the walk that many levels down; 0 does not stop
    // it, which is the run the line without `-L` makes.
    let mut one = args(root);
    one.L = 1;
    assert_eq!(
        text(&mut client, "tree", &one),
        "/module/fs/root\n├── sub\n└── note\n\n1 containers, 1 cells\n"
    );

    // A cell has nothing under it, and the root of the tree draws the
    // Kernel's own Containers.
    let note = format!("{root}/note");
    let said = text(&mut client, "tree", &args(&note));
    assert_eq!(said, format!("{note}\n\n0 containers, 0 cells\n"));
    let mut top = args("/");
    top.L = 1;
    let said = text(&mut client, "tree", &top);
    assert!(said.starts_with("/\n├── bin\n├── module\n"), "{said}");
}

#[test]
fn no_target_is_the_cwd_and_the_first_line_says_where_that_is() {
    let rig = Rig::new("tree-cwd");
    rig.file("note", b"hi\n");
    std::fs::create_dir(rig.dir().join("sub")).unwrap();
    let mut client = rig.client();
    let root = client.resolve("/module/fs/root").unwrap();

    let mut here = args("");
    here.cwd = root;
    here.L = 1;
    assert_eq!(
        text(&mut client, "tree", &here),
        "/module/fs/root\n├── sub\n└── note\n\n1 containers, 1 cells\n"
    );

    // A relative target is walked from the same `cwd`, and the first
    // line is the target as it was named.
    let mut sub = args("sub");
    sub.cwd = root;
    assert_eq!(
        text(&mut client, "tree", &sub),
        "sub\n\n0 containers, 0 cells\n"
    );
}

#[test]
fn a_cwd_too_deep_for_a_path_says_its_rd_on_the_first_line() {
    let rig = Rig::new("tree-deep");
    // `/module/fs/root` is three names, so this chain stands 67 deep,
    // past the 64 a first line will say.
    let names: Vec<String> = (0..64).map(|i| format!("d{i}")).collect();
    let deep = names.iter().fold(rig.dir().to_path_buf(), |at, n| at.join(n));
    std::fs::create_dir_all(&deep).unwrap();
    std::fs::write(deep.join("note"), b"bottom\n").unwrap();
    let mut client = rig.client();
    let path = format!("/module/fs/root/{}", names.join("/"));
    let at = client.resolve(&path).unwrap();
    let rd = format!("rd:{}", at.0);

    let mut here = args("");
    here.cwd = at;
    let said = text(&mut client, "tree", &here);
    assert_eq!(said, format!("{rd}\n└── note\n\n0 containers, 1 cells\n"));
}

#[test]
fn color_wears_the_kind_of_every_name_drawn() {
    let rig = Rig::new("tree-color");
    let mut client = rig.client();
    let mut painted = args("/module/text");
    painted.color = true;
    let said = text(&mut client, "tree", &painted);
    assert!(said.contains("└── \x1b[32mopen\x1b[0m\n"), "{said}");
}

#[test]
fn a_target_that_is_not_there_fails_the_run_and_leaves_no_cell() {
    let rig = Rig::new("tree-missing");
    let mut client = rig.client();
    let tree = client.resolve("/bin/tree").unwrap();

    assert_eq!(
        run(&mut client, "tree", &args("/nowhere")),
        Err(Fault::Kernel(Error::Invalid))
    );
    assert_eq!(
        client.children(tree),
        Ok(Vec::new()),
        "a cell for a failed run"
    );
}
