//! The shell: where a line stands in the tree, the targets it reads
//! from there, the keys that edit it, and what Tab offers under the
//! cwd. No terminal; the app's width and palette are the test's.

use reovim_client::{Client, Key};
use reovim_client_cli::{Act, Line, Out, Shell, complete, input};
use reovim_rig::Rig;

/// One command line from where `shell` stands; its output, or the line
/// a fault would print. `cd` moves `shell`, as it does for the app.
fn run(client: &mut Client, shell: &mut Shell, command: &str) -> String {
    let words: Vec<String> =
        command.split_whitespace().map(str::to_string).collect();
    match input(client, shell, &words, Out::plain()) {
        Ok(out) => String::from_utf8(out).unwrap(),
        Err(miss) => miss.to_string(),
    }
}

#[test]
fn cd_moves_the_line_and_pwd_says_where_it_stands() {
    let rig = Rig::new("shell-cd");
    rig.file("note", b"hi\n");
    let mut client = rig.client();
    let mut shell = Shell::default();
    let mut go =
        |client: &mut Client, command: &str| run(client, &mut shell, command);

    // Where every session starts, and the prompt that says so.
    assert_eq!(go(&mut client, "pwd"), "/\n");
    assert_eq!(go(&mut client, "cd /module/fs/root"), "");
    assert_eq!(go(&mut client, "pwd"), "/module/fs/root\n");

    // A name under the cwd, and the path read back from the tree.
    assert_eq!(go(&mut client, "cat note"), "hi\n");
    assert_eq!(go(&mut client, "cd .."), "");
    assert_eq!(go(&mut client, "pwd"), "/module/fs\n");
    assert_eq!(go(&mut client, "cd ."), "");
    assert_eq!(go(&mut client, "pwd"), "/module/fs\n");
    assert_eq!(go(&mut client, "cd root/../../text"), "");
    assert_eq!(go(&mut client, "pwd"), "/module/text\n");

    // A path from `/` takes the same walk, so a `.` or a `..` in it is
    // a move and never a name the tree is asked for: fs refuses a
    // child named `..`, and the Kernel has no child named `.`.
    assert_eq!(go(&mut client, "cd /module/fs/root/.."), "");
    assert_eq!(go(&mut client, "pwd"), "/module/fs\n");
    assert_eq!(go(&mut client, "ls /module/./fs"), "root\n");
    assert_eq!(go(&mut client, "cd /.."), "");
    assert_eq!(go(&mut client, "pwd"), "/\n");
    assert_eq!(go(&mut client, "cd /module/text/../fs/root/./"), "");
    assert_eq!(go(&mut client, "pwd"), "/module/fs/root\n");
    assert_eq!(go(&mut client, "cat /module/fs/root/../root/note"), "hi\n");

    // `cd -` is the one before, twice over; no target is the root,
    // and the root is its own parent.
    assert_eq!(go(&mut client, "cd /module/fs"), "");
    assert_eq!(go(&mut client, "cd /module/text"), "");
    assert_eq!(go(&mut client, "cd -"), "");
    assert_eq!(go(&mut client, "pwd"), "/module/fs\n");
    assert_eq!(go(&mut client, "cd -"), "");
    assert_eq!(go(&mut client, "pwd"), "/module/text\n");
    assert_eq!(go(&mut client, "cd"), "");
    assert_eq!(go(&mut client, "cd .."), "");
    assert_eq!(go(&mut client, "pwd"), "/\n");

    // An rd names the same Container, and the prompt still says where
    // it is, since the path is read from the tree and not from what
    // was typed.
    let rd = go(&mut client, "lookup /module/fs/root");
    assert_eq!(go(&mut client, &format!("cd rd:{}", rd.trim())), "");
    assert_eq!(go(&mut client, "pwd"), "/module/fs/root\n");
}

#[test]
fn every_word_reads_its_target_from_where_the_line_stands() {
    let rig = Rig::new("shell-targets");
    rig.file("note", b"hi\n");
    std::fs::create_dir(rig.dir().join("sub")).unwrap();
    std::fs::write(rig.dir().join("sub/deep"), b"x").unwrap();
    let mut client = rig.client();
    let mut shell = Shell::default();
    let mut go =
        |client: &mut Client, command: &str| run(client, &mut shell, command);

    assert_eq!(go(&mut client, "cd /module/fs/root"), "");
    // No target is the cwd, for a listing and for the tree under it.
    assert_eq!(go(&mut client, "ls"), "note\nsub\n");
    assert_eq!(
        go(&mut client, "tree -L 1"),
        "/module/fs/root\n├── sub\n└── note\n\n1 containers, 1 cells\n"
    );
    assert_eq!(go(&mut client, "ls sub"), "deep\n");
    assert_eq!(go(&mut client, "cat sub/deep"), "x");
    assert_eq!(go(&mut client, "ls .."), "root\n");
    assert_eq!(
        go(&mut client, "ls /sys"),
        "core\ndecls\nlimit\nlog\nloglevel\nmem\n"
    );
    assert!(go(&mut client, "stat note").starts_with("- rw "));
    assert_eq!(go(&mut client, "write note there"), "5\n");
    assert_eq!(go(&mut client, "read note"), "there");

    // What a name under the cwd cannot be. (A name that is not on
    // disk is a file fs makes on the first write, so the one that
    // resolves nowhere here is under a Container of the Kernel's.)
    assert_eq!(
        go(&mut client, "cat ../../nope/x"),
        "cat ../../nope/x: no such rd (nope)"
    );
    assert_eq!(go(&mut client, "cd note"), "cd note: not a Container; cat it");
    assert_eq!(go(&mut client, "pwd"), "/module/fs/root\n");
}

