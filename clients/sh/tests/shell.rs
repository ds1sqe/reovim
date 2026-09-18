//! Where a line stands in the tree, and the names the shell runs
//! itself: `cd` and `pwd` move and say, `load` and `unload` are the
//! server's two messages, and neither takes a redirect.

use reovim_client::Client;
use reovim_client_sh::{Shell, run};
use reovim_rig::Rig;

/// One typed line from where `shell` stands; its output, or the line a
/// Miss would print. `cd` moves `shell`, as it does for a host.
fn typed(client: &mut Client, shell: &mut Shell, line: &str) -> String {
    match run(client, shell, line) {
        Ok(ran) => String::from_utf8(ran.out).unwrap(),
        Err(miss) => miss.to_string(),
    }
}

#[test]
fn cd_moves_the_line_and_pwd_says_where_it_stands() {
    let rig = Rig::new("sh-cd");
    rig.file("note", b"hi\n");
    let mut client = rig.client();
    let mut shell = Shell::default();
    let mut go =
        |client: &mut Client, line: &str| typed(client, &mut shell, line);

    // Where every session starts, and the prompt that says so.
    assert_eq!(go(&mut client, "pwd"), "/\n");
    assert_eq!(go(&mut client, "cd /module/fs/root"), "");
    assert_eq!(go(&mut client, "pwd"), "/module/fs/root\n");

    // A name under the cwd, and the path read back from the tree.
    assert_eq!(go(&mut client, "cat note"), "hi\n");
    assert_eq!(go(&mut client, "cd .."), "");
    assert_eq!(go(&mut client, "pwd"), "/module/fs\n");
    assert_eq!(go(&mut client, "cd ."), "");
    assert_eq!(go(&mut client, "cd root/../../text"), "");
    assert_eq!(go(&mut client, "pwd"), "/module/text\n");

    // A path from `/` takes the same walk, so a `.` or a `..` in it is
    // a move and never a name the tree is asked for.
    assert_eq!(go(&mut client, "cd /module/fs/root/.."), "");
    assert_eq!(go(&mut client, "pwd"), "/module/fs\n");
    assert_eq!(go(&mut client, "ls /module/./fs"), "root\n");
    assert_eq!(go(&mut client, "cd /.."), "");
    assert_eq!(go(&mut client, "pwd"), "/\n");

    // `cd -` is the one before, twice over; no target is the root.
    assert_eq!(go(&mut client, "cd /module/fs"), "");
    assert_eq!(go(&mut client, "cd /module/text"), "");
    assert_eq!(go(&mut client, "cd -"), "");
    assert_eq!(go(&mut client, "pwd"), "/module/fs\n");
    assert_eq!(go(&mut client, "cd -"), "");
    assert_eq!(go(&mut client, "pwd"), "/module/text\n");
    assert_eq!(go(&mut client, "cd"), "");
    assert_eq!(go(&mut client, "pwd"), "/\n");

    // An rd names the same Container, and the prompt still says where
    // it is, since the path is read from the tree.
    let root = client.resolve("/module/fs/root").unwrap();
    assert_eq!(go(&mut client, &format!("cd rd:{}", root.0)), "");
    assert_eq!(go(&mut client, "pwd"), "/module/fs/root\n");

    // What a target of `cd` cannot be.
    assert_eq!(go(&mut client, "cd note"), "cd note: unsupported");
    assert_eq!(
        go(&mut client, "cd nowhere/x"),
        "cd nowhere/x: no such rd (nowhere)"
    );
    assert_eq!(go(&mut client, "pwd"), "/module/fs/root\n");
}

#[test]
fn cd_back_from_where_nothing_came_before_says_so() {
    let rig = Rig::new("sh-back");
    let mut client = rig.client();
    let mut shell = Shell::default();
    assert_eq!(
        typed(&mut client, &mut shell, "cd -"),
        "cd -: nowhere to go back to"
    );
    assert_eq!(shell.prompt(), "/ > ");
    assert_eq!(typed(&mut client, &mut shell, "cd /sys"), "");
    assert_eq!(shell.prompt(), "/sys > ");
    assert_eq!(shell.path(), "/sys");
    assert_eq!(shell.cwd(), client.resolve("/sys").unwrap());
}

#[test]
fn pwd_past_the_depth_is_the_rd() {
    let rig = Rig::new("sh-deep");
    // `/module/fs/root` is three names, so this chain stands 67 deep,
    // past the 64 a prompt will show.
    let names: Vec<String> = (0..64).map(|i| format!("d{i}")).collect();
    let deep = names.iter().fold(rig.dir().to_path_buf(), |at, n| at.join(n));
    std::fs::create_dir_all(&deep).unwrap();
    std::fs::write(deep.join("note"), b"bottom\n").unwrap();
    let mut client = rig.client();
    let mut shell = Shell::default();
    let mut go =
        |client: &mut Client, line: &str| typed(client, &mut shell, line);

    let root = format!("/module/fs/root/{}", names.join("/"));
    assert_eq!(go(&mut client, &format!("cd {root}")), "");
    let rd = go(&mut client, "pwd");
    assert!(rd.starts_with("rd:"), "past 64 names: {rd}");
    assert_eq!(go(&mut client, "ls"), "note\n");
    assert_eq!(go(&mut client, "cat note"), "bottom\n", "ops are per rd");
}

#[test]
fn a_builtin_takes_its_own_tokens_and_no_redirect() {
    let rig = Rig::new("sh-builtins");
    rig.file("out", b"");
    let mut client = rig.client();
    let mut shell = Shell::default();
    let mut go =
        |client: &mut Client, line: &str| typed(client, &mut shell, line);

    assert_eq!(go(&mut client, "pwd here"), "pwd here: usage: pwd");
    assert_eq!(go(&mut client, "cd a b"), "cd a b: usage: cd [target]");
    assert_eq!(go(&mut client, "load"), "load: usage: load <path>");
    assert_eq!(
        go(&mut client, "pwd > /module/fs/root/out"),
        "pwd: a redirect takes a run's output"
    );

    // `load` and `unload` are the server's own two messages and no op
    // of the tree, so what it refuses is one line of the shell's table.
    // The rig loads its Modules itself and serves neither.
    let loaded = go(&mut client, "load /nowhere.rvm");
    assert_eq!(loaded, "load /nowhere.rvm: unsupported");
    let gone = go(&mut client, "unload clipboard");
    assert_eq!(gone, "unload clipboard: unsupported");
}
