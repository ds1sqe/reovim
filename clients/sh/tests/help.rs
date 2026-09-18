//! `help` and Tab: both read from the tree, so what the shell can run
//! is what stands under `/bin` beside the names it runs itself, and the
//! declarations are the whole of what `help` says.

use reovim_client::Client;
use reovim_client_sh::{Shell, complete, run};
use reovim_rig::Rig;

/// One typed line; its output, or the line a Miss would print.
fn typed(client: &mut Client, shell: &mut Shell, line: &str) -> String {
    match run(client, shell, line) {
        Ok(ran) => String::from_utf8(ran.out).unwrap(),
        Err(miss) => miss.to_string(),
    }
}

#[test]
fn help_is_the_builtins_and_then_one_line_per_name_under_path() {
    let rig = Rig::new("sh-help");
    let mut client = rig.client();
    let mut shell = Shell::default();
    let said = typed(&mut client, &mut shell, "help");
    let lines: Vec<&str> = said.lines().collect();

    // The five the shell runs itself, each with its own sentence.
    assert_eq!(
        lines[..5],
        [
            "cd [target]                     \
             go there; none is /, - the one before",
            "pwd                             where the line stands now",
            "load <path>                     the Module in that .rvm file",
            "unload <name>                   the Module of that name",
            "help [name]                     these lines, or the one for name",
        ]
    );
    // Then the eight names under `/bin`, each a part per field the
    // declaration holds and nothing else: a `bool` under its letter, a
    // flag of another type taking one after it, and the positionals.
    assert_eq!(
        lines[5..13],
        [
            "call [target] [method] [arg]",
            "cat [-f] [-n N] [targets..]",
            "dmesg [-t] [-T] [-H] [-d] [-r] [-x] [-k] [-w] [-W] [-f F] [-l L]",
            "log [from]",
            "ls [-l] [-a] [-w] [targets..]",
            "stat [target]",
            "tree [-L L] [target]",
            "watch [target]",
        ]
    );
    assert!(said.contains("\na target is a path from /"), "{said}");
    assert!(said.ends_with("as\nCtrl-C does\n"), "{said}");

    // One name is its own line, from the same two places.
    let one = typed(&mut client, &mut shell, "help ls");
    assert_eq!(one, "ls [-l] [-a] [-w] [targets..]\n");
    assert_eq!(
        typed(&mut client, &mut shell, "help pwd"),
        format!("{}\n", lines[1])
    );
    assert_eq!(
        typed(&mut client, &mut shell, "help nope"),
        "nope: not under /bin; help lists them"
    );
    assert_eq!(
        typed(&mut client, &mut shell, "help ls cat"),
        "help ls cat: usage: help [name]"
    );
}

#[test]
fn tab_offers_the_names_the_flags_of_one_and_the_paths_under_the_cwd() {
    let rig = Rig::new("sh-tab");
    rig.file("note", b"");
    rig.file("other", b"");
    let mut client = rig.client();
    let mut shell = Shell::default();
    let offer = |client: &mut Client, shell: &Shell, line: &str| {
        complete(client, shell, line).candidates
    };

    // The first token: the names under `/bin`, then the builtins.
    assert_eq!(offer(&mut client, &shell, "l"), ["log", "ls", "load"]);
    assert_eq!(offer(&mut client, &shell, "p"), ["pwd"]);
    assert_eq!(offer(&mut client, &shell, "c"), ["call", "cat", "cd"]);

    // A token under a dash: the flag fields of the Driver the line
    // names, each as a token under a dash spells it.
    assert_eq!(offer(&mut client, &shell, "ls -"), ["-l", "-a", "-w"]);
    assert_eq!(offer(&mut client, &shell, "tree -"), ["-L"]);
    assert_eq!(offer(&mut client, &shell, "cat -n"), ["-n"]);
    assert_eq!(
        offer(&mut client, &shell, "dmesg -"),
        ["-t", "-T", "-H", "-d", "-r", "-x", "-k", "-w", "-W", "-f", "-l"]
    );
    let none: [&str; 0] = [];
    assert_eq!(offer(&mut client, &shell, "pwd -"), none, "no declaration");

    // Anything else is a path from where the line stands.
    assert_eq!(typed(&mut client, &mut shell, "cd /module/fs/root"), "");
    assert_eq!(offer(&mut client, &shell, "cat no"), ["note"]);
    assert_eq!(offer(&mut client, &shell, "ls /mod"), ["/module/"]);
    assert_eq!(offer(&mut client, &shell, "call open op"), none);
}