#[test]
fn cd_back_from_where_nothing_came_before_says_so() {
    let rig = Rig::new("shell-back");
    let mut client = rig.client();
    let mut shell = Shell::default();
    assert_eq!(
        run(&mut client, &mut shell, "cd -"),
        "cd -: nowhere to go back to"
    );
    assert_eq!(shell.prompt(), "/ > ");
    assert_eq!(run(&mut client, &mut shell, "cd /sys"), "");
    assert_eq!(shell.prompt(), "/sys > ");
    assert_eq!(shell.cwd(), client.resolve("/sys").unwrap());
}

#[test]
fn pwd_past_the_depth_is_the_rd() {
    let rig = Rig::new("shell-deep");
    // `/module/fs/root` is three names, so this chain stands 67 deep,
    // past the 64 a prompt will show.
    let names: Vec<String> = (0..64).map(|i| format!("d{i}")).collect();
    let deep = names.iter().fold(rig.dir().to_path_buf(), |at, n| at.join(n));
    std::fs::create_dir_all(&deep).unwrap();
    std::fs::write(deep.join("note"), b"bottom\n").unwrap();
    let mut client = rig.client();
    let mut shell = Shell::default();
    let mut go =
        |client: &mut Client, command: &str| run(client, &mut shell, command);

    // The walk is by name, one segment at a time, however deep it
    // goes; only the prompt gives up and says the rd.
    let root = format!("/module/fs/root/{}", names.join("/"));
    assert_eq!(go(&mut client, &format!("cd {root}")), "");
    let rd = go(&mut client, "pwd");
    assert!(rd.starts_with("rd:"), "past 64 names: {rd}");
    assert_eq!(go(&mut client, "ls"), "note\n");
    assert_eq!(go(&mut client, "cat note"), "bottom\n", "ops are per rd");

    // Back up to where the names fit again, and the path is a path.
    for _ in 0..3 {
        assert_eq!(go(&mut client, "cd .."), "");
    }
    let above = format!("/module/fs/root/{}\n", names[..61].join("/"));
    assert_eq!(go(&mut client, "pwd"), above);
    assert_eq!(go(&mut client, "cd ../.."), "");
    assert_eq!(go(&mut client, "cat d59/d60/d61/d62/d63/note"), "bottom\n");
}

/// The chars of `text`, one key at a time.
fn typed(line: &mut Line, text: &str) {
    for c in text.chars() {
        assert_eq!(line.key(Key::Char(c)), Act::Redraw);
    }
}

#[test]
fn the_keys_cut_the_line_and_the_screen_is_the_apps() {
    let mut line = Line::default();
    typed(&mut line, "ls /module/fs");

    // Ctrl-W takes the word before the cursor, the spaces with it.
    assert_eq!(line.key(Key::Ctrl('w')), Act::Redraw);
    assert_eq!(line.text(), "ls ");
    assert_eq!(line.key(Key::Ctrl('w')), Act::Redraw);
    assert_eq!(line.text(), "");
    assert_eq!(line.key(Key::Ctrl('w')), Act::Nothing);

    // Ctrl-A and Ctrl-E are Home and End; Ctrl-U cuts to the start.
    typed(&mut line, "cat note");
    assert_eq!(line.key(Key::Ctrl('a')), Act::Redraw);
    assert_eq!(line.key(Key::Ctrl('a')), Act::Nothing);
    assert_eq!(line.key(Key::Ctrl('e')), Act::Redraw);
    assert_eq!(line.key(Key::Ctrl('e')), Act::Nothing);
    assert_eq!(line.render("/ > "), ("/ > cat note".to_string(), 12));
    assert_eq!(line.key(Key::Left), Act::Redraw);
    assert_eq!(line.key(Key::Ctrl('u')), Act::Redraw);
    assert_eq!(line.text(), "e");
    assert_eq!(line.key(Key::Ctrl('u')), Act::Nothing);

    // Ctrl-D takes the char under the cursor while there is a line,
    // and ends the session once there is not. Ctrl-L is the app's.
    assert_eq!(line.key(Key::Ctrl('d')), Act::Redraw);
    assert_eq!(line.text(), "");
    assert_eq!(line.key(Key::Ctrl('d')), Act::Quit);
    typed(&mut line, "ab");
    assert_eq!(line.key(Key::Ctrl('d')), Act::Nothing, "at the end");
    assert_eq!(line.key(Key::Home), Act::Redraw);
    assert_eq!(line.key(Key::Ctrl('d')), Act::Redraw);
    assert_eq!(line.text(), "b");
    assert_eq!(line.key(Key::Ctrl('l')), Act::Clear);
    assert_eq!(line.text(), "b", "the screen goes, the line stays");
    assert_eq!(line.key(Key::Ctrl('c')), Act::Quit);
    assert_eq!(line.key(Key::Ctrl('x')), Act::Nothing);
}

