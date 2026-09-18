//! The shell words over the rig: what `ls`, `cat`, `stat` and `tree`
//! print of a tree the test makes itself. No terminal, so the width
//! `ls` lays its names out across is the test's to give.

use reovim_client::Client;
use reovim_client_cli::{Out, Shell, input};
use reovim_rig::Rig;

/// One command line; its output, or the line a fault would print.
fn run(client: &mut Client, command: &str, width: Option<u16>) -> String {
    let words: Vec<String> =
        command.split_whitespace().map(str::to_string).collect();
    let out = Out { width, paint: reovim_client_cli::Paint::off() };
    match input(client, &mut Shell::default(), &words, out) {
        Ok(out) => String::from_utf8(out).unwrap(),
        Err(miss) => miss.to_string(),
    }
}

/// `YYYY-MM-DD HH:MM:SS` of a time the Kernel took while the test ran.
fn dated(field: &str) -> bool {
    let shape = |(at, c): (usize, char)| match at {
        4 | 7 => c == '-',
        10 => c == ' ',
        13 | 16 => c == ':',
        _ => c.is_ascii_digit(),
    };
    field.len() == 19
        && field.starts_with("20")
        && field.char_indices().all(shape)
}

#[test]
fn ls_names_the_children_sorted_down_columns_or_one_per_line() {
    let rig = Rig::new("ls");
    for name in ["gamma", "alpha", "beta"] {
        rig.file(name, b"");
    }
    let mut client = rig.client();
    let root = "/module/fs/root";

    // Piped: one per line, sorted, whatever order the Module gives.
    assert_eq!(
        run(&mut client, &format!("ls {root}"), None),
        "alpha\nbeta\ngamma\n"
    );
    // A terminal: down the columns that fit, as ls lays them out.
    let wide = run(&mut client, &format!("ls {root}"), Some(80));
    assert_eq!(wide, "alpha  beta  gamma\n");
    let narrow = run(&mut client, &format!("ls {root}"), Some(12));
    assert_eq!(narrow, "alpha  gamma\nbeta\n");

    // No target is the root of the tree; a cell lists itself.
    let all = run(&mut client, "ls", None);
    assert_eq!(all, "bin\nmodule\nsession\nsys\nvar\n");
    assert_eq!(run(&mut client, &format!("ls {root}/beta"), None), "beta\n");

    // More than one target names each listing, as ls does.
    let both = run(&mut client, &format!("ls /module {root}"), None);
    assert_eq!(
        both,
        format!(
            "/module:\nclipboard\ncoreutils\nfs\nsysutils\ntext\n\n\
             {root}:\nalpha\nbeta\ngamma\n"
        )
    );
    assert_eq!(
        run(&mut client, "ls -z", None),
        "usage: ls [-l] [-a] [-w] [TARGET..]; help ls says more"
    );

    // A directory with nothing under it prints nothing, laid out
    // across a terminal's columns as much as down a pipe.
    std::fs::create_dir(rig.dir().join("empty")).unwrap();
    let empty = format!("ls {root}/empty");
    assert_eq!(run(&mut client, &empty, Some(80)), "");
    assert_eq!(run(&mut client, &empty, None), "");
}

#[test]
fn the_long_line_says_type_rights_mtime_apis_and_name() {
    let rig = Rig::new("ls-long");
    rig.file("note", b"hi\n");
    let mut client = rig.client();

    // A Container of the Kernel's own: `c`, read only, no api.
    let modules = run(&mut client, "ls -l /module", None);
    let fs = modules.lines().find(|l| l.ends_with("  fs")).unwrap();
    assert!(fs.starts_with("c rw  "), "{fs}");
    // A cell, and one that offers an api: the api's name, not its hash.
    let note = run(&mut client, "ls -l /module/fs/root", None);
    let fields: Vec<&str> = note.trim_end().split("  ").collect();
    assert_eq!(fields.len(), 4, "{note}");
    assert_eq!((fields[0], fields[2], fields[3]), ("- rw", "-", "note"));
    assert!(dated(fields[1]), "{note}");
    let open = run(&mut client, "ls -l /module/text", None);
    assert!(open.trim_end().ends_with("  open  open"), "{open}");

    // `-a` puts the rd number first, in both forms.
    let rds = run(&mut client, "ls -a /module/fs/root", None);
    let rd = run(&mut client, "lookup /module/fs/root/note", None);
    assert_eq!(rds, format!("{} note\n", rd.trim_end()));
    let long = run(&mut client, "ls -la /module/fs/root", None);
    assert!(long.starts_with(&format!("{} - rw  ", rd.trim_end())), "{long}");

    // An rd the Kernel does not name shows as its number.
    let root = run(&mut client, "lookup /", None);
    let stat = run(&mut client, "stat /", None);
    let first = stat.lines().next().unwrap();
    assert!(first.ends_with(&format!("  rd:{}", root.trim_end())), "{first}");
}

