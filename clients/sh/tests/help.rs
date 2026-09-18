//! `help` and Tab: both read from the tree, so what the shell can run
//! is what stands under PATH beside the names it runs itself, and the
//! declarations are the whole of what `help` says. This Client loaded
//! no Module of its own, so `/self/bin` holds the two the Host serves
//! and every other name resolves at the server's `/bin`.

use reovim_client_sh::{Shell, complete, run};
use reovim_kernel_interface::Calls;
use reovim_rig::Rig;

/// One typed line; its output, or the line a Miss would print.
fn typed(ctx: &dyn Calls, shell: &mut Shell, line: &str) -> String {
    match run(ctx, shell, line) {
        Ok(ran) => String::from_utf8(ran.out).unwrap(),
        Err(miss) => miss.to_string(),
    }
}

#[test]
fn help_is_the_builtins_and_then_one_line_per_name_under_path() {
    let rig = Rig::new("sh-help");
    let host = rig.host();
    let ctx = &host.own();
    let mut shell = Shell::default();
    let said = typed(ctx, &mut shell, "help");
    let lines: Vec<&str> = said.lines().collect();

    // The three the shell runs itself, each with its own sentence.
    assert_eq!(
        lines[..3],
        [
            "cd [target]                     \
             go there; none is /, - the one before",
            "pwd                             where the line stands now",
            "help [name]                     these lines, or the one for name",
        ]
    );
    // Then every name PATH holds, sorted, each a part per field the
    // declaration holds and nothing else: a `bool` under its letter, a
    // flag of another type taking one after it, and the positionals. A
    // run that takes nothing is the bare name. `load` and `unload` are
    // among them now, Drivers of the Client's own under `/self/bin`,
    // and `-l` is the `local` field a token never matches.
    assert_eq!(
        lines[3..16],
        [
            "call [target] [method] [arg]",
            "cat [-f] [-n N] [targets..]",
            "date",
            "dmesg [-t] [-T] [-H] [-d] [-r] [-x] [-k] [-w] [-W] [-f F] [-l L]",
            "grep [pattern] [targets..]",
            "load [-l] [path]",
            "log [from]",
            "ls [-l] [-a] [-w] [targets..]",
            "stat [target]",
            "tree [-L L] [target]",
            "unload [-l] [name]",
            "uptime",
            "watch [target]",
        ]
    );
    assert!(said.contains("\na target is a path from /"), "{said}");
    assert!(said.ends_with("as\nCtrl-C does\n"), "{said}");

    // One name is its own line, from the same two places.
    let one = typed(ctx, &mut shell, "help ls");
    assert_eq!(one, "ls [-l] [-a] [-w] [targets..]\n");
    assert_eq!(typed(ctx, &mut shell, "help date"), "date\n");
    assert_eq!(typed(ctx, &mut shell, "help load"), "load [-l] [path]\n");
    assert_eq!(typed(ctx, &mut shell, "help pwd"), format!("{}\n", lines[1]));
    assert_eq!(
        typed(ctx, &mut shell, "help nope"),
        "nope: not under /self/vim/bin /self/bin /bin; help lists them"
    );
    assert_eq!(
        typed(ctx, &mut shell, "help ls cat"),
        "help ls cat: usage: help [name]"
    );
}

#[test]
fn tab_offers_the_names_the_flags_of_one_and_the_paths_under_the_cwd() {
    let rig = Rig::new("sh-tab");
    rig.file("note", b"");
    rig.file("other", b"");
    let host = rig.host();
    let ctx = &host.own();
    let mut shell = Shell::default();
    let offer = |ctx: &dyn Calls, shell: &Shell, line: &str| {
        complete(ctx, shell, line).candidates
    };

    // The first token: the names PATH holds in the order they resolve,
    // then the builtins. Tab does not sort, a completion having to
    // offer first what the line would run.
    assert_eq!(offer(ctx, &shell, "l"), ["load", "ls", "log"]);
    assert_eq!(offer(ctx, &shell, "p"), ["pwd"]);
    assert_eq!(offer(ctx, &shell, "c"), ["cat", "call", "cd"]);

    // A token under a dash: the flag fields of the Driver the line
    // names, each as a token under a dash spells it.
    assert_eq!(offer(ctx, &shell, "ls -"), ["-l", "-a", "-w"]);
    assert_eq!(offer(ctx, &shell, "tree -"), ["-L"]);
    assert_eq!(offer(ctx, &shell, "cat -n"), ["-n"]);
    assert_eq!(
        offer(ctx, &shell, "dmesg -"),
        ["-t", "-T", "-H", "-d", "-r", "-x", "-k", "-w", "-W", "-f", "-l"]
    );
    let none: [&str; 0] = [];
    assert_eq!(offer(ctx, &shell, "pwd -"), none, "no declaration");

    // Anything else is a path from where the line stands: a name under
    // the cwd, a path from the root, and a `..` walked as the shell
    // walks one. A Container gets the slash the next Tab goes on from,
    // and a bare token offers every child.
    assert_eq!(typed(ctx, &mut shell, "cd /module/fs/root"), "");
    assert_eq!(offer(ctx, &shell, "cat no"), ["note"]);
    assert_eq!(offer(ctx, &shell, "cat "), ["note", "other"]);
    assert_eq!(offer(ctx, &shell, "ls ../../fs"), ["../../fs/"]);
    assert_eq!(offer(ctx, &shell, "ls ../../fs/"), ["../../fs/root/"]);
    assert_eq!(offer(ctx, &shell, "ls /mod"), ["/module/"]);
    // A path that resolves nowhere offers nothing and is not a fault,
    // and no token past the first offers a value.
    assert_eq!(offer(ctx, &shell, "cat nowhere/n"), none);
    assert_eq!(offer(ctx, &shell, "call open op"), none);
}
