//! Where a line stands in the tree, and the names the shell runs
//! itself: `cd` and `pwd` move and say, and neither takes a redirect.
//! `load` and `unload` are runs now, Drivers of the Client's own under
//! `/self/bin`, so they are here as any name under PATH is.

use reovim_client_sh::{Shell, run};
use reovim_kernel_interface::Calls;
use reovim_rig::Rig;
use reovim_sdk::cell;
use reovim_sdk::walk::ROOT;

/// One typed line from where `shell` stands; its output, or the line a
/// Miss would print. `cd` moves `shell`, as it does for a host.
fn typed(ctx: &dyn Calls, shell: &mut Shell, line: &str) -> String {
    match run(ctx, shell, line) {
        Ok(ran) => String::from_utf8(ran.out).unwrap(),
        Err(miss) => miss.to_string(),
    }
}

#[test]
fn cd_moves_the_line_and_pwd_says_where_it_stands() {
    let rig = Rig::new("sh-cd");
    rig.file("note", b"hi\n");
    let host = rig.host();
    let ctx = &host.own();
    let mut shell = Shell::default();
    let mut go = |ctx: &dyn Calls, line: &str| typed(ctx, &mut shell, line);

    // Where every session starts, and the prompt that says so.
    assert_eq!(go(ctx, "pwd"), "/\n");
    assert_eq!(go(ctx, "cd /module/fs/root"), "");
    assert_eq!(go(ctx, "pwd"), "/module/fs/root\n");

    // A name under the cwd, and the path read back from the tree.
    assert_eq!(go(ctx, "cat note"), "hi\n");
    assert_eq!(go(ctx, "cd .."), "");
    assert_eq!(go(ctx, "pwd"), "/module/fs\n");
    assert_eq!(go(ctx, "cd ."), "");
    assert_eq!(go(ctx, "pwd"), "/module/fs\n");
    assert_eq!(go(ctx, "cd root/../../text"), "");
    assert_eq!(go(ctx, "pwd"), "/module/text\n");

    // A path from `/` takes the same walk, so a `.` or a `..` in it is
    // a move and never a name the tree is asked for.
    assert_eq!(go(ctx, "cd /module/fs/root/.."), "");
    assert_eq!(go(ctx, "pwd"), "/module/fs\n");
    assert_eq!(go(ctx, "ls /module/./fs"), "root\n");
    assert_eq!(go(ctx, "cd /.."), "");
    assert_eq!(go(ctx, "pwd"), "/\n");
    assert_eq!(go(ctx, "cd /module/text/../fs/root/./"), "");
    assert_eq!(go(ctx, "pwd"), "/module/fs/root\n");
    assert_eq!(go(ctx, "cat /module/fs/root/../root/note"), "hi\n");

    // `cd -` is the one before, twice over; no target is the root.
    assert_eq!(go(ctx, "cd /module/fs"), "");
    assert_eq!(go(ctx, "cd /module/text"), "");
    assert_eq!(go(ctx, "cd -"), "");
    assert_eq!(go(ctx, "pwd"), "/module/fs\n");
    assert_eq!(go(ctx, "cd -"), "");
    assert_eq!(go(ctx, "pwd"), "/module/text\n");
    assert_eq!(go(ctx, "cd"), "");
    assert_eq!(go(ctx, "pwd"), "/\n");

    // An rd names the same Container, and the prompt still says where
    // it is, since the path is read from the tree.
    let root = cell::resolve(ctx, ROOT, "/module/fs/root").unwrap();
    assert_eq!(go(ctx, &format!("cd rd:{}", root.0)), "");
    assert_eq!(go(ctx, "pwd"), "/module/fs/root\n");

    // What a target of `cd` cannot be.
    assert_eq!(go(ctx, "cd note"), "cd note: unsupported");
    assert_eq!(go(ctx, "cd nowhere/x"), "cd nowhere/x: no such rd (nowhere)");
    assert_eq!(go(ctx, "pwd"), "/module/fs/root\n");
}