#[test]
fn cat_writes_the_bytes_of_each_target_and_says_ls_of_a_container() {
    let rig = Rig::new("cat");
    rig.file("one", b"first\n");
    rig.file("two", b"second\n");
    let mut client = rig.client();
    let root = "/module/fs/root";

    assert_eq!(run(&mut client, &format!("cat {root}/one"), None), "first\n");
    assert_eq!(
        run(&mut client, &format!("cat {root}/one {root}/two"), None),
        "first\nsecond\n"
    );
    assert_eq!(
        run(&mut client, &format!("cat {root}"), None),
        "cat /module/fs/root: a Container has no bytes; ls it"
    );
    assert_eq!(
        run(&mut client, &format!("cat {root}/one /nope"), None),
        "cat /nope: no such rd (nope)"
    );
}

#[test]
fn stat_says_the_times_and_every_method_of_every_api() {
    let rig = Rig::new("stat");
    let mut client = rig.client();
    let said = run(&mut client, "stat /module/text/open", None);
    let lines: Vec<&str> = said.lines().collect();

    assert!(lines[0].ends_with("  open  open"), "{said}");
    assert_eq!(
        lines[1],
        format!(
            "rd {}",
            run(&mut client, "lookup /module/text/open", None).trim_end()
        )
    );
    for (at, key) in [(2, "ctime"), (3, "mtime"), (4, "atime")] {
        let (said, date) = lines[at].split_once(' ').unwrap();
        assert_eq!(said, key);
        assert!(dated(date), "{key}: {date}");
    }
    // The api by name and version, then its methods as the declaration
    // shows them: the number, the types and the right.
    assert_eq!(lines[5], "open v1");
    assert_eq!(lines[6], "  0 open(Rd) -> Rd WRITE");
    assert_eq!(lines[7], "  1 new() -> Rd WRITE");
    assert_eq!(lines.len(), 8, "{said}");
}

#[test]
fn tree_draws_containers_first_and_counts_what_it_drew() {
    let rig = Rig::new("tree");
    rig.file("note", b"hi\n");
    std::fs::create_dir(rig.dir().join("sub")).unwrap();
    std::fs::write(rig.dir().join("sub/deep"), b"x").unwrap();
    let mut client = rig.client();

    let drawn = run(&mut client, "tree /module/fs/root", None);
    assert_eq!(
        drawn,
        "/module/fs/root\n├── sub\n│   └── deep\n└── note\n\n\
         1 containers, 2 cells\n"
    );
    // `-L N` stops at N levels, which the fs root needs on a big disk.
    let one = run(&mut client, "tree /module/fs/root -L 1", None);
    assert_eq!(
        one,
        "/module/fs/root\n├── sub\n└── note\n\n1 containers, 1 cells\n"
    );
    // No target is the root; a cell has nothing under it.
    let top = run(&mut client, "tree -L 1", None);
    assert!(top.starts_with("/\n├── bin\n├── module\n"), "{top}");
    assert_eq!(
        run(&mut client, "tree /module/fs/root/note", None),
        "/module/fs/root/note\n\n0 containers, 0 cells\n"
    );
    assert_eq!(
        run(&mut client, "tree -L x", None),
        "usage: tree [-L N] [TARGET]; help tree says more"
    );
}
