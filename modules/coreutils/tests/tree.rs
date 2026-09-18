//! `tree` over the rig: what one run's output cell holds, beside the
//! drawing the cli's own `tree` prints of the same target in the same
//! tree.

use reovim_client::{Client, Error, Fault, ROOT, Rd, Rights};
use reovim_client_cli::{Out, Paint, Shell, input};
use reovim_module_coreutils::bins::tree::Args;
use reovim_rig::Rig;

/// A run of `target`, as a shell fills the record: every level, no
/// color, from the root.
fn args(target: &str) -> Args {
    Args { L: 0, cwd: ROOT, color: false, target: target.to_string() }
}

/// `call /bin/tree run {..}`: the rd of the output cell, or the fault
/// the call answered with.
fn run(client: &mut Client, args: &Args) -> Result<Rd, Fault> {
    let tree = client.resolve("/bin/tree")?;
    let out = client.call_by_name(tree, "run", reovim_sdk::encode(args))?;
    Ok(reovim_sdk::decode(&out)?)
}

/// The cell `rd` read at an offset to the empty read that ends it, and
/// closed, which is how a shell reads one.
fn drain(client: &mut Client, rd: Rd) -> String {
    let h = client.open(rd, Rights::READ).unwrap();
    let mut bytes = Vec::new();
    loop {
        let at = bytes.len() as u64;
        let chunk = client.read(h, at, 64).unwrap();
        if chunk.is_empty() {
            break;
        }
        bytes.extend(chunk);
    }
    client.close(h).unwrap();
    String::from_utf8(bytes).unwrap()
}

/// The run and the cli line that are the same text: that text, once
/// both have printed it. `at` is where the line stands, which the cli
/// keeps as a path and the run as the `cwd` of its record.
fn same(
    client: &mut Client,
    at: (Rd, &str),
    args: &Args,
    line: &str,
    out: Out,
) -> String {
    let cell = run(client, args).unwrap();
    let said = drain(client, cell);
    let words: Vec<String> =
        line.split_whitespace().map(str::to_string).collect();
    let mut shell = Shell::at(at.0, at.1.to_string());
    let printed = input(client, &mut shell, &words, out).unwrap();
    assert_eq!(said, String::from_utf8(printed).unwrap(), "{line}");
    said
}

#[test]
fn a_drawing_puts_containers_first_and_counts_what_it_drew() {
    let rig = Rig::new("tree-draw");
    rig.file("note", b"hi\n");
    std::fs::create_dir(rig.dir().join("sub")).unwrap();
    std::fs::write(rig.dir().join("sub/deep"), b"x").unwrap();
    let mut client = rig.client();
    let root = "/module/fs/root";
    let here = (ROOT, "/");

    let drawn = same(
        &mut client,
        here,
        &args(root),
        &format!("tree {root}"),
        Out::plain(),
    );
    assert_eq!(
        drawn,
        "/module/fs/root\n├── sub\n│   └── deep\n└── note\n\n\
         1 containers, 2 cells\n"
    );

    // `L` stops the walk that many levels down; 0 does not stop
    // it, which is the run the line without `-L` makes.
    let mut one = args(root);
    one.L = 1;
    let stopped = same(
        &mut client,
        here,
        &one,
        &format!("tree {root} -L 1"),
        Out::plain(),
    );
    assert_eq!(
        stopped,
        "/module/fs/root\n├── sub\n└── note\n\n1 containers, 1 cells\n"
    );

    // A cell has nothing under it, and the root of the tree draws the
    // Kernel's own Containers.
    let note = format!("{root}/note");
    let said = same(
        &mut client,
        here,
        &args(&note),
        &format!("tree {note}"),
        Out::plain(),
    );
    assert_eq!(said, format!("{note}\n\n0 containers, 0 cells\n"));
    let mut top = args("/");
    top.L = 1;
    let said = same(&mut client, here, &top, "tree / -L 1", Out::plain());
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
    let said = same(
        &mut client,
        (root, "/module/fs/root"),
        &here,
        "tree -L 1",
        Out::plain(),
    );
    assert_eq!(
        said,
        "/module/fs/root\n├── sub\n└── note\n\n1 containers, 1 cells\n"
    );

    // A relative target is walked from the same `cwd`.
    let mut sub = args("sub");
    sub.cwd = root;
    same(
        &mut client,
        (root, "/module/fs/root"),
        &sub,
        "tree sub",
        Out::plain(),
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
    let said = same(&mut client, (at, &rd), &here, "tree", Out::plain());
    assert_eq!(said, format!("{rd}\n└── note\n\n0 containers, 1 cells\n"));
}

#[test]
fn color_wears_the_kind_of_every_name_drawn() {
    let rig = Rig::new("tree-color");
    let mut client = rig.client();
    let mut painted = args("/module/text");
    painted.color = true;
    let out = Out { width: None, paint: Paint::on() };
    let said =
        same(&mut client, (ROOT, "/"), &painted, "tree /module/text", out);
    assert!(said.contains("└── \x1b[32mopen\x1b[0m\n"), "{said}");
}

#[test]
fn a_target_that_is_not_there_fails_the_run_and_leaves_no_cell() {
    let rig = Rig::new("tree-missing");
    let mut client = rig.client();
    let tree = client.resolve("/bin/tree").unwrap();

    assert_eq!(
        run(&mut client, &args("/nowhere")),
        Err(Fault::Kernel(Error::Invalid))
    );
    assert_eq!(
        client.children(tree),
        Ok(Vec::new()),
        "a cell for a failed run"
    );
}
