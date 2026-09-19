//! `reovim server --log PATH`: every log entry in the file; without it
//! no file. Through real processes.

mod common;

use common::Server;

#[test]
fn the_log_file_holds_every_entry() {
    let server = Server::with("file", &[], true);
    // The five loads and the sink's own connect line, before any link.
    let first = server.logged(6);
    assert!(first.starts_with("1 "), "{first}");
    // Each at its level: the Kernel's own are `info`, which is 6.
    assert!(first.contains(" kernel 6 load module=fs\n"), "{first}");
    assert!(first.contains(" kernel 6 connect session="), "{first}");

    let made = server.out(&["call", "/module/fs/root", "create", "made"]);
    assert!(made.trim().parse::<u64>().unwrap() > 0, "an rd: {made}");
    // The cli's link, from its connect to the disconnect its end
    // writes; the `call` pair the op made is measurement, kept at
    // `debug`, so it is not here.
    let text = server.logged(8);
    let lines: Vec<&str> = text.lines().collect();
    assert_eq!(lines.len(), 8, "{text}");
    let link = lines[6].split_once(" kernel 6 connect ").unwrap().1;
    assert!(lines[7].ends_with(&format!("disconnect {link}")), "{text}");
    assert!(!text.contains("call rd="), "{text}");

    // `cli log` shows the same entries, and its own connect after them.
    let shown = server.out(&["log"]);
    assert!(shown.starts_with(&text), "{shown}");
    assert_eq!(shown.lines().count(), 9, "{shown}");
    let from = server.out(&["log", "8"]);
    assert!(from.starts_with(lines[7]), "{from}");

    // The same run as `dmesg` reads it: what a link did, and nothing
    // of what it asked for.
    let said = server.out(&["dmesg", "-t", "-l", "info"]);
    assert!(said.starts_with("load module="), "{said}");
    assert!(said.contains("load module=fs\n"), "{said}");
    assert!(said.contains(&format!("disconnect {link}\n")), "{said}");
    assert!(!said.contains("call "), "{said}");
    assert!(!said.contains("op="), "{said}");
}

#[test]
fn without_the_option_no_file_is_touched() {
    let server = Server::start("none");
    server.out(&["stat", "/"]);
    assert!(!server.dir.join("log").exists());
}