#[test]
fn up_walks_the_lines_that_begin_as_this_one_does() {
    let mut line = Line::default();
    for command in ["ls -l", "cat note", "ls /sys"] {
        typed(&mut line, command);
        assert_eq!(line.key(Key::Enter), Act::Submit(command.to_string()));
    }

    // An empty line walks them all, newest first.
    assert_eq!(line.key(Key::Up), Act::Redraw);
    assert_eq!(line.text(), "ls /sys");
    assert_eq!(line.key(Key::Up), Act::Redraw);
    assert_eq!(line.text(), "cat note");
    assert_eq!(line.key(Key::Down), Act::Redraw);
    assert_eq!(line.text(), "ls /sys");
    assert_eq!(line.key(Key::Down), Act::Redraw);
    assert_eq!(line.text(), "", "past the newest is the line as it was");

    // A line that has something in it walks only what begins with it,
    // and past the newest of those is that beginning again.
    typed(&mut line, "ls");
    assert_eq!(line.key(Key::Up), Act::Redraw);
    assert_eq!(line.text(), "ls /sys");
    assert_eq!(line.key(Key::Up), Act::Redraw);
    assert_eq!(line.text(), "ls -l");
    assert_eq!(line.key(Key::Up), Act::Nothing);
    assert_eq!(line.key(Key::Down), Act::Redraw);
    assert_eq!(line.text(), "ls /sys");
    assert_eq!(line.key(Key::Down), Act::Redraw);
    assert_eq!(line.text(), "ls");

    // Editing the line starts the next walk from what it holds now.
    assert_eq!(line.key(Key::Backspace), Act::Redraw);
    assert_eq!(line.key(Key::Up), Act::Redraw);
    assert_eq!(line.text(), "ls /sys");
    typed(&mut line, "x");
    assert_eq!(line.key(Key::Up), Act::Nothing);
    assert_eq!(line.text(), "ls /sysx");
}

#[test]
fn tab_offers_the_names_under_the_cwd_and_the_flags_of_the_word() {
    let rig = Rig::new("shell-complete");
    rig.file("note", b"");
    rig.file("other", b"");
    let mut client = rig.client();
    let mut shell = Shell::default();
    let offer = |client: &mut Client, shell: &Shell, line: &str| {
        complete(client, shell, line).candidates
    };

    assert_eq!(offer(&mut client, &shell, "p"), ["pwd"]);
    assert_eq!(
        offer(&mut client, &shell, "c"),
        ["cat", "cd", "children", "call"]
    );
    assert_eq!(run(&mut client, &mut shell, "cd /module/fs/root"), "");

    // A name is completed under the cwd, a path from the root, and a
    // Container gets the slash the next Tab goes on from.
    assert_eq!(offer(&mut client, &shell, "cat no"), ["note"]);
    let mut both = offer(&mut client, &shell, "cat ");
    both.sort();
    assert_eq!(both, ["note", "other"]);
    assert_eq!(offer(&mut client, &shell, "ls ../../fs"), ["../../fs/"]);
    assert_eq!(offer(&mut client, &shell, "ls ../../fs/"), ["../../fs/root/"]);
    assert_eq!(offer(&mut client, &shell, "ls /mod"), ["/module/"]);
    let none: [&str; 0] = [];
    assert_eq!(offer(&mut client, &shell, "cat nowhere/n"), none);

    // The flags of the word, and the values a `dmesg` flag takes.
    assert_eq!(offer(&mut client, &shell, "ls -"), ["-l", "-a", "-w"]);
    assert_eq!(offer(&mut client, &shell, "tree -"), ["-L"]);
    assert_eq!(offer(&mut client, &shell, "dmesg --c"), ["--color"]);
    assert_eq!(offer(&mut client, &shell, "dmesg -f k"), ["kernel"]);
    assert_eq!(offer(&mut client, &shell, "dmesg -l e"), ["emerg", "err"]);
    assert_eq!(offer(&mut client, &shell, "write -"), none);

    // A method of a target named from where the line stands.
    assert_eq!(run(&mut client, &mut shell, "cd /module/text"), "");
    assert_eq!(offer(&mut client, &shell, "call open op"), ["open"]);
}