#[test]
fn cd_back_from_where_nothing_came_before_says_so() {
    let rig = Rig::new("sh-back");
    let host = rig.host();
    let ctx = &host.own();
    let mut shell = Shell::default();
    assert_eq!(typed(ctx, &mut shell, "cd -"), "cd -: nowhere to go back to");
    assert_eq!(shell.prompt(), "/ > ");
    assert_eq!(typed(ctx, &mut shell, "cd /sys"), "");
    assert_eq!(shell.prompt(), "/sys > ");
    assert_eq!(shell.path(), "/sys");
    assert_eq!(shell.cwd(), cell::resolve(ctx, ROOT, "/sys").unwrap());
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
    let host = rig.host();
    let ctx = &host.own();
    let mut shell = Shell::default();
    let mut go = |ctx: &dyn Calls, line: &str| typed(ctx, &mut shell, line);

    let root = format!("/module/fs/root/{}", names.join("/"));
    assert_eq!(go(ctx, &format!("cd {root}")), "");
    let rd = go(ctx, "pwd");
    assert!(rd.starts_with("rd:"), "past 64 names: {rd}");
    assert_eq!(go(ctx, "ls"), "note\n");
    assert_eq!(go(ctx, "cat note"), "bottom\n", "ops are per rd");

    // Back up to where the names fit again, and the path is a path.
    for _ in 0..3 {
        assert_eq!(go(ctx, "cd .."), "");
    }
    let above = format!("/module/fs/root/{}\n", names[..61].join("/"));
    assert_eq!(go(ctx, "pwd"), above);
    assert_eq!(go(ctx, "cd ../.."), "");
    assert_eq!(go(ctx, "cat d59/d60/d61/d62/d63/note"), "bottom\n");
}

#[test]
fn a_builtin_takes_its_own_tokens_and_no_redirect() {
    let rig = Rig::new("sh-builtins");
    rig.file("out", b"");
    let host = rig.host();
    let ctx = &host.own();
    let mut shell = Shell::default();
    let mut go = |ctx: &dyn Calls, line: &str| typed(ctx, &mut shell, line);

    assert_eq!(go(ctx, "pwd here"), "pwd here: usage: pwd");
    assert_eq!(go(ctx, "cd a b"), "cd a b: usage: cd [target]");
    assert_eq!(
        go(ctx, "pwd > /module/fs/root/out"),
        "pwd: a redirect takes a run's output"
    );

    // `load` is a run, so a path nobody typed is the Driver's own
    // answer and not a usage line of the shell's: a declaration says
    // which fields a run has and not which it needs.
    assert_eq!(go(ctx, "load"), "load: invalid");

    // Without `-l` both speak to the link, and what the server says
    // comes back through the shell's one table. The rig loads its
    // Modules itself and reads no file for anyone.
    let loaded = go(ctx, "load /nowhere.rvm");
    assert_eq!(loaded, "load /nowhere.rvm: unsupported");
    let gone = go(ctx, "unload text");
    assert_eq!(gone, "unload text: unsupported");

    // With `-l` it is this Client, which was given no files at all.
    let own = go(ctx, "load -l /nowhere.rvm");
    assert_eq!(own, "load -l /nowhere.rvm: unsupported");
}

/// From the Host the shell stands on both sides, so a line walks into
/// the Client's own tree as it walks into the server's, and the two
/// names the Host serves are where PATH looks first. What a Driver of
/// the Client's own reads, and what the server's refuses, is pinned by
/// clients/core/tests/bin.rs over a Host that loaded a userland.
#[test]
fn the_clients_own_tree_stands_at_the_root_beside_the_servers() {
    let rig = Rig::new("sh-self");
    let host = rig.host();
    let ctx = &host.own();
    let mut shell = Shell::default();
    let mut go = |ctx: &dyn Calls, line: &str| typed(ctx, &mut shell, line);

    assert_eq!(go(ctx, "cd /self"), "");
    assert_eq!(go(ctx, "pwd"), "/self\n");
    assert_eq!(go(ctx, "cd bin"), "");
    assert_eq!(go(ctx, "pwd"), "/self/bin\n");

    // The first directory of PATH that holds the name wins, and this
    // Client holds `load` itself; `ls` it has not, so that one is the
    // server's, as `help` says of every name here.
    assert_eq!(go(ctx, "help load"), "load [-l] [path]\n");
    assert_eq!(go(ctx, "help ls"), "ls [-l] [-a] [-w] [targets..]\n");
}
